//! Minimal synchronous HTTP transport for External Agent Protocol v1.0.
//!
//! The adapter owns HTTP, credential verification, and ephemeral sessions. It
//! knows only the protocol dispatcher abstraction and never storage internals.

use crate::core_service::ProtocolDispatch;
use crate::protocol::{ProtocolErrorCode, ProtocolRequest, ProtocolResponse, ProtocolStatus};
use crate::{AuthenticatedPrincipal, RemoteAccessBoundary, RemoteAccessError};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

pub const HTTP_PROTOCOL_PATH: &str = "/v1/protocol";
pub const DEFAULT_HTTP_MAX_BODY_BYTES: usize = 1024 * 1024;
pub const DEFAULT_HTTP_SESSION_TTL_MS: i64 = 30_000;
pub const DEFAULT_HTTP_WORKER_THREADS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpServerConfig {
    pub listen_addr: SocketAddr,
    pub allow_non_loopback: bool,
    pub max_body_bytes: usize,
    pub session_ttl_ms: i64,
    pub worker_threads: usize,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            allow_non_loopback: false,
            max_body_bytes: DEFAULT_HTTP_MAX_BODY_BYTES,
            session_ttl_ms: DEFAULT_HTTP_SESSION_TTL_MS,
            worker_threads: DEFAULT_HTTP_WORKER_THREADS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpServerError {
    message: &'static str,
}

impl HttpServerError {
    fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for HttpServerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for HttpServerError {}

pub struct CredentialGrant {
    pub credential: String,
    pub principal_id: String,
    pub agent_ids: Vec<String>,
    pub expires_at_ms: Option<i64>,
}

#[derive(Clone)]
struct PrincipalGrant {
    principal_id: String,
    agent_ids: Vec<String>,
    expires_at_ms: Option<i64>,
}

pub trait CredentialVerifier: Send + Sync + 'static {
    fn authenticate(
        &self,
        credential: &str,
        now_ms: i64,
    ) -> Result<AuthenticatedPrincipal, RemoteAccessError>;
}

pub struct StaticCredentialVerifier {
    grants: HashMap<[u8; 32], PrincipalGrant>,
}

impl StaticCredentialVerifier {
    pub fn new(grants: Vec<CredentialGrant>) -> Result<Self, HttpServerError> {
        if grants.is_empty() {
            return Err(HttpServerError::new(
                "at least one credential grant is required",
            ));
        }
        let mut indexed = HashMap::new();
        for grant in grants {
            if grant.credential.trim().is_empty()
                || grant.principal_id.trim().is_empty()
                || grant.agent_ids.is_empty()
                || grant.agent_ids.iter().any(|agent| agent.trim().is_empty())
            {
                return Err(HttpServerError::new("credential grant is invalid"));
            }
            let digest: [u8; 32] = Sha256::digest(grant.credential.as_bytes()).into();
            if indexed
                .insert(
                    digest,
                    PrincipalGrant {
                        principal_id: grant.principal_id,
                        agent_ids: grant.agent_ids,
                        expires_at_ms: grant.expires_at_ms,
                    },
                )
                .is_some()
            {
                return Err(HttpServerError::new("credential is duplicated"));
            }
        }
        Ok(Self { grants: indexed })
    }
}

impl CredentialVerifier for StaticCredentialVerifier {
    fn authenticate(
        &self,
        credential: &str,
        now_ms: i64,
    ) -> Result<AuthenticatedPrincipal, RemoteAccessError> {
        let digest: [u8; 32] = Sha256::digest(credential.as_bytes()).into();
        let grant = self
            .grants
            .get(&digest)
            .ok_or_else(RemoteAccessError::authentication_failed)?;
        if grant
            .expires_at_ms
            .is_some_and(|expires_at_ms| now_ms >= expires_at_ms)
        {
            return Err(RemoteAccessError::authentication_failed());
        }
        Ok(AuthenticatedPrincipal {
            principal_id: grant.principal_id.clone(),
            agent_ids: grant.agent_ids.clone(),
        })
    }
}

struct HttpState {
    verifier: Arc<dyn CredentialVerifier>,
    remote: Mutex<RemoteAccessBoundary>,
    dispatcher: Arc<dyn ProtocolDispatch>,
    max_body_bytes: usize,
    session_ttl_ms: i64,
    next_session: AtomicU64,
}

pub struct HttpRemoteServer {
    listen_addr: SocketAddr,
    server: Arc<Server>,
    stopping: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

impl HttpRemoteServer {
    pub fn start(
        config: HttpServerConfig,
        verifier: Arc<dyn CredentialVerifier>,
        dispatcher: Arc<dyn ProtocolDispatch>,
    ) -> Result<Self, HttpServerError> {
        validate_config(&config)?;
        let server = Arc::new(
            Server::http(config.listen_addr)
                .map_err(|_| HttpServerError::new("HTTP listener could not start"))?,
        );
        let listen_addr = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| HttpServerError::new("HTTP listener is not an IP socket"))?;
        let state = Arc::new(HttpState {
            verifier,
            remote: Mutex::new(RemoteAccessBoundary::new()),
            dispatcher,
            max_body_bytes: config.max_body_bytes,
            session_ttl_ms: config.session_ttl_ms,
            next_session: AtomicU64::new(1),
        });
        let stopping = Arc::new(AtomicBool::new(false));
        let workers = (0..config.worker_threads)
            .map(|_| {
                let server = Arc::clone(&server);
                let stopping = Arc::clone(&stopping);
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    while !stopping.load(Ordering::Acquire) {
                        match server.recv_timeout(Duration::from_millis(50)) {
                            Ok(Some(request)) => handle_request(request, &state),
                            Ok(None) => {}
                            Err(_) if stopping.load(Ordering::Acquire) => break,
                            Err(_) => break,
                        }
                    }
                })
            })
            .collect();
        Ok(Self {
            listen_addr,
            server,
            stopping,
            workers,
        })
    }

    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    pub fn shutdown(mut self) -> Result<(), HttpServerError> {
        self.stop_and_join()
    }

    fn stop_and_join(&mut self) -> Result<(), HttpServerError> {
        self.stopping.store(true, Ordering::Release);
        self.server.unblock();
        for worker in self.workers.drain(..) {
            worker
                .join()
                .map_err(|_| HttpServerError::new("HTTP server worker failed"))?;
        }
        Ok(())
    }
}

impl Drop for HttpRemoteServer {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
    }
}

fn validate_config(config: &HttpServerConfig) -> Result<(), HttpServerError> {
    if !config.listen_addr.ip().is_loopback() && !config.allow_non_loopback {
        return Err(HttpServerError::new(
            "non-loopback HTTP binding requires explicit authorization",
        ));
    }
    if config.max_body_bytes == 0 || config.session_ttl_ms <= 0 || config.worker_threads == 0 {
        return Err(HttpServerError::new("HTTP server limits are invalid"));
    }
    Ok(())
}

fn handle_request(mut request: Request, state: &HttpState) {
    if request.url() != HTTP_PROTOCOL_PATH {
        respond_transport_error(request, 404, "NOT_FOUND", "HTTP endpoint was not found");
        return;
    }
    if request.method() != &Method::Post {
        respond_transport_error(
            request,
            405,
            "METHOD_NOT_ALLOWED",
            "HTTP method is not allowed",
        );
        return;
    }
    if !has_json_content_type(&request) {
        respond_transport_error(
            request,
            415,
            "UNSUPPORTED_MEDIA_TYPE",
            "application/json is required",
        );
        return;
    }
    let credential = match bearer_credential(&request) {
        Ok(credential) => credential,
        Err(error) => {
            respond_remote_error(request, error);
            return;
        }
    };
    let authentication_time_ms = host_now_ms();
    let principal = match state
        .verifier
        .authenticate(&credential, authentication_time_ms)
    {
        Ok(principal) => principal,
        Err(error) => {
            respond_remote_error(request, error);
            return;
        }
    };
    if request
        .body_length()
        .is_some_and(|length| length > state.max_body_bytes)
    {
        respond_transport_error(request, 413, "PAYLOAD_TOO_LARGE", "HTTP body is too large");
        return;
    }
    let mut body = Vec::new();
    if request
        .as_reader()
        .take(state.max_body_bytes.saturating_add(1) as u64)
        .read_to_end(&mut body)
        .is_err()
    {
        respond_transport_error(
            request,
            400,
            "INVALID_REQUEST",
            "HTTP body could not be read",
        );
        return;
    }
    if body.len() > state.max_body_bytes {
        respond_transport_error(request, 413, "PAYLOAD_TOO_LARGE", "HTTP body is too large");
        return;
    }
    let protocol_request = match serde_json::from_slice::<ProtocolRequest>(&body) {
        Ok(request) => request,
        Err(_) => {
            let response = ProtocolResponse::invalid_envelope(request_id_from_invalid_json(&body));
            respond_protocol(request, response);
            return;
        }
    };
    let now_ms = host_now_ms();
    let session_id = format!(
        "http-{}-{}",
        now_ms,
        state.next_session.fetch_add(1, Ordering::Relaxed)
    );
    let authorized = {
        let mut remote = match state.remote.lock() {
            Ok(remote) => remote,
            Err(_) => {
                respond_remote_error(request, RemoteAccessError::core_unavailable());
                return;
            }
        };
        if let Err(error) =
            remote.bind_authenticated(principal, session_id.clone(), now_ms, state.session_ttl_ms)
        {
            respond_remote_error(request, error);
            return;
        }
        let authorized = remote.authorize(&session_id, protocol_request, now_ms);
        remote.disconnect(&session_id);
        authorized
    };
    let authorized = match authorized {
        Ok(authorized) => authorized,
        Err(error) => {
            respond_remote_error(request, error);
            return;
        }
    };
    match state.dispatcher.dispatch(authorized.into_request(), now_ms) {
        Ok(response) => respond_protocol(request, response),
        Err(_) => respond_remote_error(request, RemoteAccessError::core_unavailable()),
    }
}

fn has_json_content_type(request: &Request) -> bool {
    request.headers().iter().any(|header| {
        header.field.equiv("Content-Type")
            && header
                .value
                .as_str()
                .split(';')
                .next()
                .map(str::trim)
                .is_some_and(|value| value.eq_ignore_ascii_case("application/json"))
    })
}

fn bearer_credential(request: &Request) -> Result<String, RemoteAccessError> {
    let values = request
        .headers()
        .iter()
        .filter(|header| header.field.equiv("Authorization"))
        .map(|header| header.value.as_str())
        .collect::<Vec<_>>();
    let [value] = values.as_slice() else {
        if values.is_empty() {
            return Err(RemoteAccessError::authentication_required());
        }
        return Err(RemoteAccessError::authentication_failed());
    };
    let Some((scheme, credential)) = value.split_once(' ') else {
        return Err(RemoteAccessError::authentication_failed());
    };
    if !scheme.eq_ignore_ascii_case("Bearer")
        || credential.trim().is_empty()
        || credential.contains(char::is_whitespace)
    {
        return Err(RemoteAccessError::authentication_failed());
    }
    Ok(credential.to_owned())
}

fn request_id_from_invalid_json(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("request_id")?.as_str().map(Into::into))
        .filter(|request_id: &String| !request_id.trim().is_empty())
}

fn respond_protocol(request: Request, response: ProtocolResponse) {
    let status = protocol_http_status(&response);
    respond_json(request, status, &response);
}

fn protocol_http_status(response: &ProtocolResponse) -> u16 {
    if response.status == ProtocolStatus::Ok {
        return 200;
    }
    match response.error.as_ref().map(|error| &error.code) {
        Some(ProtocolErrorCode::Unauthorized) => 401,
        Some(ProtocolErrorCode::Forbidden) => 403,
        Some(ProtocolErrorCode::NotFound) => 404,
        Some(
            ProtocolErrorCode::Conflict
            | ProtocolErrorCode::LeaseConflict
            | ProtocolErrorCode::RevisionConflict
            | ProtocolErrorCode::InvalidState
            | ProtocolErrorCode::IdempotencyConflict
            | ProtocolErrorCode::RecoveryRequired,
        ) => 409,
        Some(ProtocolErrorCode::ResourceExhausted) => 429,
        Some(ProtocolErrorCode::NotSupported) => 501,
        Some(ProtocolErrorCode::InternalError | ProtocolErrorCode::IntegrityError) => 500,
        _ => 400,
    }
}

fn respond_remote_error(request: Request, error: RemoteAccessError) {
    use crate::RemoteAccessErrorCode as Code;
    let status = match error.code {
        Code::AuthenticationRequired | Code::AuthenticationFailed | Code::SessionExpired => 401,
        Code::InvalidPrincipal | Code::InvalidAgentBinding => 400,
        Code::Forbidden => 403,
        Code::CoreUnavailable => 503,
    };
    respond_json(request, status, &error);
}

#[derive(Serialize)]
struct TransportError<'a> {
    code: &'a str,
    message: &'a str,
    retryable: bool,
}

fn respond_transport_error(request: Request, status: u16, code: &str, message: &str) {
    respond_json(
        request,
        status,
        &TransportError {
            code,
            message,
            retryable: false,
        },
    );
}

fn respond_json<T: Serialize>(request: Request, status: u16, value: &T) {
    let data = serde_json::to_vec(value).unwrap_or_else(|_| {
        br#"{"code":"INTERNAL_ERROR","message":"response serialization failed","retryable":false}"#.to_vec()
    });
    let mut response = Response::from_data(data).with_status_code(StatusCode(status));
    for (name, value) in [
        ("Content-Type", "application/json"),
        ("Cache-Control", "no-store"),
        ("X-Content-Type-Options", "nosniff"),
    ] {
        if let Ok(header) = Header::from_bytes(name, value) {
            response.add_header(header);
        }
    }
    let _ = request.respond(response);
}

fn host_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX)
}

pub fn is_loopback_address(address: SocketAddr) -> bool {
    match address.ip() {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback(),
    }
}
