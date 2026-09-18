use pong_core::{
    protocol::{ProtocolRequest, ProtocolResponse},
    CredentialGrant, CredentialVerifier, HttpRemoteServer, HttpServerConfig, ProtocolDispatch,
    ProtocolDispatchError, Repository, StaticCredentialVerifier, HTTP_PROTOCOL_PATH,
    MIN_HTTP_MAX_RESPONSE_BYTES,
};
use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tempfile::tempdir;

const TOKEN_A: &str = "hardening-a";
const TOKEN_B: &str = "hardening-b";

fn grant(token: &str, principal: &str, agent: &str) -> CredentialGrant {
    CredentialGrant {
        credential: token.into(),
        principal_id: principal.into(),
        agent_ids: vec![agent.into()],
        expires_at_ms: None,
    }
}

struct Dispatcher {
    panic_once: AtomicBool,
}

impl ProtocolDispatch for Dispatcher {
    fn dispatch(
        &self,
        request: ProtocolRequest,
        _now_ms: i64,
    ) -> Result<ProtocolResponse, ProtocolDispatchError> {
        if self.panic_once.swap(false, Ordering::SeqCst) {
            panic!("test handler panic");
        }
        Ok(ProtocolResponse::invalid_envelope(Some(request.request_id)))
    }
}

fn request(addr: SocketAddr, token: &str, body: &[u8]) -> (u16, Vec<u8>) {
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
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    let headers = String::from_utf8_lossy(&bytes[..split]);
    let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, bytes[split + 4..].to_vec())
}

fn hello(id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "protocol_version": "1.0",
        "request_id": id,
        "caller_agent_id": null,
        "issued_at": "2026-01-01T00:00:00Z",
        "operation": "hello"
    }))
    .unwrap()
}

fn server(
    config: HttpServerConfig,
    verifier: Arc<dyn CredentialVerifier>,
    dispatcher: Arc<Dispatcher>,
) -> HttpRemoteServer {
    HttpRemoteServer::start(config, verifier, dispatcher).unwrap()
}

#[test]
fn credentials_support_rotation_and_revocation() {
    let verifier = StaticCredentialVerifier::new(vec![grant(TOKEN_A, "p-a", "a")]).unwrap();
    assert!(verifier.authenticate(TOKEN_A, 0).is_ok());
    verifier.add_grant(grant(TOKEN_B, "p-b", "b")).unwrap();
    assert!(verifier.authenticate(TOKEN_B, 0).is_ok());
    assert!(verifier.revoke(TOKEN_A).unwrap());
    assert!(verifier.authenticate(TOKEN_A, 0).is_err());
    verifier
        .replace_grants(vec![grant(TOKEN_A, "p-a2", "a2")])
        .unwrap();
    assert!(verifier.authenticate(TOKEN_B, 0).is_err());
    assert_eq!(
        verifier.authenticate(TOKEN_A, 0).unwrap().principal_id,
        "p-a2"
    );
}

#[test]
fn duplicate_rotation_is_atomic() {
    let verifier = StaticCredentialVerifier::new(vec![grant(TOKEN_A, "p-a", "a")]).unwrap();
    assert!(verifier
        .replace_grants(vec![grant(TOKEN_B, "p-b", "b"), grant(TOKEN_B, "p-c", "c"),])
        .is_err());
    assert!(verifier.authenticate(TOKEN_A, 0).is_ok());
    assert!(verifier.authenticate(TOKEN_B, 0).is_err());
}

#[test]
fn rate_limit_and_metrics_are_in_memory_and_provider_neutral() {
    let config = HttpServerConfig {
        rate_limit_requests: 1,
        rate_limit_window_ms: 60_000,
        ..HttpServerConfig::default()
    };
    let server = server(
        config,
        Arc::new(StaticCredentialVerifier::new(vec![grant(TOKEN_A, "p-a", "a")]).unwrap()),
        Arc::new(Dispatcher {
            panic_once: AtomicBool::new(false),
        }),
    );
    let addr = server.listen_addr();
    assert_eq!(request(addr, TOKEN_A, &hello("one")).0, 400);
    assert_eq!(request(addr, TOKEN_A, &hello("two")).0, 429);
    let metrics = server.metrics();
    assert_eq!(metrics.requests_total, 2);
    assert_eq!(metrics.error_responses, 2);
    assert_eq!(metrics.rate_limit_rejections, 1);
    assert_eq!(metrics.recent_requests.len(), 1);
    assert_eq!(metrics.recent_requests[0].request_id, "one");
    assert_eq!(metrics.recent_requests[0].principal_id, "p-a");
    assert_eq!(metrics.active_requests, 0);
    server.shutdown().unwrap();
}

#[test]
fn rate_limit_is_per_principal() {
    let config = HttpServerConfig {
        rate_limit_requests: 1,
        rate_limit_window_ms: 60_000,
        ..HttpServerConfig::default()
    };
    let server = server(
        config,
        Arc::new(
            StaticCredentialVerifier::new(vec![
                grant(TOKEN_A, "p-a", "a"),
                grant(TOKEN_B, "p-b", "b"),
            ])
            .unwrap(),
        ),
        Arc::new(Dispatcher {
            panic_once: AtomicBool::new(false),
        }),
    );
    assert_eq!(request(server.listen_addr(), TOKEN_A, &hello("a")).0, 400);
    assert_eq!(request(server.listen_addr(), TOKEN_B, &hello("b")).0, 400);
    server.shutdown().unwrap();
}

#[test]
fn panic_isolated_and_next_client_is_served() {
    let server = server(
        HttpServerConfig::default(),
        Arc::new(StaticCredentialVerifier::new(vec![grant(TOKEN_A, "p-a", "a")]).unwrap()),
        Arc::new(Dispatcher {
            panic_once: AtomicBool::new(true),
        }),
    );
    let addr = server.listen_addr();
    assert_eq!(request(addr, TOKEN_A, &hello("panic")).0, 500);
    assert_eq!(request(addr, TOKEN_A, &hello("after")).0, 400);
    let metrics = server.metrics();
    assert_eq!(metrics.handler_panics, 1);
    assert_eq!(metrics.error_responses, 2);
    server.shutdown().unwrap();
}

#[test]
fn configured_response_limit_is_a_hard_limit() {
    let config = HttpServerConfig {
        max_response_bytes: MIN_HTTP_MAX_RESPONSE_BYTES,
        ..HttpServerConfig::default()
    };
    let server = server(
        config,
        Arc::new(StaticCredentialVerifier::new(vec![grant(TOKEN_A, "p-a", "a")]).unwrap()),
        Arc::new(Dispatcher {
            panic_once: AtomicBool::new(false),
        }),
    );
    let long_id = "x".repeat(1024);
    let (status, body) = request(server.listen_addr(), TOKEN_A, &hello(&long_id));
    assert_eq!(status, 500);
    assert!(body.len() <= MIN_HTTP_MAX_RESPONSE_BYTES);
    assert!(String::from_utf8_lossy(&body).contains("RESPONSE_TOO_LARGE"));
    server.shutdown().unwrap();
}

#[test]
fn malformed_request_error_does_not_leak_internals_or_secret() {
    let server = server(
        HttpServerConfig::default(),
        Arc::new(StaticCredentialVerifier::new(vec![grant(TOKEN_A, "p-a", "a")]).unwrap()),
        Arc::new(Dispatcher {
            panic_once: AtomicBool::new(false),
        }),
    );
    let (status, body) = request(server.listen_addr(), TOKEN_A, b"not-json");
    assert_eq!(status, 400);
    let body = String::from_utf8_lossy(&body).to_ascii_lowercase();
    for forbidden in ["sqlite", "filesystem", "rustc", "panic", TOKEN_A] {
        assert!(!body.contains(&forbidden.to_ascii_lowercase()));
    }
    server.shutdown().unwrap();
}

#[test]
fn invalid_resource_configuration_fails_closed() {
    let result = HttpRemoteServer::start(
        HttpServerConfig {
            worker_threads: 0,
            ..HttpServerConfig::default()
        },
        Arc::new(StaticCredentialVerifier::new(vec![grant(TOKEN_A, "p-a", "a")]).unwrap()),
        Arc::new(Dispatcher {
            panic_once: AtomicBool::new(false),
        }),
    );
    assert!(result.is_err());
}

#[test]
fn explicit_non_loopback_binding_requires_and_honors_opt_in() {
    let verifier =
        Arc::new(StaticCredentialVerifier::new(vec![grant(TOKEN_A, "p-a", "a")]).unwrap());
    let dispatcher = Arc::new(Dispatcher {
        panic_once: AtomicBool::new(false),
    });
    let server = server(
        HttpServerConfig {
            listen_addr: "0.0.0.0:0".parse().unwrap(),
            allow_non_loopback: true,
            ..HttpServerConfig::default()
        },
        verifier,
        dispatcher,
    );
    assert_eq!(server.listen_addr().ip().to_string(), "0.0.0.0");
    server.shutdown().unwrap();
}

fn init_process_repository(root: &Path) {
    let mut repository = Repository::init(root).unwrap();
    repository
        .metadata_mut()
        .record_environment(
            "http-hardening-environment",
            "http-hardening-project",
            &json!({"schema_version": 1}),
            "t0",
        )
        .unwrap();
}

fn write_credentials(path: &Path, entries: &[(&str, &str, &str)]) {
    let credentials = entries
        .iter()
        .map(|(credential, principal_id, agent_id)| {
            json!({
                "credential": credential,
                "principal_id": principal_id,
                "agent_ids": [agent_id],
                "expires_at_ms": null
            })
        })
        .collect::<Vec<_>>();
    fs::write(
        path,
        serde_json::to_vec(&json!({"credentials": credentials})).unwrap(),
    )
    .unwrap();
}

struct HttpProcess {
    child: Child,
    stdin: ChildStdin,
    addr: SocketAddr,
    stdout: BufReader<std::process::ChildStdout>,
}

impl HttpProcess {
    fn start(repository: &Path, workspace: &Path, credentials: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-http"))
            .arg("--repository")
            .arg(repository)
            .arg("--workspace-root")
            .arg(workspace)
            .arg("--credentials-file")
            .arg(credentials)
            .arg("--listen")
            .arg("127.0.0.1:0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let ready: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(ready["status"], "ready");
        let addr = ready["listen_addr"].as_str().unwrap().parse().unwrap();
        Self {
            child,
            stdin,
            addr,
            stdout,
        }
    }

    fn reload(&mut self) {
        self.stdin.write_all(b"reload-credentials\n").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let status: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(status["status"], "credentials_reloaded");
    }

    fn shutdown(mut self) {
        self.stdin.write_all(b"shutdown\n").unwrap();
        self.stdin.flush().unwrap();
        assert!(self.child.wait().unwrap().success());
    }
}

fn process_hello(addr: SocketAddr, token: &str, request_id: &str) -> u16 {
    let body = serde_json::to_vec(&json!({
        "protocol_version": "1.0",
        "request_id": request_id,
        "caller_agent_id": null,
        "issued_at": "2026-01-01T00:00:00Z",
        "operation": "hello"
    }))
    .unwrap();
    request(addr, token, &body).0
}

#[test]
fn process_credential_reload_rotates_and_revokes_without_touching_core_state() {
    let repository = tempdir().unwrap();
    let workspace = tempdir().unwrap();
    let credentials_dir = tempdir().unwrap();
    init_process_repository(repository.path());
    let credentials = credentials_dir.path().join("credentials.json");
    write_credentials(&credentials, &[(TOKEN_A, "principal-a", "agent-a")]);
    let mut process = HttpProcess::start(repository.path(), workspace.path(), &credentials);

    assert_eq!(
        process_hello(process.addr, TOKEN_A, "rotation-a-before"),
        200
    );
    write_credentials(
        &credentials,
        &[
            (TOKEN_A, "principal-a", "agent-a"),
            (TOKEN_B, "principal-b", "agent-b"),
        ],
    );
    process.reload();
    assert_eq!(process_hello(process.addr, TOKEN_B, "rotation-b"), 200);

    write_credentials(&credentials, &[(TOKEN_B, "principal-b", "agent-b")]);
    process.reload();
    assert_eq!(
        process_hello(process.addr, TOKEN_A, "rotation-a-after"),
        401
    );
    assert_eq!(
        process_hello(process.addr, TOKEN_B, "rotation-b-after"),
        200
    );
    assert!(Repository::open(repository.path()).is_err());

    process.shutdown();
    Repository::open(repository.path()).expect("reload must not disturb Core ownership");
}
