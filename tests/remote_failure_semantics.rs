use pong_core::protocol::{
    OperationResource, ProtocolError, ProtocolErrorCode, ProtocolRequest, ProtocolResponse,
    ProtocolResult, ProtocolStatus,
};
use pong_core::{
    CredentialGrant, HttpDiagnosticOutcome, HttpRemoteServer, HttpServerConfig, ProtocolDispatch,
    ProtocolDispatchError, StaticCredentialVerifier, HTTP_PROTOCOL_PATH,
};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

const TOKEN_A: &str = "failure-token-a";
const TOKEN_B: &str = "failure-token-b";
const TOKEN_C: &str = "failure-token-c";

fn grant(token: &str, principal: &str, agent: &str) -> CredentialGrant {
    CredentialGrant {
        credential: token.into(),
        principal_id: principal.into(),
        agent_ids: vec![agent.into()],
        expires_at_ms: None,
    }
}

#[derive(Clone, Copy)]
enum Reply {
    InvalidProtocol,
    FailedOperation,
    RevisionConflict,
    LeaseConflict,
    CoreUnavailable,
}

struct Dispatcher(Reply);

impl ProtocolDispatch for Dispatcher {
    fn dispatch(
        &self,
        request: ProtocolRequest,
        _now_ms: i64,
    ) -> Result<ProtocolResponse, ProtocolDispatchError> {
        let response = match self.0 {
            Reply::InvalidProtocol => ProtocolResponse {
                protocol_version: "1.0".into(),
                request_id: request.request_id,
                operation_id: request.operation_id,
                status: ProtocolStatus::Error,
                result: None,
                error: Some(protocol_error(ProtocolErrorCode::ValidationError)),
            },
            Reply::FailedOperation => ProtocolResponse {
                protocol_version: "1.0".into(),
                request_id: request.request_id.clone(),
                operation_id: request.operation_id.clone(),
                status: ProtocolStatus::Ok,
                result: Some(ProtocolResult::Operation(OperationResource {
                    operation_id: request.operation_id.unwrap(),
                    execution_id: Some("execution-failure".into()),
                    request_id: request.request_id,
                    agent_id: "agent-a".into(),
                    workspace_id: Some("workspace-failure".into()),
                    action: "fail safely".into(),
                    state: "failed".into(),
                    recording_state: "RECORDED".into(),
                    failure_code: Some("EXPECTED_FAILURE".into()),
                    started_at: "2026-09-20T00:00:00Z".into(),
                    finished_at: Some("2026-09-20T00:00:01Z".into()),
                })),
                error: None,
            },
            Reply::RevisionConflict => {
                protocol_failure(request, ProtocolErrorCode::RevisionConflict)
            }
            Reply::LeaseConflict => protocol_failure(request, ProtocolErrorCode::LeaseConflict),
            Reply::CoreUnavailable => return Err(ProtocolDispatchError),
        };
        Ok(response)
    }
}

fn protocol_error(code: ProtocolErrorCode) -> ProtocolError {
    ProtocolError {
        code,
        message: "safe protocol failure".into(),
        retryable: false,
        details: None,
    }
}

fn protocol_failure(request: ProtocolRequest, code: ProtocolErrorCode) -> ProtocolResponse {
    ProtocolResponse {
        protocol_version: "1.0".into(),
        request_id: request.request_id,
        operation_id: request.operation_id,
        status: ProtocolStatus::Error,
        result: None,
        error: Some(protocol_error(code)),
    }
}

fn server(reply: Reply, config: HttpServerConfig) -> HttpRemoteServer {
    HttpRemoteServer::start(
        config,
        Arc::new(
            StaticCredentialVerifier::new(vec![
                grant(TOKEN_A, "principal-a", "agent-a"),
                grant(TOKEN_B, "principal-b", "agent-b"),
                grant(TOKEN_C, "principal-c", "agent-c"),
            ])
            .unwrap(),
        ),
        Arc::new(Dispatcher(reply)),
    )
    .unwrap()
}

fn protocol_request(request_id: &str, operation_id: Option<&str>, agent: Option<&str>) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "protocol_version": "1.0",
        "request_id": request_id,
        "operation_id": operation_id,
        "caller_agent_id": agent,
        "issued_at": "2026-09-20T00:00:00Z",
        "operation": "hello"
    }))
    .unwrap()
}

fn authorized_request(request_id: &str, operation_id: Option<&str>) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "protocol_version": "1.0",
        "request_id": request_id,
        "operation_id": operation_id,
        "caller_agent_id": "agent-a",
        "issued_at": "2026-09-20T00:00:00Z",
        "operation": "get_operation",
        "payload": {"id": operation_id.unwrap_or("operation-a")}
    }))
    .unwrap()
}

fn request(addr: SocketAddr, token: &str, body: &[u8]) -> (u16, Value) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write!(
        stream,
        "POST {HTTP_PROTOCOL_PATH} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let split = bytes
        .windows(4)
        .position(|item| item == b"\r\n\r\n")
        .unwrap();
    let status = String::from_utf8_lossy(&bytes[..split])
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let body = serde_json::from_slice(&bytes[split + 4..]).unwrap();
    (status, body)
}

#[test]
fn operation_failure_is_protocol_success_not_http_500() {
    let server = server(Reply::FailedOperation, HttpServerConfig::default());
    let (status, response) = request(
        server.listen_addr(),
        TOKEN_A,
        &authorized_request("operation-failed", Some("operation-failed")),
    );
    assert_eq!(status, 200);
    assert_eq!(response["status"], "ok");
    assert_eq!(response["result"]["data"]["state"], "failed");
    assert_eq!(
        server.metrics().recent_diagnostics.last().unwrap().outcome,
        HttpDiagnosticOutcome::OperationFailed
    );
    server.shutdown().unwrap();
}

#[test]
fn transport_protocol_and_core_failures_remain_distinct() {
    let malformed = server(Reply::InvalidProtocol, HttpServerConfig::default());
    assert_eq!(request(malformed.listen_addr(), TOKEN_A, b"{").0, 400);
    assert_eq!(
        malformed
            .metrics()
            .recent_diagnostics
            .last()
            .unwrap()
            .outcome,
        HttpDiagnosticOutcome::MalformedRequest
    );
    malformed.shutdown().unwrap();

    let protocol = server(Reply::InvalidProtocol, HttpServerConfig::default());
    assert_eq!(
        request(
            protocol.listen_addr(),
            TOKEN_A,
            &protocol_request("protocol-error", None, None)
        )
        .0,
        400
    );
    assert_eq!(
        protocol
            .metrics()
            .recent_diagnostics
            .last()
            .unwrap()
            .outcome,
        HttpDiagnosticOutcome::ProtocolError
    );
    protocol.shutdown().unwrap();

    let unavailable = server(Reply::CoreUnavailable, HttpServerConfig::default());
    assert_eq!(
        request(
            unavailable.listen_addr(),
            TOKEN_A,
            &protocol_request("core-unavailable", None, None)
        )
        .0,
        503
    );
    assert_eq!(
        unavailable
            .metrics()
            .recent_diagnostics
            .last()
            .unwrap()
            .outcome,
        HttpDiagnosticOutcome::CoreUnavailable
    );
    unavailable.shutdown().unwrap();
}

#[test]
fn revision_and_lease_conflicts_have_stable_diagnostic_categories() {
    for (reply, expected) in [
        (
            Reply::RevisionConflict,
            HttpDiagnosticOutcome::RevisionConflict,
        ),
        (Reply::LeaseConflict, HttpDiagnosticOutcome::LeaseConflict),
    ] {
        let server = server(reply, HttpServerConfig::default());
        assert_eq!(
            request(
                server.listen_addr(),
                TOKEN_A,
                &protocol_request("conflict", None, None)
            )
            .0,
            409
        );
        assert_eq!(
            server.metrics().recent_diagnostics.last().unwrap().outcome,
            expected
        );
        server.shutdown().unwrap();
    }
}

#[test]
fn authentication_and_authorization_rejections_are_recoverable_and_secret_free() {
    let server = server(Reply::InvalidProtocol, HttpServerConfig::default());
    assert_eq!(
        request(
            server.listen_addr(),
            "invalid-secret-token",
            &protocol_request("bad-auth", None, None)
        )
        .0,
        401
    );
    assert_eq!(
        request(
            server.listen_addr(),
            TOKEN_A,
            &authorized_request("bad-authz", Some("operation-authz"))
        )
        .0,
        400
    );
    let foreign = serde_json::to_vec(&json!({
        "protocol_version": "1.0",
        "request_id": "foreign-agent",
        "caller_agent_id": "agent-b",
        "issued_at": "2026-09-20T00:00:00Z",
        "operation": "get_operation",
        "payload": {"id": "operation-a"}
    }))
    .unwrap();
    assert_eq!(request(server.listen_addr(), TOKEN_A, &foreign).0, 403);
    assert_eq!(
        request(
            server.listen_addr(),
            TOKEN_A,
            &protocol_request("after-rejections", None, None)
        )
        .0,
        400
    );
    let metrics = serde_json::to_string(&server.metrics()).unwrap();
    assert!(!metrics.contains(TOKEN_A));
    assert!(!metrics.contains("invalid-secret-token"));
    assert!(!metrics.contains("Authorization"));
    assert!(metrics.contains("AUTH_REJECTED"));
    assert!(metrics.contains("AUTHZ_REJECTED"));
    server.shutdown().unwrap();
}

#[test]
fn runtime_diagnostic_and_rate_limiter_state_are_bounded() {
    let server = server(
        Reply::InvalidProtocol,
        HttpServerConfig {
            rate_limit_requests: 100,
            rate_limit_max_principals: 2,
            ..HttpServerConfig::default()
        },
    );
    for index in 0..70 {
        let token = if index % 2 == 0 { TOKEN_A } else { TOKEN_B };
        let id = format!("bounded-{index}");
        assert_eq!(
            request(
                server.listen_addr(),
                token,
                &protocol_request(&id, None, None)
            )
            .0,
            400
        );
    }
    assert_eq!(
        request(
            server.listen_addr(),
            TOKEN_C,
            &protocol_request("capacity", None, None)
        )
        .0,
        429
    );
    let metrics = server.metrics();
    assert_eq!(metrics.recent_requests.len(), 64);
    assert_eq!(metrics.recent_diagnostics.len(), 64);
    assert_eq!(metrics.tracked_rate_limit_principals, 2);
    assert_eq!(metrics.peak_tracked_rate_limit_principals, 2);
    assert_eq!(metrics.rate_limit_capacity_rejections, 1);
    assert_eq!(
        metrics.recent_diagnostics.last().unwrap().outcome,
        HttpDiagnosticOutcome::RateLimited
    );
    server.shutdown().unwrap();
}

#[test]
fn graceful_shutdown_records_a_final_transport_event() {
    let server = server(Reply::InvalidProtocol, HttpServerConfig::default());
    let metrics = server.shutdown_with_metrics().unwrap();
    assert_eq!(
        metrics.recent_diagnostics.last().unwrap().outcome,
        HttpDiagnosticOutcome::ServerShutdown
    );
    assert_eq!(metrics.active_requests, 0);
}
