//! Independent-process HTTP remote E2E for External Agent Protocol v1.0.
//!
//! The server and client in this suite are separate OS processes.  The test
//! process owns only temporary fixtures and process supervision; it never
//! calls the HTTP handler or protocol dispatcher directly.

use pong_core::{Repository, MIN_HTTP_MAX_RESPONSE_BYTES};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "real-remote-project";
const ENVIRONMENT: &str = "real-remote-environment";
const AGENT_A: &str = "real-agent-a";
const AGENT_B: &str = "real-agent-b";
const TOKEN_A: &str = "real-opaque-token-a";
const TOKEN_B: &str = "real-opaque-token-b";
const TASK: &str = "real-task";
const WORKSPACE_A: &str = "real-workspace-a";
const WORKSPACE_B: &str = "real-workspace-b";
const EXECUTION_A: &str = "real-execution-a";
const EXECUTION_B: &str = "real-execution-b";
const CHECKPOINT_A: &str = "real-checkpoint-a";
const CHECKPOINT_B: &str = "real-checkpoint-b";
const HANDOFF: &str = "real-handoff-a-b";

fn stop_failed_child(child: &mut Child, message: &str) -> ! {
    let _ = child.kill();
    let _ = child.wait();
    panic!("{message}");
}

struct Fixture {
    repository: TempDir,
    workspace_root: TempDir,
    credentials: TempDir,
    server_credentials: PathBuf,
    credential_a: PathBuf,
    credential_b: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let repository = tempdir().expect("repository tempdir");
        let workspace_root = tempdir().expect("workspace tempdir");
        let credentials = tempdir().expect("credentials tempdir");
        let mut repository_handle = Repository::init(repository.path()).expect("init repository");
        repository_handle
            .metadata_mut()
            .record_environment(
                ENVIRONMENT,
                PROJECT,
                &json!({"schema_version": 1, "test": "real-remote-e2e"}),
                "real-remote-e2e",
            )
            .expect("record environment");
        drop(repository_handle);

        let server_credentials = credentials.path().join("server.json");
        let credential_a = credentials.path().join("a.json");
        let credential_b = credentials.path().join("b.json");
        write_credentials(
            &server_credentials,
            &[
                (TOKEN_A, "principal-a", AGENT_A),
                (TOKEN_B, "principal-b", AGENT_B),
            ],
        );
        write_credentials(&credential_a, &[(TOKEN_A, "principal-a", AGENT_A)]);
        write_credentials(&credential_b, &[(TOKEN_B, "principal-b", AGENT_B)]);
        Self {
            repository,
            workspace_root,
            credentials,
            server_credentials,
            credential_a,
            credential_b,
        }
    }

    fn repository_path(&self) -> &Path {
        self.repository.path()
    }

    fn workspace_path(&self) -> &Path {
        self.workspace_root.path()
    }
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
    .expect("write credentials");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .expect("restrict credential permissions");
    }
}

struct ServerProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    output: Receiver<String>,
    diagnostics: Receiver<String>,
    addr: SocketAddr,
}

impl ServerProcess {
    fn start(fixture: &Fixture, extra: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-http"))
            .arg("--repository")
            .arg(fixture.repository_path())
            .arg("--workspace-root")
            .arg(fixture.workspace_path())
            .arg("--credentials-file")
            .arg(&fixture.server_credentials)
            .arg("--listen")
            .arg("127.0.0.1:0")
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start pong-agent-http");
        let stdin = child.stdin.take().expect("server stdin");
        let stdout = child.stdout.take().expect("server stdout");
        let stderr = child.stderr.take().expect("server stderr");
        let (sender, output) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        let (diagnostic_sender, diagnostics) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if diagnostic_sender.send(line).is_err() {
                    break;
                }
            }
        });
        let ready_line = match output.recv_timeout(Duration::from_secs(5)) {
            Ok(line) => line,
            Err(_) => stop_failed_child(&mut child, "HTTP server did not become ready"),
        };
        let ready: Value = match serde_json::from_str(&ready_line) {
            Ok(ready) => ready,
            Err(_) => stop_failed_child(&mut child, "HTTP server readiness was not JSON"),
        };
        if ready["status"] != "ready" {
            stop_failed_child(&mut child, "HTTP server did not report ready");
        }
        let addr: SocketAddr = match ready["listen_addr"]
            .as_str()
            .and_then(|address| address.parse().ok())
        {
            Some(addr) => addr,
            None => stop_failed_child(&mut child, "HTTP server readiness address was invalid"),
        };
        if !addr.ip().is_loopback() || addr.port() == 0 {
            stop_failed_child(
                &mut child,
                "HTTP server did not bind a dynamic loopback address",
            );
        }
        println!("real_remote_e2e server ready at {addr}");
        Self {
            child,
            stdin: Some(stdin),
            output,
            diagnostics,
            addr,
        }
    }

    fn next_control_line(&self) -> String {
        self.output
            .recv_timeout(Duration::from_secs(5))
            .expect("server control response")
    }

    fn send_control(&mut self, command: &[u8]) {
        let stdin = self.stdin.as_mut().expect("server stdin");
        stdin.write_all(command).expect("server control command");
        stdin.flush().expect("flush server control command");
    }

    fn metrics(&mut self) -> Value {
        self.send_control(b"metrics\n");
        serde_json::from_str(&self.next_control_line()).expect("metrics JSON")
    }

    fn reload_credentials(&mut self) -> Value {
        self.send_control(b"reload-credentials\n");
        serde_json::from_str(&self.next_control_line()).expect("reload JSON")
    }

    fn diagnostics(&self) -> String {
        let mut lines = Vec::new();
        while let Ok(line) = self.diagnostics.recv_timeout(Duration::from_millis(20)) {
            lines.push(line);
        }
        lines.join("\n")
    }

    fn is_alive(&mut self) -> bool {
        self.child.try_wait().expect("poll server").is_none()
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait().expect("poll server") {
                return Some(status);
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn graceful_shutdown(&mut self) -> ExitStatus {
        if self.child.try_wait().expect("poll server").is_some() {
            return self.child.wait().expect("wait for stopped server");
        }
        self.send_control(b"shutdown\n");
        self.stdin.take();
        self.wait_for_exit(Duration::from_secs(5))
            .expect("graceful server shutdown timed out")
    }

    fn kill(&mut self) -> ExitStatus {
        let _ = self.child.kill();
        self.child.wait().expect("wait for killed server")
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_some() {
            return;
        }
        if let Some(stdin) = self.stdin.as_mut() {
            let _ = stdin.write_all(b"shutdown\n");
            let _ = stdin.flush();
        }
        self.stdin.take();
        if self.wait_for_exit(Duration::from_secs(5)).is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

struct ClientProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    output: Receiver<String>,
}

impl ClientProcess {
    fn start(endpoint: SocketAddr, credentials: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-http-client"))
            .arg("--endpoint")
            .arg(endpoint.to_string())
            .arg("--credentials-file")
            .arg(credentials)
            .arg("--timeout-ms")
            .arg("3000")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start pong-agent-http-client");
        let stdin = child.stdin.take().expect("client stdin");
        let stdout = child.stdout.take().expect("client stdout");
        let (sender, output) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin: Some(stdin),
            output,
        }
    }

    fn send(&mut self, request: &Value) -> Value {
        let mut stdin = self.stdin.as_mut().expect("client stdin");
        serde_json::to_writer(&mut stdin, request).expect("write protocol request");
        stdin.write_all(b"\n").expect("write request delimiter");
        stdin.flush().expect("flush protocol request");
        let line = self
            .output
            .recv_timeout(Duration::from_secs(5))
            .expect("client response");
        serde_json::from_str(&line).expect("client response JSON")
    }

    fn finish(&mut self) -> ExitStatus {
        self.stdin.take();
        self.wait_for_exit(Duration::from_secs(5))
            .unwrap_or_else(|| {
                let _ = self.child.kill();
                let _ = self.child.wait();
                panic!("client did not exit after stdin closed");
            })
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait().expect("poll client") {
                return Some(status);
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ClientProcess {
    fn drop(&mut self) {
        self.stdin.take();
        if self.wait_for_exit(Duration::from_secs(1)).is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

struct JsonlProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<std::process::ChildStdout>,
}

impl JsonlProcess {
    fn start(fixture: &Fixture) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-protocol"))
            .arg("--repository")
            .arg(fixture.repository_path())
            .arg("--workspace-root")
            .arg(fixture.workspace_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start JSONL process");
        let stdin = child.stdin.take().expect("JSONL stdin");
        let stdout = BufReader::new(child.stdout.take().expect("JSONL stdout"));
        Self {
            child,
            stdin: Some(stdin),
            stdout,
        }
    }

    fn send(&mut self, request: &Value) -> Value {
        let stdin = self.stdin.as_mut().expect("JSONL stdin");
        serde_json::to_writer(&mut *stdin, request).expect("write JSONL request");
        stdin.write_all(b"\n").expect("write JSONL delimiter");
        stdin.flush().expect("flush JSONL request");
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("read JSONL response");
        serde_json::from_str(&line).expect("JSONL response")
    }

    fn finish(&mut self) -> ExitStatus {
        self.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().expect("poll JSONL process") {
                return status;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                panic!("JSONL process did not exit after stdin closed");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for JsonlProcess {
    fn drop(&mut self) {
        self.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(1);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn run_raw_body(endpoint: SocketAddr, credentials: &Path, body: &[u8]) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-http-client"))
        .arg("--endpoint")
        .arg(endpoint.to_string())
        .arg("--credentials-file")
        .arg(credentials)
        .arg("--raw-body")
        .arg("--timeout-ms")
        .arg("3000")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start raw HTTP client");
    let mut stdin = child.stdin.take().expect("raw client stdin");
    stdin.write_all(body).expect("write raw body");
    drop(stdin);
    let mut output = String::new();
    child
        .stdout
        .take()
        .expect("raw client stdout")
        .read_to_string(&mut output)
        .expect("read raw client");
    let status = child.wait().expect("wait raw client");
    assert!(status.success(), "raw client exit: {status}");
    serde_json::from_str(output.trim()).expect("raw client JSON")
}

fn run_raw_http(endpoint: SocketAddr, credentials: &Path, request: &[u8]) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-http-client"))
        .arg("--endpoint")
        .arg(endpoint.to_string())
        .arg("--credentials-file")
        .arg(credentials)
        .arg("--raw-http")
        .arg("--timeout-ms")
        .arg("3000")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start raw HTTP client");
    let mut stdin = child.stdin.take().expect("raw HTTP stdin");
    stdin.write_all(request).expect("write raw HTTP request");
    drop(stdin);
    let mut output = String::new();
    child
        .stdout
        .take()
        .expect("raw HTTP stdout")
        .read_to_string(&mut output)
        .expect("read raw HTTP client");
    let status = child.wait().expect("wait raw HTTP client");
    assert!(status.success(), "raw HTTP client exit: {status}");
    serde_json::from_str(output.trim()).expect("raw HTTP client JSON")
}

fn request(
    caller: Option<&str>,
    request_id: &str,
    operation_id: Option<&str>,
    operation: &str,
    payload: Option<Value>,
) -> Value {
    let mut value = json!({
        "protocol_version": "1.0",
        "request_id": request_id,
        "caller_agent_id": caller,
        "issued_at": format!("real-time:{request_id}"),
        "operation_id": operation_id,
        "operation": operation
    });
    if let Some(payload) = payload {
        value["payload"] = payload;
    }
    value
}

fn response(envelope: Value) -> Value {
    assert!(envelope["error"].is_null(), "protocol error: {envelope}");
    assert_eq!(envelope["response"]["protocol_version"], "1.0");
    envelope["response"].clone()
}

fn call(client: &mut ClientProcess, request: Value) -> Value {
    let envelope = client.send(&request);
    assert_eq!(envelope["http_status"], 200, "HTTP failure: {envelope}");
    response(envelope)
}

fn expect_error(client: &mut ClientProcess, request: Value, status: u16, code: &str) -> Value {
    let envelope = client.send(&request);
    assert_eq!(
        envelope["http_status"], status,
        "unexpected response: {envelope}"
    );
    let actual_code = envelope["response"]["error"]["code"]
        .as_str()
        .or_else(|| envelope["body"]["code"].as_str());
    assert_eq!(actual_code, Some(code), "{envelope}");
    envelope
}

fn data(response: Value, kind: &str) -> Value {
    assert_eq!(response["status"], "ok", "{response}");
    assert_eq!(response["result"]["kind"], kind, "{response}");
    response["result"]["data"].clone()
}

fn assert_safe_error(envelope: &Value, fixture: &Fixture) {
    let rendered = envelope.to_string();
    let repository = fixture.repository_path().to_string_lossy();
    let workspace = fixture.workspace_path().to_string_lossy();
    for forbidden in [
        TOKEN_A,
        TOKEN_B,
        repository.as_ref(),
        workspace.as_ref(),
        "SQLite",
        "rusqlite",
        "src\\",
        "src/",
        "stack trace",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "external error leaked {forbidden:?}: {rendered}"
        );
    }
}

fn register(client: &mut ClientProcess, agent: &str) {
    data(
        call(
            client,
            request(
                Some(agent),
                &format!("register:{agent}"),
                None,
                "register_agent",
                Some(json!({
                    "agent_id": agent,
                    "provider_metadata": "independent-process",
                    "display_name": null
                })),
            ),
        ),
        "agent",
    );
}

#[derive(Clone, Copy)]
enum SemanticActor {
    A,
    B,
}

fn protocol_semantics(response: &Value) -> Value {
    let data = &response["result"]["data"];
    let details = &response["error"]["details"];
    json!({
        "status": response["status"],
        "kind": response["result"]["kind"],
        "operation_id": data["operation_id"],
        "execution_id": data["execution_id"],
        "workspace_id": data["workspace_id"],
        "task_id": data["task_id"],
        "agent_id": data["agent_id"],
        "state": data["state"],
        "revision": data["revision"],
        "error_code": response["error"]["code"],
        "retryable": response["error"]["retryable"],
        "expected_revision": details["expected_revision"],
        "actual_revision": details["actual_revision"]
    })
}

fn run_semantic_equivalence_scenario(
    send: &mut impl FnMut(SemanticActor, &Value) -> Value,
) -> Vec<Value> {
    let mut outcomes = Vec::new();
    {
        let mut perform = |actor, request: Value| {
            let response = send(actor, &request);
            outcomes.push(protocol_semantics(&response));
            response
        };

        perform(
            SemanticActor::A,
            request(None, "equivalence-hello", None, "hello", None),
        );
        for (actor, agent) in [(SemanticActor::A, AGENT_A), (SemanticActor::B, AGENT_B)] {
            perform(
                actor,
                request(
                    Some(agent),
                    &format!("equivalence-register-{agent}"),
                    None,
                    "register_agent",
                    Some(json!({
                        "agent_id": agent,
                        "provider_metadata": "semantic-equivalence",
                        "display_name": null
                    })),
                ),
            );
        }
        perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "equivalence-task",
                None,
                "create_task",
                Some(json!({
                    "task_id": TASK,
                    "project_id": PROJECT,
                    "goal_ref": "goal:semantic-equivalence",
                    "context_ref": null
                })),
            ),
        );
        perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "equivalence-workspace",
                None,
                "create_workspace",
                Some(json!({
                    "workspace_id": WORKSPACE_A,
                    "project_id": PROJECT,
                    "binding_ref": WORKSPACE_A,
                    "branch_ref": null,
                    "environment_id": ENVIRONMENT
                })),
            ),
        );
        perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "equivalence-execution-a",
                None,
                "create_execution",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "task_id": TASK,
                    "parent_execution_id": null,
                    "workspace_id": WORKSPACE_A,
                    "base_version_id": null
                })),
            ),
        );
        perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "equivalence-start-a",
                None,
                "start_execution",
                Some(json!({"execution_id": EXECUTION_A, "expected_revision": 0})),
            ),
        );

        let operation_request = request(
            Some(AGENT_A),
            "equivalence-operation-request",
            Some("equivalence-operation"),
            "start_operation",
            Some(json!({"execution_id": EXECUTION_A, "action": "equivalence"})),
        );
        let first_operation = perform(SemanticActor::A, operation_request.clone());
        let retried_operation = perform(SemanticActor::A, operation_request);
        assert_eq!(
            first_operation["result"]["data"], retried_operation["result"]["data"],
            "exact retry changed the durable Operation"
        );

        let revision_conflict = perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "equivalence-stale-revision",
                None,
                "pause_execution",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "expected_revision": 0,
                    "outcome_code": null
                })),
            ),
        );
        assert_eq!(revision_conflict["error"]["code"], "REVISION_CONFLICT");
        let refreshed = perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "equivalence-refresh",
                None,
                "get_execution",
                Some(json!({"id": EXECUTION_A})),
            ),
        );
        let revision = refreshed["result"]["data"]["revision"]
            .as_i64()
            .expect("refreshed revision");
        let paused = perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "equivalence-pause-after-refresh",
                None,
                "pause_execution",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "expected_revision": revision,
                    "outcome_code": null
                })),
            ),
        );
        let paused_revision = paused["result"]["data"]["revision"]
            .as_i64()
            .expect("paused revision");
        perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "equivalence-restart-after-refresh",
                None,
                "start_execution",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "expected_revision": paused_revision
                })),
            ),
        );

        perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "equivalence-lease-a",
                None,
                "acquire_workspace_lease",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "workspace_id": WORKSPACE_A,
                    "ttl_ms": 60000
                })),
            ),
        );
        perform(
            SemanticActor::B,
            request(
                Some(AGENT_B),
                "equivalence-execution-b",
                None,
                "create_execution",
                Some(json!({
                    "execution_id": EXECUTION_B,
                    "task_id": TASK,
                    "parent_execution_id": null,
                    "workspace_id": WORKSPACE_A,
                    "base_version_id": null
                })),
            ),
        );
        perform(
            SemanticActor::B,
            request(
                Some(AGENT_B),
                "equivalence-start-b",
                None,
                "start_execution",
                Some(json!({"execution_id": EXECUTION_B, "expected_revision": 0})),
            ),
        );
        let lease_conflict = perform(
            SemanticActor::B,
            request(
                Some(AGENT_B),
                "equivalence-lease-conflict",
                None,
                "acquire_workspace_lease",
                Some(json!({
                    "execution_id": EXECUTION_B,
                    "workspace_id": WORKSPACE_A,
                    "ttl_ms": 60000
                })),
            ),
        );
        assert_eq!(lease_conflict["error"]["code"], "LEASE_CONFLICT");

        let unknown = perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "equivalence-unknown-operation",
                None,
                "get_operation",
                Some(json!({"id": "equivalence-missing-operation"})),
            ),
        );
        assert_eq!(unknown["error"]["code"], "NOT_FOUND");
        let mut unsupported = request(None, "equivalence-unsupported-version", None, "hello", None);
        unsupported["protocol_version"] = json!("9.0");
        let unsupported = perform(SemanticActor::A, unsupported);
        assert_eq!(unsupported["error"]["code"], "UNSUPPORTED_VERSION");
    }
    outcomes
}

#[test]
fn real_process_auth_authorization_failure_isolation_and_limits() {
    let fixture = Fixture::new();
    let mut server = ServerProcess::start(&fixture, &["--max-body-bytes", "512"]);
    let mut client_a = ClientProcess::start(server.addr, &fixture.credential_a);
    let hello = call(
        &mut client_a,
        request(None, "hello-process", None, "hello", None),
    );
    assert!(data(hello, "hello")["provider_neutral"] == true);

    let invalid_credentials = fixture.credentials.path().join("invalid.json");
    write_credentials(
        &invalid_credentials,
        &[("invalid-token", "invalid-principal", "invalid-agent")],
    );
    let mut invalid_client = ClientProcess::start(server.addr, &invalid_credentials);
    let invalid = invalid_client.send(&request(None, "invalid-auth", None, "hello", None));
    assert_eq!(invalid["http_status"], 401);
    assert_eq!(invalid["body"]["code"], "AUTHENTICATION_FAILED");
    assert!(!invalid.to_string().contains("invalid-token"));
    assert_safe_error(&invalid, &fixture);

    let forbidden = client_a.send(&request(
        Some(AGENT_B),
        "forbidden-agent",
        None,
        "register_agent",
        Some(json!({
            "agent_id": AGENT_B,
            "provider_metadata": null,
            "display_name": null
        })),
    ));
    assert_eq!(forbidden["http_status"], 403, "{forbidden}");
    assert_eq!(forbidden["body"]["code"], "FORBIDDEN", "{forbidden}");
    assert_safe_error(&forbidden, &fixture);
    let malformed = run_raw_body(server.addr, &fixture.credential_a, b"not-json");
    assert_eq!(malformed["http_status"], 400);
    assert_eq!(malformed["response"]["error"]["code"], "VALIDATION_ERROR");
    assert_safe_error(&malformed, &fixture);
    let oversized = run_raw_body(server.addr, &fixture.credential_a, &vec![b'x'; 513]);
    assert_eq!(oversized["http_status"], 413);
    assert_eq!(oversized["body"]["code"], "PAYLOAD_TOO_LARGE");
    assert_safe_error(&oversized, &fixture);
    let malformed_http = run_raw_http(
        server.addr,
        &fixture.credential_a,
        b"BROKEN REQUEST\r\n\r\n",
    );
    assert!(
        malformed_http["http_status"] == 400 || !malformed_http["error"].is_null(),
        "malformed HTTP was not rejected: {malformed_http}"
    );
    assert_safe_error(&malformed_http, &fixture);

    let healthy_after_rejections = call(
        &mut client_a,
        request(None, "healthy-after-rejections", None, "hello", None),
    );
    assert_eq!(
        data(healthy_after_rejections, "hello")["protocol_versions"],
        json!(["1.0"])
    );
    assert!(server.is_alive());

    let metrics = server.metrics();
    assert!(metrics["requests_total"].as_u64().unwrap_or(0) >= 5);
    assert!(metrics["recent_requests"]
        .to_string()
        .contains("hello-process"));
    assert!(!metrics.to_string().contains(TOKEN_A));
    assert!(!metrics.to_string().contains("Authorization"));
    let diagnostics = server.diagnostics();
    assert!(!diagnostics.contains(TOKEN_A));
    assert!(!diagnostics.contains(TOKEN_B));
    assert!(!diagnostics.contains("invalid-token"));
    assert!(!diagnostics.contains("Authorization"));
}

#[test]
fn real_process_rate_worker_and_response_limits_are_observable() {
    let fixture = Fixture::new();
    let mut limited = ServerProcess::start(
        &fixture,
        &[
            "--rate-limit-requests",
            "1",
            "--rate-limit-window-ms",
            "60000",
            "--worker-threads",
            "2",
        ],
    );
    let mut client = ClientProcess::start(limited.addr, &fixture.credential_a);
    let first = call(
        &mut client,
        request(None, "rate-first", None, "hello", None),
    );
    assert_eq!(data(first, "hello")["transport_neutral"], true);
    let second = client.send(&request(None, "rate-second", None, "hello", None));
    assert_eq!(second["http_status"], 429, "{second}");
    assert_eq!(second["body"]["code"], "RATE_LIMITED");
    let metrics = limited.metrics();
    assert_eq!(metrics["worker_threads"], 2);
    assert!(metrics["peak_active_requests"].as_u64().unwrap_or(0) <= 2);
    assert!(metrics["rate_limit_rejections"].as_u64().unwrap_or(0) >= 1);
    assert!(limited.graceful_shutdown().success());

    let response_limit = MIN_HTTP_MAX_RESPONSE_BYTES.to_string();
    let response_args = ["--max-response-bytes", response_limit.as_str()];
    let mut response_limited = ServerProcess::start(&fixture, &response_args);
    let body = br#"{"protocol_version":"1.0","request_id":"response-limit","issued_at":"t","operation":"hello"}"#;
    let raw_request = format!(
        "POST /v1/protocol HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN_A}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        String::from_utf8_lossy(body)
    );
    let oversized_response = run_raw_http(
        response_limited.addr,
        &fixture.credential_a,
        raw_request.as_bytes(),
    );
    assert_eq!(oversized_response["http_status"], 500);
    assert_eq!(oversized_response["body"]["code"], "RESPONSE_TOO_LARGE");
    assert!(!oversized_response.to_string().contains(TOKEN_A));
    assert!(response_limited.is_alive());
}

#[test]
fn real_process_graceful_shutdown_releases_core_ownership() {
    let fixture = Fixture::new();
    let mut server = ServerProcess::start(&fixture, &[]);
    let mut client = ClientProcess::start(server.addr, &fixture.credential_a);
    let hello = call(
        &mut client,
        request(None, "graceful-before-shutdown", None, "hello", None),
    );
    assert_eq!(data(hello, "hello")["provider_neutral"], true);
    let stopped_addr = server.addr;
    assert!(server.graceful_shutdown().success());
    assert!(server.wait_for_exit(Duration::from_millis(1)).is_some());
    assert!(
        TcpStream::connect_timeout(&stopped_addr, Duration::from_millis(250)).is_err(),
        "graceful shutdown left the old listener accepting connections"
    );

    let mut replacement = ServerProcess::start(&fixture, &[]);
    let mut recovered = ClientProcess::start(replacement.addr, &fixture.credential_a);
    let hello = call(
        &mut recovered,
        request(None, "graceful-after-shutdown", None, "hello", None),
    );
    assert_eq!(data(hello, "hello")["provider_neutral"], true);
    assert!(replacement.is_alive());
}

#[test]
fn real_process_protocol_lifecycle_retry_revision_lease_and_multi_client() {
    let fixture = Fixture::new();
    let mut server = ServerProcess::start(&fixture, &[]);
    let mut client_a = ClientProcess::start(server.addr, &fixture.credential_a);
    let mut client_b = ClientProcess::start(server.addr, &fixture.credential_b);
    register(&mut client_a, AGENT_A);
    register(&mut client_b, AGENT_B);

    data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "create-task",
                None,
                "create_task",
                Some(json!({
                    "task_id": TASK,
                    "project_id": PROJECT,
                    "goal_ref": "goal:real-remote",
                    "context_ref": null
                })),
            ),
        ),
        "task",
    );
    for (client, agent, workspace, id) in [
        (&mut client_a, AGENT_A, WORKSPACE_A, "workspace-a"),
        (&mut client_b, AGENT_B, WORKSPACE_B, "workspace-b"),
    ] {
        data(
            call(
                client,
                request(
                    Some(agent),
                    id,
                    None,
                    "create_workspace",
                    Some(json!({
                        "workspace_id": workspace,
                        "project_id": PROJECT,
                        "binding_ref": workspace,
                        "branch_ref": null,
                        "environment_id": ENVIRONMENT
                    })),
                ),
            ),
            "workspace",
        );
    }
    data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "create-execution-a",
                None,
                "create_execution",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "task_id": TASK,
                    "parent_execution_id": null,
                    "workspace_id": WORKSPACE_A,
                    "base_version_id": null
                })),
            ),
        ),
        "execution",
    );
    data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "start-execution-a",
                None,
                "start_execution",
                Some(json!({"execution_id": EXECUTION_A, "expected_revision": 0})),
            ),
        ),
        "execution",
    );
    let operation_request = request(
        Some(AGENT_A),
        "operation-request",
        Some("operation-real-retry"),
        "start_operation",
        Some(json!({"execution_id": EXECUTION_A, "action": "remote.e2e"})),
    );
    let first_operation = data(call(&mut client_a, operation_request.clone()), "operation");
    let second_operation = data(call(&mut client_a, operation_request), "operation");
    assert_eq!(first_operation, second_operation);

    let stale = expect_error(
        &mut client_a,
        request(
            Some(AGENT_A),
            "stale-revision",
            None,
            "pause_execution",
            Some(json!({
                "execution_id": EXECUTION_A,
                "expected_revision": 0,
                "outcome_code": null
            })),
        ),
        409,
        "REVISION_CONFLICT",
    );
    assert_safe_error(&stale, &fixture);
    let refreshed = data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "refresh-after-revision-conflict",
                None,
                "get_execution",
                Some(json!({"id": EXECUTION_A})),
            ),
        ),
        "execution",
    );
    let refreshed_revision = refreshed["revision"].as_i64().expect("execution revision");
    let paused = data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "pause-after-refresh",
                None,
                "pause_execution",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "expected_revision": refreshed_revision,
                    "outcome_code": null
                })),
            ),
        ),
        "execution",
    );
    assert_eq!(paused["state"], "paused");
    let restarted = data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "restart-after-refresh",
                None,
                "start_execution",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "expected_revision": paused["revision"]
                })),
            ),
        ),
        "execution",
    );
    assert_eq!(restarted["state"], "running");

    let unknown_operation = expect_error(
        &mut client_a,
        request(
            Some(AGENT_A),
            "unknown-operation",
            None,
            "get_operation",
            Some(json!({"id": "operation-does-not-exist"})),
        ),
        404,
        "NOT_FOUND",
    );
    assert_safe_error(&unknown_operation, &fixture);

    let lease = data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "lease-a",
                None,
                "acquire_workspace_lease",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "workspace_id": WORKSPACE_A,
                    "ttl_ms": 60000
                })),
            ),
        ),
        "lease",
    )["authority"]
        .clone();
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "create-execution-b",
                None,
                "create_execution",
                Some(json!({
                    "execution_id": EXECUTION_B,
                    "task_id": TASK,
                    "parent_execution_id": null,
                    "workspace_id": WORKSPACE_A,
                    "base_version_id": null
                })),
            ),
        ),
        "execution",
    );
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "start-execution-b",
                None,
                "start_execution",
                Some(json!({"execution_id": EXECUTION_B, "expected_revision": 0})),
            ),
        ),
        "execution",
    );
    let lease_conflict = expect_error(
        &mut client_b,
        request(
            Some(AGENT_B),
            "lease-b-conflict",
            None,
            "acquire_workspace_lease",
            Some(json!({
                "execution_id": EXECUTION_B,
                "workspace_id": WORKSPACE_A,
                "ttl_ms": 60000
            })),
        ),
        409,
        "LEASE_CONFLICT",
    );
    assert_safe_error(&lease_conflict, &fixture);
    assert_eq!(lease["agent_id"], AGENT_A);

    // Three independent client processes issue requests concurrently to one
    // HTTP process and therefore one Core dispatcher.
    let clients = [
        (fixture.credential_a.clone(), "multi-client-c"),
        (fixture.credential_b.clone(), "multi-client-d"),
        (fixture.credential_a.clone(), "multi-client-e"),
    ]
    .into_iter()
    .map(|(credentials, request_id)| {
        let endpoint = server.addr;
        thread::spawn(move || {
            let mut client = ClientProcess::start(endpoint, &credentials);
            let hello = data(
                call(&mut client, request(None, request_id, None, "hello", None)),
                "hello",
            );
            assert_eq!(hello["transport_neutral"], true);
            assert!(client.finish().success());
        })
    })
    .collect::<Vec<_>>();
    for client in clients {
        client.join().expect("concurrent client process");
    }
    let after_concurrency = call(
        &mut client_a,
        request(None, "healthy-after-concurrency", None, "hello", None),
    );
    assert_eq!(data(after_concurrency, "hello")["transport_neutral"], true);
    assert!(server.is_alive());
}

#[test]
fn real_process_full_checkpoint_handoff_resume_lifecycle_survives_restart() {
    let fixture = Fixture::new();
    let mut server = ServerProcess::start(&fixture, &[]);
    let mut client_a = ClientProcess::start(server.addr, &fixture.credential_a);
    let mut client_b = ClientProcess::start(server.addr, &fixture.credential_b);
    register(&mut client_a, AGENT_A);
    register(&mut client_b, AGENT_B);
    data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "full-task",
                None,
                "create_task",
                Some(json!({
                    "task_id": TASK,
                    "project_id": PROJECT,
                    "goal_ref": "goal:full-real-remote",
                    "context_ref": null
                })),
            ),
        ),
        "task",
    );
    for (client, agent, workspace, request_id) in [
        (&mut client_a, AGENT_A, WORKSPACE_A, "full-workspace-a"),
        (&mut client_b, AGENT_B, WORKSPACE_B, "full-workspace-b"),
    ] {
        data(
            call(
                client,
                request(
                    Some(agent),
                    request_id,
                    None,
                    "create_workspace",
                    Some(json!({
                        "workspace_id": workspace,
                        "project_id": PROJECT,
                        "binding_ref": workspace,
                        "branch_ref": null,
                        "environment_id": ENVIRONMENT
                    })),
                ),
            ),
            "workspace",
        );
    }
    data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "full-execution-a",
                None,
                "create_execution",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "task_id": TASK,
                    "parent_execution_id": null,
                    "workspace_id": WORKSPACE_A,
                    "base_version_id": null
                })),
            ),
        ),
        "execution",
    );
    data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "full-start-a",
                None,
                "start_execution",
                Some(json!({"execution_id": EXECUTION_A, "expected_revision": 0})),
            ),
        ),
        "execution",
    );
    let lease_a = data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "full-lease-a",
                None,
                "acquire_workspace_lease",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "workspace_id": WORKSPACE_A,
                    "ttl_ms": 60000
                })),
            ),
        ),
        "lease",
    )["authority"]
        .clone();
    let source_path = fixture.workspace_path().join(WORKSPACE_A).join("work.txt");
    fs::write(&source_path, "remote Agent A work").expect("write source workspace");
    let publication_a = data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "full-publish-a",
                Some("operation:full-publish-a"),
                "publish_version",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "workspace_id": WORKSPACE_A,
                    "lease": lease_a,
                    "expected_workspace_revision": 0,
                    "parent_version_id": null,
                    "update_version_head": true
                })),
            ),
        ),
        "version_publication",
    );
    let version_a = publication_a["version"]["version_id"]
        .as_str()
        .expect("source version id")
        .to_owned();
    data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "full-set-version-a",
                None,
                "set_execution_current_version",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "version_id": version_a,
                    "lease": lease_a,
                    "expected_execution_revision": 1,
                    "expected_workspace_revision": 2
                })),
            ),
        ),
        "execution",
    );
    data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "full-checkpoint-a",
                None,
                "create_checkpoint",
                Some(json!({
                    "checkpoint_id": CHECKPOINT_A,
                    "task_id": TASK,
                    "execution_id": EXECUTION_A,
                    "workspace_id": WORKSPACE_A,
                    "version_id": version_a,
                    "source_operation_id": "operation:full-publish-a",
                    "reason_code": "handoff_ready"
                })),
            ),
        ),
        "checkpoint",
    );
    data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "full-interrupt-a",
                None,
                "interrupt_execution",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "expected_revision": 2,
                    "outcome_code": "handoff"
                })),
            ),
        ),
        "execution",
    );
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "full-resume-b",
                None,
                "resume_from_checkpoint",
                Some(json!({
                    "execution_id": EXECUTION_B,
                    "task_id": TASK,
                    "parent_execution_id": EXECUTION_A,
                    "workspace_id": WORKSPACE_B,
                    "checkpoint_id": CHECKPOINT_A
                })),
            ),
        ),
        "resume",
    );
    data(
        call(
            &mut client_a,
            request(
                Some(AGENT_A),
                "full-handoff",
                None,
                "create_handoff",
                Some(json!({
                    "handoff_id": HANDOFF,
                    "task_id": TASK,
                    "from_execution_id": EXECUTION_A,
                    "to_execution_id": EXECUTION_B,
                    "source_version_id": version_a,
                    "checkpoint_id": CHECKPOINT_A,
                    "reason_code": "runtime_change"
                })),
            ),
        ),
        "handoff",
    );
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "full-start-b",
                None,
                "start_execution",
                Some(json!({"execution_id": EXECUTION_B, "expected_revision": 0})),
            ),
        ),
        "execution",
    );
    let lease_b = data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "full-lease-b",
                None,
                "acquire_workspace_lease",
                Some(json!({
                    "execution_id": EXECUTION_B,
                    "workspace_id": WORKSPACE_B,
                    "ttl_ms": 60000
                })),
            ),
        ),
        "lease",
    )["authority"]
        .clone();
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "full-materialize-b",
                None,
                "materialize_version",
                Some(json!({
                    "execution_id": EXECUTION_B,
                    "workspace_id": WORKSPACE_B,
                    "source_version_id": version_a,
                    "lease": lease_b,
                    "expected_workspace_revision": 0
                })),
            ),
        ),
        "snapshot",
    );
    let continuation_path = fixture.workspace_path().join(WORKSPACE_B).join("work.txt");
    assert_eq!(
        fs::read_to_string(&continuation_path).unwrap(),
        "remote Agent A work"
    );
    fs::write(
        &continuation_path,
        "remote Agent A work\nremote Agent B continuation",
    )
    .expect("write continuation workspace");
    let publication_b = data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "full-publish-b",
                Some("operation:full-publish-b"),
                "publish_version",
                Some(json!({
                    "execution_id": EXECUTION_B,
                    "workspace_id": WORKSPACE_B,
                    "lease": lease_b,
                    "expected_workspace_revision": 1,
                    "parent_version_id": null,
                    "update_version_head": true
                })),
            ),
        ),
        "version_publication",
    );
    let version_b = publication_b["version"]["version_id"]
        .as_str()
        .expect("continuation version id")
        .to_owned();
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "full-checkpoint-b",
                None,
                "create_checkpoint",
                Some(json!({
                    "checkpoint_id": CHECKPOINT_B,
                    "task_id": TASK,
                    "execution_id": EXECUTION_B,
                    "workspace_id": WORKSPACE_B,
                    "version_id": version_b,
                    "source_operation_id": "operation:full-publish-b",
                    "reason_code": "continuation_saved"
                })),
            ),
        ),
        "checkpoint",
    );

    let metrics = server.metrics();
    let publication_correlation = metrics["recent_requests"]
        .as_array()
        .expect("recent request correlations")
        .iter()
        .find(|entry| entry["request_id"] == "full-publish-b")
        .expect("publication request correlation");
    assert_eq!(
        publication_correlation["operation_id"],
        "operation:full-publish-b"
    );
    assert_eq!(publication_correlation["execution_id"], EXECUTION_B);
    assert_eq!(publication_correlation["principal_id"], "principal-b");
    assert_eq!(publication_correlation["agent_id"], AGENT_B);
    assert_eq!(publication_correlation["workspace_id"], WORKSPACE_B);
    assert!(!metrics.to_string().contains(TOKEN_A));
    assert!(!metrics.to_string().contains(TOKEN_B));
    assert!(!metrics.to_string().contains("Authorization"));

    let _ = server.kill();
    let mut replacement = ServerProcess::start(&fixture, &[]);
    let mut recovered_a = ClientProcess::start(replacement.addr, &fixture.credential_a);
    let mut recovered_b = ClientProcess::start(replacement.addr, &fixture.credential_b);
    for (client, agent, execution) in [
        (&mut recovered_a, AGENT_A, EXECUTION_A),
        (&mut recovered_b, AGENT_B, EXECUTION_B),
    ] {
        let recovered = call(
            client,
            request(
                Some(agent),
                &format!("recovered-execution-{execution}"),
                None,
                "get_execution",
                Some(json!({"id": execution})),
            ),
        );
        assert_eq!(data(recovered, "execution")["execution_id"], execution);
    }
    for (client, agent, version) in [
        (&mut recovered_a, AGENT_A, version_a.as_str()),
        (&mut recovered_b, AGENT_B, version_b.as_str()),
    ] {
        let recovered = call(
            client,
            request(
                Some(agent),
                &format!("recovered-version-{version}"),
                None,
                "get_version",
                Some(json!({"id": version})),
            ),
        );
        assert_eq!(data(recovered, "version")["version_id"], version);
    }
    for (client, agent, checkpoint) in [
        (&mut recovered_a, AGENT_A, CHECKPOINT_A),
        (&mut recovered_b, AGENT_B, CHECKPOINT_B),
    ] {
        let recovered = call(
            client,
            request(
                Some(agent),
                &format!("recovered-checkpoint-{checkpoint}"),
                None,
                "get_checkpoint",
                Some(json!({"id": checkpoint})),
            ),
        );
        assert_eq!(data(recovered, "checkpoint")["checkpoint_id"], checkpoint);
    }
    let handoff = call(
        &mut recovered_a,
        request(
            Some(AGENT_A),
            "recovered-handoff",
            None,
            "get_handoff",
            Some(json!({"id": HANDOFF})),
        ),
    );
    assert_eq!(data(handoff, "handoff")["handoff_id"], HANDOFF);
    let inspection = call(
        &mut recovered_b,
        request(
            Some(AGENT_B),
            "recovered-inspection-b",
            None,
            "inspect_execution",
            Some(json!({"id": EXECUTION_B})),
        ),
    );
    assert_eq!(
        data(inspection, "execution_inspection")["execution"]["execution_id"],
        EXECUTION_B
    );
    assert!(replacement.is_alive());
}

#[test]
fn real_process_restart_reconnect_and_second_core_protection() {
    let fixture = Fixture::new();
    let mut server = ServerProcess::start(&fixture, &[]);
    let mut client = ClientProcess::start(server.addr, &fixture.credential_a);
    register(&mut client, AGENT_A);
    data(
        call(
            &mut client,
            request(
                Some(AGENT_A),
                "create-task-restart",
                None,
                "create_task",
                Some(json!({
                    "task_id": TASK,
                    "project_id": PROJECT,
                    "goal_ref": "goal:restart",
                    "context_ref": null
                })),
            ),
        ),
        "task",
    );
    data(
        call(
            &mut client,
            request(
                Some(AGENT_A),
                "create-restart-workspace",
                None,
                "create_workspace",
                Some(json!({
                    "workspace_id": WORKSPACE_A,
                    "project_id": PROJECT,
                    "binding_ref": WORKSPACE_A,
                    "branch_ref": null,
                    "environment_id": ENVIRONMENT
                })),
            ),
        ),
        "workspace",
    );
    data(
        call(
            &mut client,
            request(
                Some(AGENT_A),
                "create-restart-execution",
                None,
                "create_execution",
                Some(json!({
                    "execution_id": EXECUTION_A,
                    "task_id": TASK,
                    "parent_execution_id": null,
                    "workspace_id": WORKSPACE_A,
                    "base_version_id": null
                })),
            ),
        ),
        "execution",
    );
    data(
        call(
            &mut client,
            request(
                Some(AGENT_A),
                "start-restart-execution",
                None,
                "start_execution",
                Some(json!({"execution_id": EXECUTION_A, "expected_revision": 0})),
            ),
        ),
        "execution",
    );
    data(
        call(
            &mut client,
            request(
                Some(AGENT_A),
                "start-restart-operation",
                Some("operation-restart"),
                "start_operation",
                Some(json!({"execution_id": EXECUTION_A, "action": "restart"})),
            ),
        ),
        "operation",
    );
    let second = Command::new(env!("CARGO_BIN_EXE_pong-agent-http"))
        .arg("--repository")
        .arg(fixture.repository_path())
        .arg("--workspace-root")
        .arg(fixture.workspace_path())
        .arg("--credentials-file")
        .arg(&fixture.server_credentials)
        .arg("--listen")
        .arg("127.0.0.1:0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("start second Core");
    assert_eq!(second.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&second.stderr).contains("CONFLICT"));
    assert!(server.is_alive());

    // Lose the client connection after sending a durable operation.  The
    // fresh client resolves it by request identity after the Core restarts.
    let uncertain_request = request(
        Some(AGENT_A),
        "uncertain-restart-request",
        Some("operation-uncertain-restart"),
        "start_operation",
        Some(json!({"execution_id": EXECUTION_A, "action": "uncertain"})),
    );
    let body = serde_json::to_vec(&uncertain_request).unwrap();
    let mut raw_client = Command::new(env!("CARGO_BIN_EXE_pong-agent-http-client"))
        .arg("--endpoint")
        .arg(server.addr.to_string())
        .arg("--credentials-file")
        .arg(&fixture.credential_a)
        .arg("--disconnect-after-send")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start disconnecting client");
    raw_client
        .stdin
        .take()
        .expect("disconnect client stdin")
        .write_all(&body)
        .expect("write disconnect request");
    let _ = raw_client.wait();
    drop(client);
    let _killed_status = server.kill();

    let mut replacement = ServerProcess::start(&fixture, &[]);
    let mut fresh_client = ClientProcess::start(replacement.addr, &fixture.credential_a);
    let operation = call(
        &mut fresh_client,
        request(
            Some(AGENT_A),
            "resolve-restart-operation",
            None,
            "resolve_operation",
            Some(json!({
                "project_id": PROJECT,
                "request_id": "uncertain-restart-request"
            })),
        ),
    );
    assert_eq!(
        data(operation, "operation")["operation_id"],
        "operation-uncertain-restart"
    );
    let operation = call(
        &mut fresh_client,
        request(
            Some(AGENT_A),
            "get-restart-operation",
            None,
            "get_operation",
            Some(json!({"id": "operation-restart"})),
        ),
    );
    assert_eq!(
        data(operation, "operation")["operation_id"],
        "operation-restart"
    );
    assert!(replacement.is_alive());
}

#[test]
fn real_process_credential_rotation_and_http_jsonl_equivalence() {
    let fixture = Fixture::new();
    let mut server = ServerProcess::start(&fixture, &[]);
    let mut client_a = ClientProcess::start(server.addr, &fixture.credential_a);
    let hello_request = request(None, "equivalence-hello", None, "hello", None);
    let http_response = call(&mut client_a, hello_request.clone());
    assert_eq!(
        data(http_response.clone(), "hello")["protocol_versions"],
        json!(["1.0"])
    );
    let mut client_b = ClientProcess::start(server.addr, &fixture.credential_b);
    register(&mut client_b, AGENT_B);
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "rotation-task",
                None,
                "create_task",
                Some(json!({
                    "task_id": "rotation-task",
                    "project_id": PROJECT,
                    "goal_ref": "goal:rotation",
                    "context_ref": null
                })),
            ),
        ),
        "task",
    );
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "rotation-workspace",
                None,
                "create_workspace",
                Some(json!({
                    "workspace_id": WORKSPACE_B,
                    "project_id": PROJECT,
                    "binding_ref": WORKSPACE_B,
                    "branch_ref": null,
                    "environment_id": ENVIRONMENT
                })),
            ),
        ),
        "workspace",
    );
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "rotation-execution",
                None,
                "create_execution",
                Some(json!({
                    "execution_id": EXECUTION_B,
                    "task_id": "rotation-task",
                    "parent_execution_id": null,
                    "workspace_id": WORKSPACE_B,
                    "base_version_id": null
                })),
            ),
        ),
        "execution",
    );
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "rotation-start",
                None,
                "start_execution",
                Some(json!({"execution_id": EXECUTION_B, "expected_revision": 0})),
            ),
        ),
        "execution",
    );
    data(
        call(
            &mut client_b,
            request(
                Some(AGENT_B),
                "rotation-operation",
                Some("operation-rotation-running"),
                "start_operation",
                Some(json!({"execution_id": EXECUTION_B, "action": "rotation"})),
            ),
        ),
        "operation",
    );

    write_credentials(
        &fixture.server_credentials,
        &[(TOKEN_B, "principal-b", AGENT_B)],
    );
    assert_eq!(
        server.reload_credentials()["status"],
        "credentials_reloaded"
    );
    let revoked = client_a.send(&request(
        None,
        "revoked-after-rotation",
        None,
        "hello",
        None,
    ));
    assert_eq!(revoked["http_status"], 401);
    assert_safe_error(&revoked, &fixture);
    let rotated = call(
        &mut client_b,
        request(None, "rotated-hello", None, "hello", None),
    );
    assert_eq!(data(rotated, "hello")["protocol_versions"], json!(["1.0"]));
    let execution = call(
        &mut client_b,
        request(
            Some(AGENT_B),
            "rotation-existing-execution",
            None,
            "get_execution",
            Some(json!({"id": EXECUTION_B})),
        ),
    );
    assert_eq!(data(execution, "execution")["state"], "running");
    let operation = call(
        &mut client_b,
        request(
            Some(AGENT_B),
            "rotation-existing-operation",
            None,
            "get_operation",
            Some(json!({"id": "operation-rotation-running"})),
        ),
    );
    assert_eq!(
        data(operation, "operation")["operation_id"],
        "operation-rotation-running"
    );
    assert!(server.is_alive());

    server.graceful_shutdown();
    let mut jsonl = JsonlProcess::start(&fixture);
    let jsonl_response = jsonl.send(&hello_request);
    assert_eq!(http_response, jsonl_response);
    assert!(jsonl.finish().success());
}

#[test]
fn real_process_http_jsonl_semantics_cover_conflicts_retry_and_reconnect() {
    let http_fixture = Fixture::new();
    let mut server = ServerProcess::start(&http_fixture, &[]);
    let mut http_a = ClientProcess::start(server.addr, &http_fixture.credential_a);
    let mut http_b = ClientProcess::start(server.addr, &http_fixture.credential_b);
    let http_outcomes = run_semantic_equivalence_scenario(&mut |actor, request| {
        let envelope = match actor {
            SemanticActor::A => http_a.send(request),
            SemanticActor::B => http_b.send(request),
        };
        response(envelope)
    });
    assert!(http_a.finish().success());
    assert!(http_b.finish().success());

    let jsonl_fixture = Fixture::new();
    let mut jsonl = JsonlProcess::start(&jsonl_fixture);
    let jsonl_outcomes =
        run_semantic_equivalence_scenario(&mut |_actor, request| jsonl.send(request));
    assert!(jsonl.finish().success());
    assert_eq!(
        http_outcomes, jsonl_outcomes,
        "HTTP and JSONL changed protocol-level outcomes"
    );

    let resolve_request = request(
        Some(AGENT_A),
        "equivalence-resolve-after-reconnect",
        None,
        "resolve_operation",
        Some(json!({
            "project_id": PROJECT,
            "request_id": "equivalence-operation-request"
        })),
    );
    let mut reconnected_http = ClientProcess::start(server.addr, &http_fixture.credential_a);
    let http_reconnected = response(reconnected_http.send(&resolve_request));
    let mut reconnected_jsonl = JsonlProcess::start(&jsonl_fixture);
    let jsonl_reconnected = reconnected_jsonl.send(&resolve_request);
    assert_eq!(
        protocol_semantics(&http_reconnected),
        protocol_semantics(&jsonl_reconnected)
    );
    assert_eq!(
        http_reconnected["result"]["data"]["operation_id"],
        "equivalence-operation"
    );
    assert_eq!(
        jsonl_reconnected["result"]["data"]["operation_id"],
        "equivalence-operation"
    );
    assert!(reconnected_http.finish().success());
    assert!(reconnected_jsonl.finish().success());
    assert!(server.graceful_shutdown().success());
}

fn run_resource_authorization_scenario(
    workspace_root: &Path,
    send: &mut impl FnMut(SemanticActor, &Value) -> Value,
) -> Vec<Value> {
    let mut outcomes = Vec::new();
    let mut perform = |actor, request: Value| {
        let result = send(actor, &request);
        outcomes.push(protocol_semantics(&result));
        result
    };
    for (actor, agent) in [(SemanticActor::A, AGENT_A), (SemanticActor::B, AGENT_B)] {
        assert_eq!(
            perform(
                actor,
                request(
                    Some(agent),
                    &format!("authz-register-{agent}"),
                    None,
                    "register_agent",
                    Some(json!({
                        "agent_id": agent,
                        "provider_metadata": null,
                        "display_name": null
                    }))
                )
            )["status"],
            "ok"
        );
    }
    for (id, operation, payload) in [
        (
            "task",
            "create_task",
            json!({"task_id": TASK, "project_id": PROJECT, "goal_ref": "shared", "context_ref": null}),
        ),
        (
            "workspace",
            "create_workspace",
            json!({"workspace_id": WORKSPACE_A, "project_id": PROJECT, "binding_ref": WORKSPACE_A, "branch_ref": null, "environment_id": ENVIRONMENT}),
        ),
        (
            "execution",
            "create_execution",
            json!({"execution_id": EXECUTION_A, "task_id": TASK, "parent_execution_id": null, "workspace_id": WORKSPACE_A, "base_version_id": null}),
        ),
        (
            "start",
            "start_execution",
            json!({"execution_id": EXECUTION_A, "expected_revision": 0}),
        ),
    ] {
        assert_eq!(
            perform(
                SemanticActor::A,
                request(
                    Some(AGENT_A),
                    &format!("authz-{id}"),
                    None,
                    operation,
                    Some(payload)
                )
            )["status"],
            "ok"
        );
    }
    let lease = perform(
        SemanticActor::A,
        request(
            Some(AGENT_A),
            "authz-lease-a",
            None,
            "acquire_workspace_lease",
            Some(
                json!({"execution_id": EXECUTION_A, "workspace_id": WORKSPACE_A, "ttl_ms": 60000}),
            ),
        ),
    )["result"]["data"]["authority"]
        .clone();
    fs::write(
        workspace_root.join(WORKSPACE_A).join("authz.txt"),
        "shared source",
    )
    .unwrap();
    let publication = perform(
        SemanticActor::A,
        request(
            Some(AGENT_A),
            "authz-publish",
            Some("authz-publish-operation"),
            "publish_version",
            Some(json!({
                "execution_id": EXECUTION_A,
                "workspace_id": WORKSPACE_A,
                "lease": lease,
                "expected_workspace_revision": 0,
                "parent_version_id": null,
                "update_version_head": true
            })),
        ),
    );
    assert_eq!(publication["status"], "ok", "{publication}");
    let version = publication["result"]["data"]["version"]["version_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        perform(
            SemanticActor::A,
            request(
                Some(AGENT_A),
                "authz-checkpoint",
                None,
                "create_checkpoint",
                Some(json!({
                    "checkpoint_id": CHECKPOINT_A,
                    "task_id": TASK,
                    "execution_id": EXECUTION_A,
                    "workspace_id": WORKSPACE_A,
                    "version_id": version,
                    "source_operation_id": "authz-publish-operation",
                    "reason_code": "ready"
                }))
            )
        )["status"],
        "ok"
    );
    fs::write(
        workspace_root.join(WORKSPACE_A).join("authz.txt"),
        "shared changed",
    )
    .unwrap();
    for (id, operation, payload, expected) in [
        ("task", "get_task", json!({"id": TASK}), "ok"),
        (
            "workspace",
            "get_workspace",
            json!({"id": WORKSPACE_A}),
            "ok",
        ),
        ("version", "get_version", json!({"id": version}), "ok"),
        (
            "checkpoint",
            "get_checkpoint",
            json!({"id": CHECKPOINT_A}),
            "ok",
        ),
        (
            "checkpoints",
            "list_checkpoints",
            json!({"task_id": TASK}),
            "ok",
        ),
        ("handoffs", "list_handoffs", json!({"task_id": TASK}), "ok"),
        (
            "diff",
            "diff_workspace_version",
            json!({"workspace_id": WORKSPACE_A, "source_version_id": version}),
            "ok",
        ),
        (
            "execution",
            "get_execution",
            json!({"id": EXECUTION_A}),
            "error",
        ),
        (
            "inspect",
            "inspect_execution",
            json!({"id": EXECUTION_A}),
            "error",
        ),
        (
            "operation",
            "get_operation",
            json!({"id": "authz-publish-operation"}),
            "error",
        ),
        (
            "write",
            "start_execution",
            json!({"execution_id": EXECUTION_A, "expected_revision": 1}),
            "error",
        ),
        (
            "lease",
            "acquire_workspace_lease",
            json!({"execution_id": EXECUTION_A, "workspace_id": WORKSPACE_A, "ttl_ms": 60000}),
            "error",
        ),
        (
            "checkpoint-write",
            "create_checkpoint",
            json!({
                "checkpoint_id": "foreign-checkpoint",
                "task_id": TASK,
                "execution_id": EXECUTION_A,
                "workspace_id": WORKSPACE_A,
                "version_id": version,
                "source_operation_id": null,
                "reason_code": "foreign"
            }),
            "error",
        ),
        (
            "handoff-write",
            "create_handoff",
            json!({
                "handoff_id": "foreign-handoff",
                "task_id": TASK,
                "from_execution_id": EXECUTION_A,
                "to_execution_id": "missing",
                "source_version_id": version,
                "checkpoint_id": CHECKPOINT_A,
                "reason_code": "foreign"
            }),
            "error",
        ),
        (
            "materialize",
            "materialize_version",
            json!({
                "execution_id": EXECUTION_A,
                "workspace_id": WORKSPACE_A,
                "source_version_id": version,
                "lease": lease,
                "expected_workspace_revision": 2
            }),
            "error",
        ),
    ] {
        let response = perform(
            SemanticActor::B,
            request(
                Some(AGENT_B),
                &format!("authz-{id}-b"),
                None,
                operation,
                Some(payload),
            ),
        );
        assert_eq!(response["status"], expected, "{id}: {response}");
        if expected == "error" {
            assert_eq!(response["error"]["code"], "FORBIDDEN", "{id}: {response}");
        }
        if id == "workspace" {
            assert_eq!(response["result"]["data"]["workspace"]["revision"], 2);
            assert_eq!(
                response["result"]["data"]["lease"]["authority"]["agent_id"],
                AGENT_A
            );
        }
        if id == "diff" {
            assert_eq!(
                response["result"]["data"]["entries"][0]["path"],
                "authz.txt"
            );
            let rendered = response.to_string();
            assert!(!rendered.contains(&workspace_root.to_string_lossy().to_string()));
            assert!(!rendered.contains("shared changed"));
        }
    }
    let forbidden_state = perform(
        SemanticActor::B,
        request(
            Some(AGENT_B),
            "authz-foreign-checkpoint-query",
            None,
            "get_checkpoint",
            Some(json!({"id": "foreign-checkpoint"})),
        ),
    );
    assert_eq!(forbidden_state["error"]["code"], "NOT_FOUND");
    let healthy = perform(
        SemanticActor::A,
        request(
            Some(AGENT_A),
            "authz-owner-after-denials",
            None,
            "inspect_execution",
            Some(json!({"id": EXECUTION_A})),
        ),
    );
    assert_eq!(healthy["status"], "ok");
    assert_eq!(healthy["result"]["data"]["execution"]["revision"], 1);
    let resumed = perform(
        SemanticActor::B,
        request(
            Some(AGENT_B),
            "authz-resume-b",
            None,
            "resume_from_checkpoint",
            Some(json!({
                "execution_id": EXECUTION_B,
                "task_id": TASK,
                "parent_execution_id": EXECUTION_A,
                "workspace_id": WORKSPACE_A,
                "checkpoint_id": CHECKPOINT_A
            })),
        ),
    );
    assert_eq!(resumed["status"], "ok", "{resumed}");
    let handoff = perform(
        SemanticActor::A,
        request(
            Some(AGENT_A),
            "authz-owner-handoff",
            None,
            "create_handoff",
            Some(json!({
                "handoff_id": HANDOFF,
                "task_id": TASK,
                "from_execution_id": EXECUTION_A,
                "to_execution_id": EXECUTION_B,
                "source_version_id": version,
                "checkpoint_id": CHECKPOINT_A,
                "reason_code": "continuation"
            })),
        ),
    );
    assert_eq!(handoff["status"], "ok", "{handoff}");
    let shared_handoff = perform(
        SemanticActor::B,
        request(
            Some(AGENT_B),
            "authz-read-handoff-b",
            None,
            "get_handoff",
            Some(json!({"id": HANDOFF})),
        ),
    );
    assert_eq!(shared_handoff["status"], "ok", "{shared_handoff}");
    let own_continuation = perform(
        SemanticActor::B,
        request(
            Some(AGENT_B),
            "authz-own-continuation-b",
            None,
            "get_execution",
            Some(json!({"id": EXECUTION_B})),
        ),
    );
    assert_eq!(own_continuation["status"], "ok", "{own_continuation}");
    outcomes
}

#[test]
fn real_process_cross_agent_authorization_matches_http_and_jsonl() {
    let http_fixture = Fixture::new();
    let mut server = ServerProcess::start(&http_fixture, &[]);
    let mut client_a = ClientProcess::start(server.addr, &http_fixture.credential_a);
    let mut client_b = ClientProcess::start(server.addr, &http_fixture.credential_b);
    let http = run_resource_authorization_scenario(
        http_fixture.workspace_path(),
        &mut |actor, request| {
            response(match actor {
                SemanticActor::A => client_a.send(request),
                SemanticActor::B => client_b.send(request),
            })
        },
    );
    assert!(server.is_alive());
    assert!(client_a.finish().success());
    assert!(client_b.finish().success());
    assert!(server.graceful_shutdown().success());

    let jsonl_fixture = Fixture::new();
    let mut jsonl = JsonlProcess::start(&jsonl_fixture);
    let jsonl = run_resource_authorization_scenario(
        jsonl_fixture.workspace_path(),
        &mut |_actor, request| jsonl.send(request),
    );
    assert_eq!(
        http, jsonl,
        "HTTP and JSONL diverged on resource authorization"
    );
}
