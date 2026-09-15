//! Transport-neutral remote access boundary.
//!
//! Authentication belongs to a transport/security adapter. This module accepts
//! only an already-authenticated Principal, binds it to an ephemeral session,
//! and authorizes its asserted Agent identity before the unchanged external
//! Agent protocol is dispatched. It owns no credentials, network handles, or
//! durable Pong state.

use crate::protocol::{ProtocolCall, ProtocolRequest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

pub const REMOTE_ACCESS_CONTRACT_VERSION: &str = "1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteAccessCapabilities {
    pub contract_version: String,
    pub authentication_required: bool,
    pub reauthentication_required_on_reconnect: bool,
    pub core_restart_invalidates_sessions: bool,
    pub disconnect_cancels_operations: bool,
    pub durable_state_reconciliation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedPrincipal {
    pub principal_id: String,
    pub agent_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteSession {
    pub session_id: String,
    pub principal_id: String,
    pub established_at_ms: i64,
    pub expires_at_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RemoteAccessErrorCode {
    AuthenticationRequired,
    AuthenticationFailed,
    SessionExpired,
    InvalidPrincipal,
    InvalidAgentBinding,
    Forbidden,
    CoreUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteAccessError {
    pub code: RemoteAccessErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl RemoteAccessError {
    pub fn authentication_required() -> Self {
        Self::new(
            RemoteAccessErrorCode::AuthenticationRequired,
            "caller authentication is required",
            false,
        )
    }

    pub fn authentication_failed() -> Self {
        Self::new(
            RemoteAccessErrorCode::AuthenticationFailed,
            "caller authentication failed",
            false,
        )
    }

    pub fn core_unavailable() -> Self {
        Self::new(
            RemoteAccessErrorCode::CoreUnavailable,
            "Pong Core is unavailable",
            true,
        )
    }

    fn new(code: RemoteAccessErrorCode, message: &str, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Clone, Debug)]
struct BoundSession {
    view: RemoteSession,
    agent_ids: BTreeSet<String>,
}

/// Ephemeral session and Principal-to-Agent authorization boundary.
///
/// Recreating this value after Core restart intentionally invalidates all old
/// sessions. Durable recovery remains the responsibility of the protocol and
/// Core using Agent, Execution, Operation, and Workspace identities.
#[derive(Debug, Default)]
pub struct RemoteAccessBoundary {
    sessions: HashMap<String, BoundSession>,
}

impl RemoteAccessBoundary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn capabilities(&self) -> RemoteAccessCapabilities {
        RemoteAccessCapabilities {
            contract_version: REMOTE_ACCESS_CONTRACT_VERSION.into(),
            authentication_required: true,
            reauthentication_required_on_reconnect: true,
            core_restart_invalidates_sessions: true,
            disconnect_cancels_operations: false,
            durable_state_reconciliation: true,
        }
    }

    /// Bind a Principal that a transport/security adapter already authenticated.
    pub fn bind_authenticated(
        &mut self,
        principal: AuthenticatedPrincipal,
        session_id: impl Into<String>,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<RemoteSession, RemoteAccessError> {
        let session_id = session_id.into();
        if principal.principal_id.trim().is_empty()
            || session_id.trim().is_empty()
            || principal.agent_ids.is_empty()
            || principal.agent_ids.iter().any(|id| id.trim().is_empty())
            || ttl_ms <= 0
        {
            return Err(RemoteAccessError::new(
                RemoteAccessErrorCode::InvalidPrincipal,
                "authenticated Principal or session binding is invalid",
                false,
            ));
        }
        let Some(expires_at_ms) = now_ms.checked_add(ttl_ms) else {
            return Err(RemoteAccessError::new(
                RemoteAccessErrorCode::InvalidPrincipal,
                "session lifetime is invalid",
                false,
            ));
        };
        if self.sessions.contains_key(&session_id) {
            return Err(RemoteAccessError::new(
                RemoteAccessErrorCode::Forbidden,
                "session identity is already bound",
                false,
            ));
        }
        let view = RemoteSession {
            session_id: session_id.clone(),
            principal_id: principal.principal_id,
            established_at_ms: now_ms,
            expires_at_ms,
        };
        self.sessions.insert(
            session_id,
            BoundSession {
                view: view.clone(),
                agent_ids: principal.agent_ids.into_iter().collect(),
            },
        );
        Ok(view)
    }

    pub fn authorize(
        &mut self,
        session_id: &str,
        request: ProtocolRequest,
        now_ms: i64,
    ) -> Result<AuthorizedProtocolRequest, RemoteAccessError> {
        let Some(session) = self.sessions.get(session_id) else {
            return Err(RemoteAccessError::new(
                RemoteAccessErrorCode::AuthenticationRequired,
                "an authenticated session is required",
                false,
            ));
        };
        if now_ms >= session.view.expires_at_ms {
            self.sessions.remove(session_id);
            return Err(RemoteAccessError::new(
                RemoteAccessErrorCode::SessionExpired,
                "authenticated session has expired",
                true,
            ));
        }
        if !matches!(request.call, ProtocolCall::Hello) {
            let Some(agent_id) = request.caller_agent_id.as_deref() else {
                return Err(RemoteAccessError::new(
                    RemoteAccessErrorCode::InvalidAgentBinding,
                    "protocol request has no Principal-bound Agent identity",
                    false,
                ));
            };
            if !session.agent_ids.contains(agent_id) {
                return Err(RemoteAccessError::new(
                    RemoteAccessErrorCode::Forbidden,
                    "Principal is not authorized for the asserted Agent",
                    false,
                ));
            }
        }
        Ok(AuthorizedProtocolRequest {
            session: session.view.clone(),
            request,
        })
    }

    pub fn disconnect(&mut self, session_id: &str) -> bool {
        self.sessions.remove(session_id).is_some()
    }

    pub fn invalidate_all(&mut self) {
        self.sessions.clear();
    }

    pub fn session(&self, session_id: &str) -> Option<&RemoteSession> {
        self.sessions.get(session_id).map(|session| &session.view)
    }
}

#[derive(Clone, Debug)]
pub struct AuthorizedProtocolRequest {
    session: RemoteSession,
    request: ProtocolRequest,
}

impl AuthorizedProtocolRequest {
    pub fn session(&self) -> &RemoteSession {
        &self.session
    }

    pub fn request(&self) -> &ProtocolRequest {
        &self.request
    }

    pub fn into_request(self) -> ProtocolRequest {
        self.request
    }
}
