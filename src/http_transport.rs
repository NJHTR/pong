//! Minimal synchronous HTTP transport for External Agent Protocol v1.0.
//!
//! The adapter owns HTTP, credential verification, and ephemeral sessions. It
//! knows only the protocol dispatcher abstraction and never storage internals.

use crate::core_service::ProtocolDispatch;
use crate::protocol::{
    ProtocolErrorCode, ProtocolRequest, ProtocolResponse, ProtocolResult, ProtocolStatus,
};
use crate::{AuthenticatedPrincipal, RemoteAccessBoundary, RemoteAccessError};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

pub const HTTP_PROTOCOL_PATH: &str = "/v1/protocol";
pub const DEFAULT_HTTP_MAX_BODY_BYTES: usize = 1024 * 1024;
pub const DEFAULT_HTTP_SESSION_TTL_MS: i64 = 30_000;
pub const DEFAULT_HTTP_WORKER_THREADS: usize = 8;
pub const DEFAULT_HTTP_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
pub const DEFAULT_HTTP_RATE_LIMIT_REQUESTS: usize = 120;
pub const DEFAULT_HTTP_RATE_LIMIT_WINDOW_MS: i64 = 60_000;
pub const DEFAULT_HTTP_RATE_LIMIT_MAX_PRINCIPALS: usize = 1024;
const HTTP_DIAGNOSTIC_CAPACITY: usize = 64;
const RESPONSE_TOO_LARGE_BODY: &[u8] =
    br#"{"code":"RESPONSE_TOO_LARGE","message":"HTTP response is too large","retryable":false}"#;
pub const MIN_HTTP_MAX_RESPONSE_BYTES: usize = RESPONSE_TOO_LARGE_BODY.len();

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpServerConfig {
    pub listen_addr: SocketAddr,
    pub allow_non_loopback: bool,
    pub max_body_bytes: usize,
    pub session_ttl_ms: i64,
    pub worker_threads: usize,
    pub max_response_bytes: usize,
    pub rate_limit_requests: usize,
    pub rate_limit_window_ms: i64,
    pub rate_limit_max_principals: usize,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            allow_non_loopback: false,
            max_body_bytes: DEFAULT_HTTP_MAX_BODY_BYTES,
            session_ttl_ms: DEFAULT_HTTP_SESSION_TTL_MS,
            worker_threads: DEFAULT_HTTP_WORKER_THREADS,
            max_response_bytes: DEFAULT_HTTP_MAX_RESPONSE_BYTES,
            rate_limit_requests: DEFAULT_HTTP_RATE_LIMIT_REQUESTS,
            rate_limit_window_ms: DEFAULT_HTTP_RATE_LIMIT_WINDOW_MS,
            rate_limit_max_principals: DEFAULT_HTTP_RATE_LIMIT_MAX_PRINCIPALS,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredentialGrant {
    pub credential: String,
    pub principal_id: String,
    pub agent_ids: Vec<String>,
    pub expires_at_ms: Option<i64>,
}

fn validate_grant(grant: &CredentialGrant) -> Result<(), HttpServerError> {
    if grant.credential.trim().is_empty()
        || grant.principal_id.trim().is_empty()
        || grant.agent_ids.is_empty()
        || grant.agent_ids.iter().any(|agent| agent.trim().is_empty())
    {
        return Err(HttpServerError::new("credential grant is invalid"));
    }
    Ok(())
}

fn grant_parts(grant: CredentialGrant) -> Result<([u8; 32], PrincipalGrant), HttpServerError> {
    validate_grant(&grant)?;
    let digest: [u8; 32] = Sha256::digest(grant.credential.as_bytes()).into();
    Ok((
        digest,
        PrincipalGrant {
            principal_id: grant.principal_id,
            agent_ids: grant.agent_ids,
            expires_at_ms: grant.expires_at_ms,
        },
    ))
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
    grants: RwLock<HashMap<[u8; 32], PrincipalGrant>>,
}

impl StaticCredentialVerifier {
    pub fn new(grants: Vec<CredentialGrant>) -> Result<Self, HttpServerError> {
        let indexed = index_grants(grants)?;
        Ok(Self {
            grants: RwLock::new(indexed),
        })
    }

    /// Adds a credential without invalidating existing credentials.
    pub fn add_grant(&self, grant: CredentialGrant) -> Result<(), HttpServerError> {
        let (digest, principal_grant) = grant_parts(grant)?;
        let mut grants = self
            .grants
            .write()
            .map_err(|_| HttpServerError::new("credential store is unavailable"))?;
        if grants.contains_key(&digest) {
            return Err(HttpServerError::new("credential is duplicated"));
        }
        grants.insert(digest, principal_grant);
        Ok(())
    }

    /// Atomically replaces all active credentials after the new set validates.
    pub fn replace_grants(&self, grants: Vec<CredentialGrant>) -> Result<(), HttpServerError> {
        let replacement = index_grants(grants)?;
        let mut active = self
            .grants
            .write()
            .map_err(|_| HttpServerError::new("credential store is unavailable"))?;
        *active = replacement;
        Ok(())
    }

    /// Revokes a credential. The credential secret is never retained in diagnostics.
    pub fn revoke(&self, credential: &str) -> Result<bool, HttpServerError> {
        let digest: [u8; 32] = Sha256::digest(credential.as_bytes()).into();
        let mut grants = self
            .grants
            .write()
            .map_err(|_| HttpServerError::new("credential store is unavailable"))?;
        Ok(grants.remove(&digest).is_some())
    }
}

fn index_grants(
    grants: Vec<CredentialGrant>,
) -> Result<HashMap<[u8; 32], PrincipalGrant>, HttpServerError> {
    if grants.is_empty() {
        return Err(HttpServerError::new(
            "at least one credential grant is required",
        ));
    }
    let mut indexed = HashMap::new();
    for grant in grants {
        let (digest, principal_grant) = grant_parts(grant)?;
        if indexed.insert(digest, principal_grant).is_some() {
            return Err(HttpServerError::new("credential is duplicated"));
        }
    }
    Ok(indexed)
}

impl CredentialVerifier for StaticCredentialVerifier {
    fn authenticate(
        &self,
        credential: &str,
        now_ms: i64,
    ) -> Result<AuthenticatedPrincipal, RemoteAccessError> {
        let digest: [u8; 32] = Sha256::digest(credential.as_bytes()).into();
        let grants = self
            .grants
            .read()
            .map_err(|_| RemoteAccessError::core_unavailable())?;
        let grant = grants
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HttpRequestCorrelation {
    pub request_id: String,
    pub operation_id: Option<String>,
    pub principal_id: String,
    pub agent_id: Option<String>,
    pub execution_id: Option<String>,
    pub workspace_id: Option<String>,
    pub status: u16,
    pub latency_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HttpMetricsSnapshot {
    pub requests_total: u64,
    pub error_responses: u64,
    pub auth_rejections: u64,
    pub authorization_rejections: u64,
    pub rate_limit_rejections: u64,
    pub payload_rejections: u64,
    pub active_requests: u64,
    pub peak_active_requests: u64,
    pub worker_threads: usize,
    pub handler_panics: u64,
    pub tracked_rate_limit_principals: u64,
    pub peak_tracked_rate_limit_principals: u64,
    pub rate_limit_capacity_rejections: u64,
    pub recent_requests: Vec<HttpRequestCorrelation>,
    pub recent_diagnostics: Vec<HttpDiagnosticEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HttpDiagnosticOutcome {
    RequestCompleted,
    AuthRejected,
    AuthzRejected,
    RateLimited,
    MalformedRequest,
    ProtocolError,
    RevisionConflict,
    LeaseConflict,
    OperationAccepted,
    OperationCompleted,
    OperationFailed,
    UnknownOutcome,
    CoreUnavailable,
    ServerShutdown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HttpDiagnosticEvent {
    pub request_id: Option<String>,
    pub operation_id: Option<String>,
    pub principal_id: Option<String>,
    pub agent_id: Option<String>,
    pub execution_id: Option<String>,
    pub workspace_id: Option<String>,
    pub status: Option<u16>,
    pub outcome: HttpDiagnosticOutcome,
    pub latency_ms: u64,
}

struct HttpMetrics {
    requests_total: AtomicU64,
    error_responses: AtomicU64,
    auth_rejections: AtomicU64,
    authorization_rejections: AtomicU64,
    rate_limit_rejections: AtomicU64,
    payload_rejections: AtomicU64,
    active_requests: AtomicU64,
    peak_active_requests: AtomicU64,
    worker_threads: usize,
    handler_panics: AtomicU64,
    tracked_rate_limit_principals: AtomicU64,
    peak_tracked_rate_limit_principals: AtomicU64,
    rate_limit_capacity_rejections: AtomicU64,
    recent_requests: Mutex<VecDeque<HttpRequestCorrelation>>,
    recent_diagnostics: Mutex<VecDeque<HttpDiagnosticEvent>>,
}

impl HttpMetrics {
    fn new(worker_threads: usize) -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            error_responses: AtomicU64::new(0),
            auth_rejections: AtomicU64::new(0),
            authorization_rejections: AtomicU64::new(0),
            rate_limit_rejections: AtomicU64::new(0),
            payload_rejections: AtomicU64::new(0),
            active_requests: AtomicU64::new(0),
            peak_active_requests: AtomicU64::new(0),
            worker_threads,
            handler_panics: AtomicU64::new(0),
            tracked_rate_limit_principals: AtomicU64::new(0),
            peak_tracked_rate_limit_principals: AtomicU64::new(0),
            rate_limit_capacity_rejections: AtomicU64::new(0),
            recent_requests: Mutex::new(VecDeque::with_capacity(HTTP_DIAGNOSTIC_CAPACITY)),
            recent_diagnostics: Mutex::new(VecDeque::with_capacity(HTTP_DIAGNOSTIC_CAPACITY)),
        }
    }

    fn snapshot(&self) -> HttpMetricsSnapshot {
        HttpMetricsSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            error_responses: self.error_responses.load(Ordering::Relaxed),
            auth_rejections: self.auth_rejections.load(Ordering::Relaxed),
            authorization_rejections: self.authorization_rejections.load(Ordering::Relaxed),
            rate_limit_rejections: self.rate_limit_rejections.load(Ordering::Relaxed),
            payload_rejections: self.payload_rejections.load(Ordering::Relaxed),
            active_requests: self.active_requests.load(Ordering::Relaxed),
            peak_active_requests: self.peak_active_requests.load(Ordering::Relaxed),
            worker_threads: self.worker_threads,
            handler_panics: self.handler_panics.load(Ordering::Relaxed),
            tracked_rate_limit_principals: self
                .tracked_rate_limit_principals
                .load(Ordering::Relaxed),
            peak_tracked_rate_limit_principals: self
                .peak_tracked_rate_limit_principals
                .load(Ordering::Relaxed),
            rate_limit_capacity_rejections: self
                .rate_limit_capacity_rejections
                .load(Ordering::Relaxed),
            recent_requests: self
                .recent_requests
                .lock()
                .map(|items| items.iter().cloned().collect())
                .unwrap_or_default(),
            recent_diagnostics: self
                .recent_diagnostics
                .lock()
                .map(|items| items.iter().cloned().collect())
                .unwrap_or_default(),
        }
    }

    fn record(&self, correlation: HttpRequestCorrelation) {
        if let Ok(mut items) = self.recent_requests.lock() {
            if items.len() >= HTTP_DIAGNOSTIC_CAPACITY {
                items.pop_front();
            }
            items.push_back(correlation);
        }
    }

    fn record_diagnostic(&self, event: HttpDiagnosticEvent) {
        if let Ok(mut items) = self.recent_diagnostics.lock() {
            if items.len() >= HTTP_DIAGNOSTIC_CAPACITY {
                items.pop_front();
            }
            items.push_back(event);
        }
    }

    fn rate_limiter_size(&self, tracked: usize, capacity_rejected: bool) {
        let tracked = u64::try_from(tracked).unwrap_or(u64::MAX);
        self.tracked_rate_limit_principals
            .store(tracked, Ordering::Relaxed);
        let mut peak = self
            .peak_tracked_rate_limit_principals
            .load(Ordering::Relaxed);
        while tracked > peak {
            match self
                .peak_tracked_rate_limit_principals
                .compare_exchange_weak(peak, tracked, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
        if capacity_rejected {
            self.rate_limit_capacity_rejections
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    fn request_started(&self) {
        let active = self.active_requests.fetch_add(1, Ordering::Relaxed) + 1;
        let mut peak = self.peak_active_requests.load(Ordering::Relaxed);
        while active > peak {
            match self.peak_active_requests.compare_exchange_weak(
                peak,
                active,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }
}

struct RateLimiter {
    limit: usize,
    window_ms: i64,
    max_principals: usize,
    principals: HashMap<String, (i64, usize)>,
}

struct RateLimitDecision {
    allowed: bool,
    tracked_principals: usize,
    capacity_rejected: bool,
}

impl RateLimiter {
    fn new(limit: usize, window_ms: i64, max_principals: usize) -> Self {
        Self {
            limit,
            window_ms,
            max_principals,
            principals: HashMap::new(),
        }
    }

    fn allow(&mut self, principal_id: &str, now_ms: i64) -> RateLimitDecision {
        self.principals.retain(|_, (window_started_ms, _)| {
            now_ms.saturating_sub(*window_started_ms) < self.window_ms
        });
        if !self.principals.contains_key(principal_id)
            && self.principals.len() >= self.max_principals
        {
            return RateLimitDecision {
                allowed: false,
                tracked_principals: self.principals.len(),
                capacity_rejected: true,
            };
        }
        let allowed = {
            let entry = self
                .principals
                .entry(principal_id.to_owned())
                .or_insert((now_ms, 0));
            if now_ms.saturating_sub(entry.0) >= self.window_ms {
                *entry = (now_ms, 0);
            }
            if entry.1 >= self.limit {
                false
            } else {
                entry.1 += 1;
                true
            }
        };
        if !allowed {
            return RateLimitDecision {
                allowed: false,
                tracked_principals: self.principals.len(),
                capacity_rejected: false,
            };
        }
        RateLimitDecision {
            allowed: true,
            tracked_principals: self.principals.len(),
            capacity_rejected: false,
        }
    }
}

struct HttpState {
    verifier: Arc<dyn CredentialVerifier>,
    remote: Mutex<RemoteAccessBoundary>,
    dispatcher: Arc<dyn ProtocolDispatch>,
    max_body_bytes: usize,
    session_ttl_ms: i64,
    next_session: AtomicU64,
    max_response_bytes: usize,
    rate_limiter: Mutex<RateLimiter>,
    metrics: Arc<HttpMetrics>,
}

pub struct HttpRemoteServer {
    listen_addr: SocketAddr,
    server: Arc<Server>,
    stopping: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
    metrics: Arc<HttpMetrics>,
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
            max_response_bytes: config.max_response_bytes,
            rate_limiter: Mutex::new(RateLimiter::new(
                config.rate_limit_requests,
                config.rate_limit_window_ms,
                config.rate_limit_max_principals,
            )),
            metrics: Arc::new(HttpMetrics::new(config.worker_threads)),
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
                            Ok(Some(request)) => {
                                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    handle_request(request, &state)
                                }))
                                .is_err()
                                {
                                    state
                                        .metrics
                                        .error_responses
                                        .fetch_add(1, Ordering::Relaxed);
                                    state.metrics.handler_panics.fetch_add(1, Ordering::Relaxed);
                                    state.metrics.record_diagnostic(HttpDiagnosticEvent {
                                        request_id: None,
                                        operation_id: None,
                                        principal_id: None,
                                        agent_id: None,
                                        execution_id: None,
                                        workspace_id: None,
                                        status: Some(500),
                                        outcome: HttpDiagnosticOutcome::CoreUnavailable,
                                        latency_ms: 0,
                                    });
                                }
                            }
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
            metrics: Arc::clone(&state.metrics),
        })
    }

    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    pub fn metrics(&self) -> HttpMetricsSnapshot {
        self.metrics.snapshot()
    }

    pub fn shutdown(mut self) -> Result<(), HttpServerError> {
        self.stop_and_join()
    }

    pub fn shutdown_with_metrics(mut self) -> Result<HttpMetricsSnapshot, HttpServerError> {
        self.stop_and_join()?;
        Ok(self.metrics.snapshot())
    }

    fn stop_and_join(&mut self) -> Result<(), HttpServerError> {
        if !self.stopping.swap(true, Ordering::AcqRel) {
            self.metrics.record_diagnostic(HttpDiagnosticEvent {
                request_id: None,
                operation_id: None,
                principal_id: None,
                agent_id: None,
                execution_id: None,
                workspace_id: None,
                status: None,
                outcome: HttpDiagnosticOutcome::ServerShutdown,
                latency_ms: 0,
            });
        }
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
    if config.max_body_bytes == 0
        || config.max_response_bytes < MIN_HTTP_MAX_RESPONSE_BYTES
        || config.session_ttl_ms <= 0
        || config.worker_threads == 0
        || config.rate_limit_requests == 0
        || config.rate_limit_window_ms <= 0
        || config.rate_limit_max_principals == 0
    {
        return Err(HttpServerError::new("HTTP server limits are invalid"));
    }
    Ok(())
}

fn handle_request(mut request: Request, state: &HttpState) {
    let started = Instant::now();
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    state.metrics.request_started();
    let _active = ActiveRequest {
        metrics: Arc::clone(&state.metrics),
    };
    if request.url() != HTTP_PROTOCOL_PATH {
        mark_error(state);
        record_diagnostic(
            state,
            started,
            HttpDiagnosticOutcome::MalformedRequest,
            Some(404),
        );
        respond_transport_error(
            request,
            404,
            "NOT_FOUND",
            "HTTP endpoint was not found",
            state.max_response_bytes,
        );
        return;
    }
    if request.method() != &Method::Post {
        mark_error(state);
        record_diagnostic(
            state,
            started,
            HttpDiagnosticOutcome::MalformedRequest,
            Some(405),
        );
        respond_transport_error(
            request,
            405,
            "METHOD_NOT_ALLOWED",
            "HTTP method is not allowed",
            state.max_response_bytes,
        );
        return;
    }
    if !has_json_content_type(&request) {
        mark_error(state);
        record_diagnostic(
            state,
            started,
            HttpDiagnosticOutcome::MalformedRequest,
            Some(415),
        );
        respond_transport_error(
            request,
            415,
            "UNSUPPORTED_MEDIA_TYPE",
            "application/json is required",
            state.max_response_bytes,
        );
        return;
    }
    let credential = match bearer_credential(&request) {
        Ok(credential) => credential,
        Err(error) => {
            mark_error(state);
            state
                .metrics
                .auth_rejections
                .fetch_add(1, Ordering::Relaxed);
            record_diagnostic(
                state,
                started,
                HttpDiagnosticOutcome::AuthRejected,
                Some(401),
            );
            respond_remote_error(request, error, state.max_response_bytes);
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
            mark_error(state);
            state
                .metrics
                .auth_rejections
                .fetch_add(1, Ordering::Relaxed);
            record_diagnostic(
                state,
                started,
                HttpDiagnosticOutcome::AuthRejected,
                Some(401),
            );
            respond_remote_error(request, error, state.max_response_bytes);
            return;
        }
    };
    if request
        .body_length()
        .is_some_and(|length| length > state.max_body_bytes)
    {
        mark_error(state);
        state
            .metrics
            .payload_rejections
            .fetch_add(1, Ordering::Relaxed);
        record_diagnostic_for_principal(
            state,
            started,
            HttpDiagnosticOutcome::MalformedRequest,
            413,
            &principal.principal_id,
        );
        respond_transport_error(
            request,
            413,
            "PAYLOAD_TOO_LARGE",
            "HTTP body is too large",
            state.max_response_bytes,
        );
        return;
    }
    let rate_limit = state
        .rate_limiter
        .lock()
        .map(|mut limiter| limiter.allow(&principal.principal_id, authentication_time_ms))
        .unwrap_or(RateLimitDecision {
            allowed: false,
            tracked_principals: 0,
            capacity_rejected: false,
        });
    state
        .metrics
        .rate_limiter_size(rate_limit.tracked_principals, rate_limit.capacity_rejected);
    if !rate_limit.allowed {
        mark_error(state);
        state
            .metrics
            .rate_limit_rejections
            .fetch_add(1, Ordering::Relaxed);
        record_diagnostic_for_principal(
            state,
            started,
            HttpDiagnosticOutcome::RateLimited,
            429,
            &principal.principal_id,
        );
        respond_transport_error(
            request,
            429,
            "RATE_LIMITED",
            "request rate limit exceeded",
            state.max_response_bytes,
        );
        return;
    }
    let mut body = Vec::new();
    if request
        .as_reader()
        .take(state.max_body_bytes.saturating_add(1) as u64)
        .read_to_end(&mut body)
        .is_err()
    {
        mark_error(state);
        record_diagnostic_for_principal(
            state,
            started,
            HttpDiagnosticOutcome::MalformedRequest,
            400,
            &principal.principal_id,
        );
        respond_transport_error(
            request,
            400,
            "INVALID_REQUEST",
            "HTTP body could not be read",
            state.max_response_bytes,
        );
        return;
    }
    if body.len() > state.max_body_bytes {
        mark_error(state);
        state
            .metrics
            .payload_rejections
            .fetch_add(1, Ordering::Relaxed);
        record_diagnostic_for_principal(
            state,
            started,
            HttpDiagnosticOutcome::MalformedRequest,
            413,
            &principal.principal_id,
        );
        respond_transport_error(
            request,
            413,
            "PAYLOAD_TOO_LARGE",
            "HTTP body is too large",
            state.max_response_bytes,
        );
        return;
    }
    let protocol_request = match serde_json::from_slice::<ProtocolRequest>(&body) {
        Ok(request) => request,
        Err(_) => {
            mark_error(state);
            let mut event =
                diagnostic_event(started, HttpDiagnosticOutcome::MalformedRequest, Some(400));
            event.request_id = request_id_from_invalid_json(&body);
            event.principal_id = Some(principal.principal_id.clone());
            state.metrics.record_diagnostic(event);
            let response = ProtocolResponse::invalid_envelope(request_id_from_invalid_json(&body));
            respond_protocol(request, response, state.max_response_bytes);
            return;
        }
    };
    let request_id = protocol_request.request_id.clone();
    let operation_id = protocol_request.operation_id.clone();
    let agent_id = protocol_request.caller_agent_id.clone();
    let (execution_id, workspace_id) = correlation_resources(&protocol_request);
    let principal_id = principal.principal_id.clone();
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
                mark_error(state);
                let mut event =
                    diagnostic_event(started, HttpDiagnosticOutcome::CoreUnavailable, Some(503));
                event.request_id = Some(request_id);
                event.operation_id = operation_id;
                event.principal_id = Some(principal_id);
                event.agent_id = agent_id;
                event.execution_id = execution_id;
                event.workspace_id = workspace_id;
                state.metrics.record_diagnostic(event);
                respond_remote_error(
                    request,
                    RemoteAccessError::core_unavailable(),
                    state.max_response_bytes,
                );
                return;
            }
        };
        if let Err(error) =
            remote.bind_authenticated(principal, session_id.clone(), now_ms, state.session_ttl_ms)
        {
            mark_error(state);
            state
                .metrics
                .authorization_rejections
                .fetch_add(1, Ordering::Relaxed);
            respond_remote_error(request, error, state.max_response_bytes);
            return;
        }
        let authorized = remote.authorize(&session_id, protocol_request, now_ms);
        remote.disconnect(&session_id);
        authorized
    };
    let authorized = match authorized {
        Ok(authorized) => authorized,
        Err(error) => {
            mark_error(state);
            state
                .metrics
                .authorization_rejections
                .fetch_add(1, Ordering::Relaxed);
            let mut event = diagnostic_event(
                started,
                HttpDiagnosticOutcome::AuthzRejected,
                Some(remote_error_status(&error)),
            );
            event.request_id = Some(request_id);
            event.operation_id = operation_id;
            event.principal_id = Some(principal_id);
            event.agent_id = agent_id;
            event.execution_id = execution_id;
            event.workspace_id = workspace_id;
            state.metrics.record_diagnostic(event);
            respond_remote_error(request, error, state.max_response_bytes);
            return;
        }
    };
    match state.dispatcher.dispatch(authorized.into_request(), now_ms) {
        Ok(response) => {
            let status = protocol_http_status(&response);
            if status >= 400 {
                mark_error(state);
            }
            state.metrics.record(HttpRequestCorrelation {
                request_id: request_id.clone(),
                operation_id: operation_id.clone(),
                principal_id: principal_id.clone(),
                agent_id: agent_id.clone(),
                execution_id: execution_id.clone(),
                workspace_id: workspace_id.clone(),
                status,
                latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            });
            let outcome = protocol_diagnostic_outcome(&response);
            let delivered = respond_protocol(request, response, state.max_response_bytes);
            let mut event = diagnostic_event(
                started,
                if delivered {
                    outcome
                } else {
                    HttpDiagnosticOutcome::UnknownOutcome
                },
                Some(status),
            );
            event.request_id = Some(request_id);
            event.operation_id = operation_id;
            event.principal_id = Some(principal_id);
            event.agent_id = agent_id;
            event.execution_id = execution_id;
            event.workspace_id = workspace_id;
            state.metrics.record_diagnostic(event);
        }
        Err(_) => {
            mark_error(state);
            let mut event =
                diagnostic_event(started, HttpDiagnosticOutcome::CoreUnavailable, Some(503));
            event.request_id = Some(request_id);
            event.operation_id = operation_id;
            event.principal_id = Some(principal_id);
            event.agent_id = agent_id;
            event.execution_id = execution_id;
            event.workspace_id = workspace_id;
            state.metrics.record_diagnostic(event);
            respond_remote_error(
                request,
                RemoteAccessError::core_unavailable(),
                state.max_response_bytes,
            )
        }
    }
}

fn diagnostic_event(
    started: Instant,
    outcome: HttpDiagnosticOutcome,
    status: Option<u16>,
) -> HttpDiagnosticEvent {
    HttpDiagnosticEvent {
        request_id: None,
        operation_id: None,
        principal_id: None,
        agent_id: None,
        execution_id: None,
        workspace_id: None,
        status,
        outcome,
        latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}

fn record_diagnostic(
    state: &HttpState,
    started: Instant,
    outcome: HttpDiagnosticOutcome,
    status: Option<u16>,
) {
    state
        .metrics
        .record_diagnostic(diagnostic_event(started, outcome, status));
}

fn record_diagnostic_for_principal(
    state: &HttpState,
    started: Instant,
    outcome: HttpDiagnosticOutcome,
    status: u16,
    principal_id: &str,
) {
    let mut event = diagnostic_event(started, outcome, Some(status));
    event.principal_id = Some(principal_id.to_owned());
    state.metrics.record_diagnostic(event);
}

fn protocol_diagnostic_outcome(response: &ProtocolResponse) -> HttpDiagnosticOutcome {
    if let Some(error) = response.error.as_ref() {
        return match error.code {
            ProtocolErrorCode::RevisionConflict => HttpDiagnosticOutcome::RevisionConflict,
            ProtocolErrorCode::LeaseConflict => HttpDiagnosticOutcome::LeaseConflict,
            ProtocolErrorCode::RecoveryRequired => HttpDiagnosticOutcome::UnknownOutcome,
            _ => HttpDiagnosticOutcome::ProtocolError,
        };
    }
    match response.result.as_ref() {
        Some(ProtocolResult::Operation(operation)) => match operation.state.as_str() {
            "running" => HttpDiagnosticOutcome::OperationAccepted,
            "completed" => HttpDiagnosticOutcome::OperationCompleted,
            "failed" | "cancelled" => HttpDiagnosticOutcome::OperationFailed,
            _ => HttpDiagnosticOutcome::RequestCompleted,
        },
        _ => HttpDiagnosticOutcome::RequestCompleted,
    }
}

fn mark_error(state: &HttpState) {
    state
        .metrics
        .error_responses
        .fetch_add(1, Ordering::Relaxed);
}

fn correlation_resources(request: &ProtocolRequest) -> (Option<String>, Option<String>) {
    let Ok(value) = serde_json::to_value(&request.call) else {
        return (None, None);
    };
    let payload = value.get("payload").unwrap_or(&value);
    let execution_id = payload
        .get("execution_id")
        .or_else(|| payload.get("from_execution_id"))
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let workspace_id = payload
        .get("workspace_id")
        .or_else(|| {
            payload
                .get("lease")
                .and_then(|lease| lease.get("workspace_id"))
        })
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    (execution_id, workspace_id)
}

struct ActiveRequest {
    metrics: Arc<HttpMetrics>,
}

impl Drop for ActiveRequest {
    fn drop(&mut self) {
        self.metrics.active_requests.fetch_sub(1, Ordering::Relaxed);
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

fn respond_protocol(
    request: Request,
    response: ProtocolResponse,
    max_response_bytes: usize,
) -> bool {
    let status = protocol_http_status(&response);
    respond_json(request, status, &response, max_response_bytes)
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

fn respond_remote_error(request: Request, error: RemoteAccessError, max_response_bytes: usize) {
    let status = remote_error_status(&error);
    respond_json(request, status, &error, max_response_bytes);
}

fn remote_error_status(error: &RemoteAccessError) -> u16 {
    use crate::RemoteAccessErrorCode as Code;
    match error.code {
        Code::AuthenticationRequired | Code::AuthenticationFailed | Code::SessionExpired => 401,
        Code::InvalidPrincipal | Code::InvalidAgentBinding => 400,
        Code::Forbidden => 403,
        Code::CoreUnavailable => 503,
    }
}

#[derive(Serialize)]
struct TransportError<'a> {
    code: &'a str,
    message: &'a str,
    retryable: bool,
}

fn respond_transport_error(
    request: Request,
    status: u16,
    code: &str,
    message: &str,
    max_response_bytes: usize,
) {
    respond_json(
        request,
        status,
        &TransportError {
            code,
            message,
            retryable: false,
        },
        max_response_bytes,
    );
}

fn respond_json<T: Serialize>(
    request: Request,
    mut status: u16,
    value: &T,
    max_response_bytes: usize,
) -> bool {
    let mut data = serde_json::to_vec(value).unwrap_or_else(|_| {
        br#"{"code":"INTERNAL_ERROR","message":"response serialization failed","retryable":false}"#.to_vec()
    });
    if data.len() > max_response_bytes {
        status = 500;
        data = RESPONSE_TOO_LARGE_BODY.to_vec();
    }
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
    request.respond(response).is_ok()
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
