//! M4-013 real HTTP transport contract for External Agent Protocol v1.0.

use pong_core::protocol::{WorkspaceBindingError, WorkspaceBindingResolver};
use pong_core::{
    AgentProtocolCore, CredentialGrant, HttpRemoteServer, HttpServerConfig, ProtocolDispatch,
    ProtocolDispatchError, Repository, StaticCredentialVerifier, HTTP_PROTOCOL_PATH,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "http-project";
const ENVIRONMENT: &str = "http-environment";
const AGENT_A: &str = "http-agent-a";
const AGENT_B: &str = "http-agent-b";
const TOKEN_A: &str = "opaque-http-credential-a";
const TOKEN_B: &str = "opaque-http-credential-b";
const TOKEN_EXPIRED: &str = "opaque-http-credential-expired";
const TASK: &str = "http-task";
const W1: &str = "http-workspace-a";
const E1: &str = "http-execution-a";

struct Bindings {
    root: PathBuf,
}

impl WorkspaceBindingResolver for Bindings {
    fn resolve(&self, binding_ref: &str) -> Result<PathBuf, WorkspaceBindingError> {
        if binding_ref.is_empty()
            || binding_ref == "."
            || binding_ref == ".."
            || !binding_ref
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(WorkspaceBindingError::Invalid);
        }
        Ok(self.root.join(binding_ref))
    }
}

fn grants() -> Vec<CredentialGrant> {
    vec![
        CredentialGrant {
            credential: TOKEN_A.into(),
            principal_id: "principal-a".into(),
            agent_ids: vec![AGENT_A.into()],
            expires_at_ms: None,
        },
        CredentialGrant {
            credential: TOKEN_B.into(),
            principal_id: "principal-b".into(),
            agent_ids: vec![AGENT_B.into()],
            expires_at_ms: None,
        },
        CredentialGrant {
            credential: TOKEN_EXPIRED.into(),
            principal_id: "principal-expired".into(),
            agent_ids: vec!["expired-agent".into()],
            expires_at_ms: Some(0),
        },
    ]
}

fn init_repository(root: &Path) {
    let mut repository = Repository::init(root).unwrap();
    repository
        .metadata_mut()
        .record_environment(
            ENVIRONMENT,
            PROJECT,
            &json!({"schema_version": 1, "transport": "http"}),
            "t0",
        )
        .unwrap();
}

fn start_server(repository_root: &Path, workspace_root: &Path) -> HttpRemoteServer {
    let repository = Repository::open_as_core_owner(repository_root).unwrap();
    let dispatcher = Arc::new(AgentProtocolCore::new(
        repository,
        Bindings {
            root: workspace_root.to_path_buf(),
        },
    ));
    let verifier = Arc::new(StaticCredentialVerifier::new(grants()).unwrap());
    HttpRemoteServer::start(HttpServerConfig::default(), verifier, dispatcher).unwrap()
}

struct Fixture {
    server: Option<HttpRemoteServer>,
    repository_dir: TempDir,
    workspace_dir: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let repository_dir = tempdir().unwrap();
        let workspace_dir = tempdir().unwrap();
        init_repository(repository_dir.path());
        let server = start_server(repository_dir.path(), workspace_dir.path());
        Self {
            server: Some(server),
            repository_dir,
            workspace_dir,
        }
    }

    fn addr(&self) -> SocketAddr {
        self.server.as_ref().unwrap().listen_addr()
    }

    fn restart(&mut self) {
        self.server.take().unwrap().shutdown().unwrap();
        self.server = Some(start_server(
            self.repository_dir.path(),
            self.workspace_dir.path(),
        ));
    }
}

struct HttpResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: Value,
}

fn http_raw(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: Option<&str>,
    content_type: Option<&str>,
    body: &[u8],
) -> HttpResponse {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    )
    .unwrap();
    if let Some(content_type) = content_type {
        write!(stream, "Content-Type: {content_type}\r\n").unwrap();
    }
    if let Some(token) = token {
        write!(stream, "Authorization: Bearer {token}\r\n").unwrap();
    }
    stream.write_all(b"\r\n").unwrap();
    stream.write_all(body).unwrap();
    stream.flush().unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    parse_http_response(&bytes)
}

fn parse_http_response(bytes: &[u8]) -> HttpResponse {
    let separator = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("HTTP response headers");
    let head = std::str::from_utf8(&bytes[..separator]).unwrap();
    let mut lines = head.lines();
    let status = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    let body = serde_json::from_slice(&bytes[separator + 4..]).unwrap();
    HttpResponse {
        status,
        headers,
        body,
    }
}

fn envelope(
    caller: Option<&str>,
    request_id: &str,
    operation_id: Option<&str>,
    operation: &str,
    payload: Option<Value>,
) -> Value {
    let mut request = json!({
        "protocol_version": "1.0",
        "request_id": request_id,
        "caller_agent_id": caller,
        "issued_at": format!("time:{request_id}"),
        "operation_id": operation_id,
        "operation": operation
    });
    if let Some(payload) = payload {
        request["payload"] = payload;
    }
    request
}

fn call(addr: SocketAddr, token: &str, request: &Value) -> HttpResponse {
    http_raw(
        addr,
        "POST",
        HTTP_PROTOCOL_PATH,
        Some(token),
        Some("application/json"),
        request.to_string().as_bytes(),
    )
}

fn ok(response: &HttpResponse, kind: &str) -> Value {
    assert_eq!(response.status, 200, "{:#}", response.body);
    assert_eq!(response.body["status"], "ok", "{:#}", response.body);
    assert_eq!(response.body["result"]["kind"], kind);
    response.body["result"]["data"].clone()
}

fn register(addr: SocketAddr, token: &str, agent: &str) {
    ok(
        &call(
            addr,
            token,
            &envelope(
                Some(agent),
                &format!("register:{agent}"),
                None,
                "register_agent",
                Some(json!({
                    "agent_id": agent,
                    "provider_metadata": "runtime-neutral",
                    "display_name": null
                })),
            ),
        ),
        "agent",
    );
}

fn bootstrap_a(fixture: &Fixture) {
    let addr = fixture.addr();
    register(addr, TOKEN_A, AGENT_A);
    ok(
        &call(
            addr,
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "create-task",
                None,
                "create_task",
                Some(json!({
                    "task_id": TASK,
                    "project_id": PROJECT,
                    "goal_ref": "goal:http",
                    "context_ref": null
                })),
            ),
        ),
        "task",
    );
    ok(
        &call(
            addr,
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "create-w1",
                None,
                "create_workspace",
                Some(json!({
                    "workspace_id": W1,
                    "project_id": PROJECT,
                    "binding_ref": "runtime-a",
                    "branch_ref": null,
                    "environment_id": ENVIRONMENT
                })),
            ),
        ),
        "workspace",
    );
    ok(
        &call(
            addr,
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "create-e1",
                None,
                "create_execution",
                Some(json!({
                    "execution_id": E1,
                    "task_id": TASK,
                    "parent_execution_id": null,
                    "workspace_id": W1,
                    "base_version_id": null
                })),
            ),
        ),
        "execution",
    );
    ok(
        &call(
            addr,
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "start-e1",
                None,
                "start_execution",
                Some(json!({"execution_id": E1, "expected_revision": 0})),
            ),
        ),
        "execution",
    );
}

#[test]
fn server_starts_local_only_and_serves_protocol_without_cors() {
    let fixture = Fixture::new();
    assert!(fixture.addr().ip().is_loopback());
    let response = call(
        fixture.addr(),
        TOKEN_A,
        &envelope(None, "hello", None, "hello", None),
    );
    let hello = ok(&response, "hello");
    assert_eq!(hello["protocol_versions"], json!(["1.0"]));
    assert_eq!(response.headers["cache-control"], "no-store");
    assert_eq!(response.headers["x-content-type-options"], "nosniff");
    assert!(!response.headers.contains_key("access-control-allow-origin"));
}

#[test]
fn authentication_is_required_invalid_and_expired_credentials_fail_closed() {
    let fixture = Fixture::new();
    let body = envelope(None, "hello", None, "hello", None).to_string();
    let missing = http_raw(
        fixture.addr(),
        "POST",
        HTTP_PROTOCOL_PATH,
        None,
        Some("application/json"),
        body.as_bytes(),
    );
    assert_eq!(missing.status, 401);
    assert_eq!(missing.body["code"], "AUTHENTICATION_REQUIRED");
    for token in ["invalid-credential", TOKEN_EXPIRED] {
        let rejected = http_raw(
            fixture.addr(),
            "POST",
            HTTP_PROTOCOL_PATH,
            Some(token),
            Some("application/json"),
            body.as_bytes(),
        );
        assert_eq!(rejected.status, 401);
        assert_eq!(rejected.body["code"], "AUTHENTICATION_FAILED");
        assert!(!rejected.body.to_string().contains(token));
    }
}

#[test]
fn principal_agent_binding_precedes_protocol_dispatch() {
    let fixture = Fixture::new();
    let response = call(
        fixture.addr(),
        TOKEN_A,
        &envelope(
            Some(AGENT_B),
            "forbidden-agent",
            None,
            "register_agent",
            Some(json!({
                "agent_id": AGENT_B,
                "provider_metadata": null,
                "display_name": null
            })),
        ),
    );
    assert_eq!(response.status, 403);
    assert_eq!(response.body["code"], "FORBIDDEN");
}

#[test]
fn malformed_version_method_path_media_and_body_limit_are_transport_safe() {
    let fixture = Fixture::new();
    let malformed = http_raw(
        fixture.addr(),
        "POST",
        HTTP_PROTOCOL_PATH,
        Some(TOKEN_A),
        Some("application/json"),
        br#"{"request_id":"bad","broken":true}"#,
    );
    assert_eq!(malformed.status, 400);
    assert_eq!(malformed.body["error"]["code"], "VALIDATION_ERROR");
    let mut version = envelope(None, "old-version", None, "hello", None);
    version["protocol_version"] = json!("0.9");
    let version = call(fixture.addr(), TOKEN_A, &version);
    assert_eq!(version.status, 400);
    assert_eq!(version.body["error"]["code"], "UNSUPPORTED_VERSION");
    let method = http_raw(fixture.addr(), "GET", HTTP_PROTOCOL_PATH, None, None, b"");
    assert_eq!(method.status, 405);
    let path = http_raw(fixture.addr(), "POST", "/other", None, None, b"");
    assert_eq!(path.status, 404);
    let media = http_raw(
        fixture.addr(),
        "POST",
        HTTP_PROTOCOL_PATH,
        Some(TOKEN_A),
        Some("text/plain"),
        b"{}",
    );
    assert_eq!(media.status, 415);

    let repository_dir = tempdir().unwrap();
    let workspace_dir = tempdir().unwrap();
    init_repository(repository_dir.path());
    let repository = Repository::open_as_core_owner(repository_dir.path()).unwrap();
    let dispatcher = Arc::new(AgentProtocolCore::new(
        repository,
        Bindings {
            root: workspace_dir.path().into(),
        },
    ));
    let limited = HttpRemoteServer::start(
        HttpServerConfig {
            max_body_bytes: 8,
            ..HttpServerConfig::default()
        },
        Arc::new(StaticCredentialVerifier::new(grants()).unwrap()),
        dispatcher,
    )
    .unwrap();
    let oversized = http_raw(
        limited.listen_addr(),
        "POST",
        HTTP_PROTOCOL_PATH,
        Some(TOKEN_A),
        Some("application/json"),
        b"123456789",
    );
    assert_eq!(oversized.status, 413);
    assert_eq!(oversized.body["code"], "PAYLOAD_TOO_LARGE");
}

#[test]
fn duplicate_http_retry_replays_one_durable_operation() {
    let fixture = Fixture::new();
    bootstrap_a(&fixture);
    let request = envelope(
        Some(AGENT_A),
        "request-retry",
        Some("operation-retry"),
        "start_operation",
        Some(json!({"execution_id": E1, "action": "http.work"})),
    );
    let first = call(fixture.addr(), TOKEN_A, &request);
    let second = call(fixture.addr(), TOKEN_A, &request);
    assert_eq!(first.body, second.body);
    let operation = ok(&second, "operation");
    assert_eq!(operation["operation_id"], "operation-retry");
    assert_eq!(operation["state"], "started");
}

#[test]
fn lost_http_response_is_resolved_after_new_connection() {
    let fixture = Fixture::new();
    bootstrap_a(&fixture);
    let request = envelope(
        Some(AGENT_A),
        "request-uncertain-http",
        Some("operation-uncertain-http"),
        "start_operation",
        Some(json!({"execution_id": E1, "action": "http.uncertain"})),
    );
    let body = request.to_string();
    let mut stream = TcpStream::connect(fixture.addr()).unwrap();
    write!(
        stream,
        "POST {HTTP_PROTOCOL_PATH} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nAuthorization: Bearer {TOKEN_A}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        fixture.addr(),
        body.len()
    )
    .unwrap();
    stream.flush().unwrap();
    drop(stream);

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let resolved = call(
            fixture.addr(),
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "resolve-uncertain-http",
                None,
                "resolve_operation",
                Some(json!({
                    "project_id": PROJECT,
                    "request_id": "request-uncertain-http"
                })),
            ),
        );
        if resolved.status == 200 {
            assert_eq!(
                ok(&resolved, "operation")["operation_id"],
                "operation-uncertain-http"
            );
            break;
        }
        assert_eq!(resolved.body["error"]["code"], "NOT_FOUND");
        assert!(
            Instant::now() < deadline,
            "operation did not become durable"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn core_restart_preserves_operation_and_rejects_second_core() {
    let mut fixture = Fixture::new();
    bootstrap_a(&fixture);
    ok(
        &call(
            fixture.addr(),
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "request-restart-http",
                Some("operation-restart-http"),
                "start_operation",
                Some(json!({"execution_id": E1, "action": "http.restart"})),
            ),
        ),
        "operation",
    );
    assert!(Repository::open(fixture.repository_dir.path()).is_err());
    fixture.restart();
    let operation = ok(
        &call(
            fixture.addr(),
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "get-after-http-restart",
                None,
                "get_operation",
                Some(json!({"id": "operation-restart-http"})),
            ),
        ),
        "operation",
    );
    assert_eq!(operation["state"], "started");
}

#[test]
fn protocol_ownership_revision_and_lease_errors_keep_wire_codes() {
    let fixture = Fixture::new();
    bootstrap_a(&fixture);
    register(fixture.addr(), TOKEN_B, AGENT_B);
    let foreign = call(
        fixture.addr(),
        TOKEN_B,
        &envelope(
            Some(AGENT_B),
            "foreign-execution",
            None,
            "get_execution",
            Some(json!({"id": E1})),
        ),
    );
    assert_eq!(foreign.status, 403);
    assert_eq!(foreign.body["error"]["code"], "FORBIDDEN");
    let foreign_workspace = call(
        fixture.addr(),
        TOKEN_B,
        &envelope(
            Some(AGENT_B),
            "foreign-workspace",
            None,
            "get_workspace",
            Some(json!({"id": W1})),
        ),
    );
    assert_eq!(foreign_workspace.status, 200);
    assert_eq!(
        foreign_workspace.body["result"]["kind"],
        "workspace_inspection"
    );
    let stale = call(
        fixture.addr(),
        TOKEN_A,
        &envelope(
            Some(AGENT_A),
            "stale-http",
            None,
            "pause_execution",
            Some(json!({
                "execution_id": E1,
                "expected_revision": 0,
                "outcome_code": null
            })),
        ),
    );
    assert_eq!(stale.status, 409);
    assert_eq!(stale.body["error"]["code"], "REVISION_CONFLICT");
    let lease = ok(
        &call(
            fixture.addr(),
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "lease-http",
                None,
                "acquire_workspace_lease",
                Some(json!({
                    "execution_id": E1,
                    "workspace_id": W1,
                    "ttl_ms": 60000
                })),
            ),
        ),
        "lease",
    );
    let mut invalid_authority = lease["authority"].clone();
    invalid_authority["epoch"] = json!(invalid_authority["epoch"].as_i64().unwrap() + 1);
    let conflict = call(
        fixture.addr(),
        TOKEN_A,
        &envelope(
            Some(AGENT_A),
            "lease-conflict-http",
            None,
            "release_workspace_lease",
            Some(json!({"lease": invalid_authority})),
        ),
    );
    assert_eq!(conflict.status, 409);
    assert_eq!(conflict.body["error"]["code"], "LEASE_CONFLICT");
}

#[test]
fn same_workspace_clients_are_coordinated_by_existing_lease() {
    let fixture = Fixture::new();
    bootstrap_a(&fixture);
    register(fixture.addr(), TOKEN_B, AGENT_B);
    let lease_a = ok(
        &call(
            fixture.addr(),
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "lease-a",
                None,
                "acquire_workspace_lease",
                Some(json!({"execution_id": E1, "workspace_id": W1, "ttl_ms": 60000})),
            ),
        ),
        "lease",
    );
    assert_eq!(lease_a["authority"]["agent_id"], AGENT_A);
    ok(
        &call(
            fixture.addr(),
            TOKEN_B,
            &envelope(
                Some(AGENT_B),
                "create-e2-shared",
                None,
                "create_execution",
                Some(json!({
                    "execution_id": "http-execution-b-shared",
                    "task_id": TASK,
                    "parent_execution_id": null,
                    "workspace_id": W1,
                    "base_version_id": null
                })),
            ),
        ),
        "execution",
    );
    ok(
        &call(
            fixture.addr(),
            TOKEN_B,
            &envelope(
                Some(AGENT_B),
                "start-e2-shared",
                None,
                "start_execution",
                Some(json!({
                    "execution_id": "http-execution-b-shared",
                    "expected_revision": 0
                })),
            ),
        ),
        "execution",
    );
    let conflict = call(
        fixture.addr(),
        TOKEN_B,
        &envelope(
            Some(AGENT_B),
            "lease-b-conflict",
            None,
            "acquire_workspace_lease",
            Some(json!({
                "execution_id": "http-execution-b-shared",
                "workspace_id": W1,
                "ttl_ms": 60000
            })),
        ),
    );
    assert_eq!(conflict.status, 409);
    assert_eq!(conflict.body["error"]["code"], "LEASE_CONFLICT");
}

#[test]
fn three_real_http_clients_create_isolated_executions_concurrently() {
    let repository_dir = tempdir().unwrap();
    let workspace_dir = tempdir().unwrap();
    init_repository(repository_dir.path());
    let agents = [
        ("multi-agent-a", "multi-token-a"),
        ("multi-agent-b", "multi-token-b"),
        ("multi-agent-c", "multi-token-c"),
    ];
    let verifier = StaticCredentialVerifier::new(
        agents
            .iter()
            .map(|(agent, token)| CredentialGrant {
                credential: (*token).into(),
                principal_id: format!("principal:{agent}"),
                agent_ids: vec![(*agent).into()],
                expires_at_ms: None,
            })
            .collect(),
    )
    .unwrap();
    let repository = Repository::open_as_core_owner(repository_dir.path()).unwrap();
    let server = HttpRemoteServer::start(
        HttpServerConfig::default(),
        Arc::new(verifier),
        Arc::new(AgentProtocolCore::new(
            repository,
            Bindings {
                root: workspace_dir.path().into(),
            },
        )),
    )
    .unwrap();
    let addr = server.listen_addr();
    let handles = agents
        .into_iter()
        .map(|(agent, token)| {
            thread::spawn(move || {
                register(addr, token, agent);
                let task = format!("task:{agent}");
                let workspace = format!("workspace:{agent}");
                let execution = format!("execution:{agent}");
                ok(
                    &call(
                        addr,
                        token,
                        &envelope(
                            Some(agent),
                            &format!("create-task:{agent}"),
                            None,
                            "create_task",
                            Some(json!({
                                "task_id": task,
                                "project_id": PROJECT,
                                "goal_ref": "goal:concurrent",
                                "context_ref": null
                            })),
                        ),
                    ),
                    "task",
                );
                ok(
                    &call(
                        addr,
                        token,
                        &envelope(
                            Some(agent),
                            &format!("create-workspace:{agent}"),
                            None,
                            "create_workspace",
                            Some(json!({
                                "workspace_id": workspace,
                                "project_id": PROJECT,
                                "binding_ref": agent,
                                "branch_ref": null,
                                "environment_id": ENVIRONMENT
                            })),
                        ),
                    ),
                    "workspace",
                );
                ok(
                    &call(
                        addr,
                        token,
                        &envelope(
                            Some(agent),
                            &format!("create-execution:{agent}"),
                            None,
                            "create_execution",
                            Some(json!({
                                "execution_id": execution,
                                "task_id": task,
                                "parent_execution_id": null,
                                "workspace_id": workspace,
                                "base_version_id": null
                            })),
                        ),
                    ),
                    "execution",
                )
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        assert_eq!(handle.join().unwrap()["state"], "created");
    }
}

#[test]
fn http_completes_checkpoint_handoff_resume_and_continuation_checkpoint() {
    let fixture = Fixture::new();
    let addr = fixture.addr();
    bootstrap_a(&fixture);
    register(addr, TOKEN_B, AGENT_B);
    ok(
        &call(
            addr,
            TOKEN_B,
            &envelope(
                Some(AGENT_B),
                "create-w2",
                None,
                "create_workspace",
                Some(json!({
                    "workspace_id": "http-workspace-b",
                    "project_id": PROJECT,
                    "binding_ref": "runtime-b",
                    "branch_ref": null,
                    "environment_id": ENVIRONMENT
                })),
            ),
        ),
        "workspace",
    );
    let lease_a = ok(
        &call(
            addr,
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "handoff-lease-a",
                None,
                "acquire_workspace_lease",
                Some(json!({"execution_id": E1, "workspace_id": W1, "ttl_ms": 60000})),
            ),
        ),
        "lease",
    )["authority"]
        .clone();
    fs::write(
        fixture
            .workspace_dir
            .path()
            .join("runtime-a")
            .join("work.txt"),
        "remote Agent A work",
    )
    .unwrap();
    let publication_a = ok(
        &call(
            addr,
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "handoff-publish-a",
                Some("operation:http:publish-a"),
                "publish_version",
                Some(json!({
                    "execution_id": E1,
                    "workspace_id": W1,
                    "lease": lease_a,
                    "expected_workspace_revision": 0,
                    "parent_version_id": null,
                    "update_version_head": true
                })),
            ),
        ),
        "version_publication",
    );
    let version_a = publication_a["version"]["version_id"].as_str().unwrap();
    ok(
        &call(
            addr,
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "handoff-set-a",
                None,
                "set_execution_current_version",
                Some(json!({
                    "execution_id": E1,
                    "version_id": version_a,
                    "lease": lease_a,
                    "expected_execution_revision": 1,
                    "expected_workspace_revision": 2
                })),
            ),
        ),
        "execution",
    );
    ok(
        &call(
            addr,
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "handoff-checkpoint-a",
                None,
                "create_checkpoint",
                Some(json!({
                    "checkpoint_id": "checkpoint:http:a",
                    "task_id": TASK,
                    "execution_id": E1,
                    "workspace_id": W1,
                    "version_id": version_a,
                    "source_operation_id": "operation:http:publish-a",
                    "reason_code": "handoff_ready"
                })),
            ),
        ),
        "checkpoint",
    );
    ok(
        &call(
            addr,
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "handoff-interrupt-a",
                None,
                "interrupt_execution",
                Some(json!({
                    "execution_id": E1,
                    "expected_revision": 2,
                    "outcome_code": "handoff"
                })),
            ),
        ),
        "execution",
    );
    ok(
        &call(
            addr,
            TOKEN_B,
            &envelope(
                Some(AGENT_B),
                "handoff-resume-b",
                None,
                "resume_from_checkpoint",
                Some(json!({
                    "execution_id": "http-execution-b",
                    "task_id": TASK,
                    "parent_execution_id": E1,
                    "workspace_id": "http-workspace-b",
                    "checkpoint_id": "checkpoint:http:a"
                })),
            ),
        ),
        "resume",
    );
    ok(
        &call(
            addr,
            TOKEN_A,
            &envelope(
                Some(AGENT_A),
                "handoff-record",
                None,
                "create_handoff",
                Some(json!({
                    "handoff_id": "handoff:http:a-b",
                    "task_id": TASK,
                    "from_execution_id": E1,
                    "to_execution_id": "http-execution-b",
                    "source_version_id": version_a,
                    "checkpoint_id": "checkpoint:http:a",
                    "reason_code": "runtime_change"
                })),
            ),
        ),
        "handoff",
    );
    ok(
        &call(
            addr,
            TOKEN_B,
            &envelope(
                Some(AGENT_B),
                "handoff-start-b",
                None,
                "start_execution",
                Some(json!({"execution_id": "http-execution-b", "expected_revision": 0})),
            ),
        ),
        "execution",
    );
    let lease_b = ok(
        &call(
            addr,
            TOKEN_B,
            &envelope(
                Some(AGENT_B),
                "handoff-lease-b",
                None,
                "acquire_workspace_lease",
                Some(json!({
                    "execution_id": "http-execution-b",
                    "workspace_id": "http-workspace-b",
                    "ttl_ms": 60000
                })),
            ),
        ),
        "lease",
    )["authority"]
        .clone();
    ok(
        &call(
            addr,
            TOKEN_B,
            &envelope(
                Some(AGENT_B),
                "handoff-materialize-b",
                None,
                "materialize_version",
                Some(json!({
                    "execution_id": "http-execution-b",
                    "workspace_id": "http-workspace-b",
                    "source_version_id": version_a,
                    "lease": lease_b,
                    "expected_workspace_revision": 0
                })),
            ),
        ),
        "snapshot",
    );
    let b_path = fixture
        .workspace_dir
        .path()
        .join("runtime-b")
        .join("work.txt");
    assert_eq!(fs::read_to_string(&b_path).unwrap(), "remote Agent A work");
    fs::write(&b_path, "remote Agent A work\nremote Agent B continuation").unwrap();
    let publication_b = ok(
        &call(
            addr,
            TOKEN_B,
            &envelope(
                Some(AGENT_B),
                "handoff-publish-b",
                Some("operation:http:publish-b"),
                "publish_version",
                Some(json!({
                    "execution_id": "http-execution-b",
                    "workspace_id": "http-workspace-b",
                    "lease": lease_b,
                    "expected_workspace_revision": 1,
                    "parent_version_id": null,
                    "update_version_head": true
                })),
            ),
        ),
        "version_publication",
    );
    let version_b = publication_b["version"]["version_id"].as_str().unwrap();
    ok(
        &call(
            addr,
            TOKEN_B,
            &envelope(
                Some(AGENT_B),
                "handoff-checkpoint-b",
                None,
                "create_checkpoint",
                Some(json!({
                    "checkpoint_id": "checkpoint:http:b",
                    "task_id": TASK,
                    "execution_id": "http-execution-b",
                    "workspace_id": "http-workspace-b",
                    "version_id": version_b,
                    "source_operation_id": "operation:http:publish-b",
                    "reason_code": "continuation_saved"
                })),
            ),
        ),
        "checkpoint",
    );
}

struct SlowDispatcher {
    started: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl ProtocolDispatch for SlowDispatcher {
    fn dispatch(
        &self,
        _request: pong_core::protocol::ProtocolRequest,
        _now_ms: i64,
    ) -> Result<pong_core::protocol::ProtocolResponse, ProtocolDispatchError> {
        self.started.send(()).unwrap();
        self.release.lock().unwrap().recv().unwrap();
        Ok(pong_core::protocol::ProtocolResponse::invalid_envelope(
            Some("drained".into()),
        ))
    }
}

#[test]
fn graceful_shutdown_stops_accepting_and_drains_in_flight_request() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = HttpRemoteServer::start(
        HttpServerConfig::default(),
        Arc::new(StaticCredentialVerifier::new(grants()).unwrap()),
        Arc::new(SlowDispatcher {
            started: started_tx,
            release: Mutex::new(release_rx),
        }),
    )
    .unwrap();
    let addr = server.listen_addr();
    let client =
        thread::spawn(move || call(addr, TOKEN_A, &envelope(None, "slow", None, "hello", None)));
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let (stopped_tx, stopped_rx) = mpsc::channel();
    thread::spawn(move || {
        server.shutdown().unwrap();
        stopped_tx.send(()).unwrap();
    });
    assert!(stopped_rx.recv_timeout(Duration::from_millis(50)).is_err());
    release_tx.send(()).unwrap();
    stopped_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(client.join().unwrap().status, 400);
    assert!(TcpStream::connect(addr).is_err());
}

struct JsonlTransport {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

struct HttpProcess {
    child: Child,
    stdin: ChildStdin,
    addr: SocketAddr,
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
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        let bytes_read = stdout.read_line(&mut line).unwrap();
        if bytes_read == 0 {
            let status = child.try_wait().unwrap();
            let mut stderr = String::new();
            if let Some(mut pipe) = child.stderr.take() {
                pipe.read_to_string(&mut stderr).unwrap();
            }
            panic!(
                "pong-agent-http exited before readiness (status={status:?}); stderr: {}",
                stderr.trim()
            );
        }
        let ready: Value = serde_json::from_str(&line).unwrap_or_else(|error| {
            panic!("pong-agent-http emitted invalid readiness JSON ({error}); line={line:?}")
        });
        assert_eq!(ready["status"], "ready");
        let addr = ready["listen_addr"].as_str().unwrap().parse().unwrap();
        Self { child, stdin, addr }
    }

    fn shutdown(mut self) {
        self.stdin.write_all(b"shutdown\n").unwrap();
        self.stdin.flush().unwrap();
        assert!(self.child.wait().unwrap().success());
    }
}

#[test]
fn production_http_process_owns_core_authenticates_and_shuts_down_cleanly() {
    let repository = tempdir().unwrap();
    let workspace = tempdir().unwrap();
    let credential_dir = tempdir().unwrap();
    init_repository(repository.path());
    let credential_file = credential_dir.path().join("credentials.json");
    fs::write(
        &credential_file,
        serde_json::to_vec(&json!({
            "credentials": [{
                "credential": TOKEN_A,
                "principal_id": "principal-a",
                "agent_ids": [AGENT_A],
                "expires_at_ms": null
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&credential_file, fs::Permissions::from_mode(0o600))
            .expect("restrict process credential permissions");
    }
    let process = HttpProcess::start(repository.path(), workspace.path(), &credential_file);
    ok(
        &call(
            process.addr,
            TOKEN_A,
            &envelope(None, "process-hello", None, "hello", None),
        ),
        "hello",
    );
    assert!(Repository::open(repository.path()).is_err());
    let second = Command::new(env!("CARGO_BIN_EXE_pong-agent-http"))
        .arg("--repository")
        .arg(repository.path())
        .arg("--workspace-root")
        .arg(workspace.path())
        .arg("--credentials-file")
        .arg(&credential_file)
        .arg("--listen")
        .arg("127.0.0.1:0")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&second.stderr).contains("CONFLICT"));
    process.shutdown();
    Repository::open(repository.path()).expect("graceful shutdown releases Core ownership");
}

#[test]
fn default_config_rejects_implicit_non_loopback_binding() {
    let repository = tempdir().unwrap();
    let workspace = tempdir().unwrap();
    init_repository(repository.path());
    let repository = Repository::open_as_core_owner(repository.path()).unwrap();
    let result = HttpRemoteServer::start(
        HttpServerConfig {
            listen_addr: "0.0.0.0:0".parse().unwrap(),
            ..HttpServerConfig::default()
        },
        Arc::new(StaticCredentialVerifier::new(grants()).unwrap()),
        Arc::new(AgentProtocolCore::new(
            repository,
            Bindings {
                root: workspace.path().into(),
            },
        )),
    );
    assert!(result.is_err());
}

impl JsonlTransport {
    fn start(repository: &Path, workspace: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-protocol"))
            .arg("--repository")
            .arg(repository)
            .arg("--workspace-root")
            .arg(workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        Self {
            stdin: Some(child.stdin.take().unwrap()),
            stdout: BufReader::new(child.stdout.take().unwrap()),
            child,
        }
    }

    fn call(&mut self, request: &Value) -> Value {
        let stdin = self.stdin.as_mut().unwrap();
        serde_json::to_writer(&mut *stdin, request).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn close(mut self) {
        drop(self.stdin.take());
        assert!(self.child.wait().unwrap().success());
    }
}

#[test]
fn http_and_jsonl_are_semantically_equivalent_for_same_protocol_requests() {
    let http = Fixture::new();
    let jsonl_repository = tempdir().unwrap();
    let jsonl_workspace = tempdir().unwrap();
    init_repository(jsonl_repository.path());
    let mut jsonl = JsonlTransport::start(jsonl_repository.path(), jsonl_workspace.path());
    let requests = vec![
        envelope(None, "equivalent-hello", None, "hello", None),
        envelope(
            Some(AGENT_A),
            "equivalent-register",
            None,
            "register_agent",
            Some(json!({
                "agent_id": AGENT_A,
                "provider_metadata": "runtime-neutral",
                "display_name": null
            })),
        ),
        envelope(
            Some(AGENT_A),
            "equivalent-task",
            None,
            "create_task",
            Some(json!({
                "task_id": TASK,
                "project_id": PROJECT,
                "goal_ref": "goal:equivalence",
                "context_ref": null
            })),
        ),
    ];
    for request in requests {
        let http_response = call(http.addr(), TOKEN_A, &request);
        assert_eq!(http_response.body, jsonl.call(&request));
    }
    let mut unsupported = envelope(None, "equivalent-version", None, "hello", None);
    unsupported["protocol_version"] = json!("9.0");
    assert_eq!(
        call(http.addr(), TOKEN_A, &unsupported).body,
        jsonl.call(&unsupported)
    );
    jsonl.close();
}

#[test]
fn http_source_is_provider_neutral_and_does_not_access_repository() {
    let source = include_str!("../src/http_transport.rs");
    for forbidden in [
        "Repository",
        "rusqlite",
        "WorkspaceManager",
        "CodexAgent",
        "ClaudeAgent",
        "CursorAgent",
        "McpServer",
    ] {
        assert!(
            !source.contains(forbidden),
            "unexpected coupling: {forbidden}"
        );
    }
}
