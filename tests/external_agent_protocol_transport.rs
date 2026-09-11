//! Process-boundary JSON Lines validation for the minimal local transport.

use pong_core::Repository;
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "project-transport";
const ENVIRONMENT: &str = "environment-transport";
const AGENT_A: &str = "agent-transport-a";
const AGENT_B: &str = "agent-transport-b";
const TASK: &str = "task-transport";
const W1: &str = "workspace-transport-a";
const W2: &str = "workspace-transport-b";
const E1: &str = "execution-transport-a";
const E2: &str = "execution-transport-b";

struct Fixture {
    repository_dir: TempDir,
    workspace_root: TempDir,
    w1_path: PathBuf,
    w2_path: PathBuf,
}

fn fixture() -> Fixture {
    let repository_dir = tempdir().expect("repository directory");
    let workspace_root = tempdir().expect("workspace binding root");
    let mut repository = Repository::init(repository_dir.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            ENVIRONMENT,
            PROJECT,
            &json!({"schema_version": 1, "test": "protocol-transport"}),
            "t0",
        )
        .expect("environment");
    drop(repository);
    Fixture {
        w1_path: workspace_root.path().join("runtime-a"),
        w2_path: workspace_root.path().join("runtime-b"),
        repository_dir,
        workspace_root,
    }
}

struct Transport {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Transport {
    fn start(repository: &Path, workspace_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-protocol"))
            .arg("--repository")
            .arg(repository)
            .arg("--workspace-root")
            .arg(workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start transport");
        let stdin = child.stdin.take().expect("transport stdin");
        let stdout = BufReader::new(child.stdout.take().expect("transport stdout"));
        Self {
            child,
            stdin: Some(stdin),
            stdout,
        }
    }

    fn send(&mut self, value: &Value) -> Value {
        let stdin = self.stdin.as_mut().expect("open transport stdin");
        serde_json::to_writer(&mut *stdin, value).expect("write JSON request");
        stdin.write_all(b"\n").expect("write request delimiter");
        stdin.flush().expect("flush request");
        self.read_response()
    }

    fn send_raw(&mut self, line: &str) -> Value {
        let stdin = self.stdin.as_mut().expect("open transport stdin");
        stdin.write_all(line.as_bytes()).expect("write raw request");
        stdin.write_all(b"\n").expect("write request delimiter");
        stdin.flush().expect("flush request");
        self.read_response()
    }

    fn read_response(&mut self) -> Value {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("read transport response");
        assert!(!line.is_empty(), "transport exited without a response");
        assert_eq!(line.matches('\n').count(), 1);
        serde_json::from_str(&line).expect("response JSON")
    }

    fn close(mut self) {
        drop(self.stdin.take());
        let status = self.child.wait().expect("wait for transport");
        assert!(status.success(), "transport exit: {status}");
    }
}

fn envelope(
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
        "issued_at": format!("time:{request_id}"),
        "operation_id": operation_id,
        "operation": operation
    });
    if let Some(payload) = payload {
        value["payload"] = payload;
    }
    value
}

fn ok(response: Value, kind: &str) -> Value {
    assert_eq!(response["status"], "ok", "{response:#}");
    assert!(response["error"].is_null());
    assert_eq!(response["result"]["kind"], kind);
    response["result"]["data"].clone()
}

fn register(transport: &mut Transport, agent: &str, provider: &str) {
    ok(
        transport.send(&envelope(
            Some(agent),
            &format!("register:{agent}"),
            None,
            "register_agent",
            Some(json!({
                "agent_id": agent,
                "provider_metadata": provider,
                "display_name": null
            })),
        )),
        "agent",
    );
}

fn create_workspace(transport: &mut Transport, agent: &str, workspace: &str, binding: &str) {
    ok(
        transport.send(&envelope(
            Some(agent),
            &format!("create:{workspace}"),
            None,
            "create_workspace",
            Some(json!({
                "workspace_id": workspace,
                "project_id": PROJECT,
                "binding_ref": binding,
                "branch_ref": null,
                "environment_id": ENVIRONMENT
            })),
        )),
        "workspace",
    );
}

fn acquire(transport: &mut Transport, agent: &str, execution: &str, workspace: &str) -> Value {
    ok(
        transport.send(&envelope(
            Some(agent),
            &format!("lease:{execution}"),
            None,
            "acquire_workspace_lease",
            Some(json!({
                "execution_id": execution,
                "workspace_id": workspace,
                "ttl_ms": 1_000_000
            })),
        )),
        "lease",
    )["authority"]
        .clone()
}

#[test]
fn json_lines_transport_rejects_bad_envelopes_and_unsafe_bindings() {
    let fixture = fixture();
    let mut transport =
        Transport::start(fixture.repository_dir.path(), fixture.workspace_root.path());
    let malformed = transport.send_raw("{not-json");
    assert_eq!(malformed["status"], "error");
    assert_eq!(malformed["request_id"], "unknown");
    assert_eq!(malformed["error"]["code"], "VALIDATION_ERROR");

    register(&mut transport, AGENT_A, "runtime-a");
    let traversal = transport.send(&envelope(
        Some(AGENT_A),
        "unsafe-binding",
        None,
        "create_workspace",
        Some(json!({
            "workspace_id": W1,
            "project_id": PROJECT,
            "binding_ref": "../escape",
            "branch_ref": null,
            "environment_id": ENVIRONMENT
        })),
    ));
    assert_eq!(traversal["status"], "error");
    assert_eq!(traversal["error"]["code"], "VALIDATION_ERROR");
    assert!(!traversal
        .to_string()
        .contains(fixture.workspace_root.path().to_string_lossy().as_ref()));
    assert!(!fixture.workspace_root.path().join("escape").exists());
    transport.close();
}

#[test]
fn process_transport_completes_handoff_and_reconnects_without_session_state() {
    let fixture = fixture();
    let mut transport =
        Transport::start(fixture.repository_dir.path(), fixture.workspace_root.path());
    ok(
        transport.send(&envelope(None, "hello", None, "hello", None)),
        "hello",
    );
    register(&mut transport, AGENT_A, "runtime-a");
    register(&mut transport, AGENT_B, "runtime-b");
    ok(
        transport.send(&envelope(
            Some(AGENT_A),
            "create-task",
            None,
            "create_task",
            Some(json!({
                "task_id": TASK,
                "project_id": PROJECT,
                "goal_ref": "goal:transport-handoff",
                "context_ref": "context:transport-test"
            })),
        )),
        "task",
    );
    create_workspace(&mut transport, AGENT_A, W1, "runtime-a");
    create_workspace(&mut transport, AGENT_B, W2, "runtime-b");
    ok(
        transport.send(&envelope(
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
        )),
        "execution",
    );
    ok(
        transport.send(&envelope(
            Some(AGENT_A),
            "start-e1",
            None,
            "start_execution",
            Some(json!({"execution_id": E1, "expected_revision": 0})),
        )),
        "execution",
    );
    let lease_a = acquire(&mut transport, AGENT_A, E1, W1);
    fs::write(fixture.w1_path.join("work.txt"), "runtime A work").unwrap();
    let publication_a = ok(
        transport.send(&envelope(
            Some(AGENT_A),
            "publish-e1",
            Some("operation:transport:e1"),
            "publish_version",
            Some(json!({
                "execution_id": E1,
                "workspace_id": W1,
                "lease": lease_a,
                "expected_workspace_revision": 0,
                "parent_version_id": null,
                "update_version_head": true
            })),
        )),
        "version_publication",
    );
    let source_version = publication_a["version"]["version_id"]
        .as_str()
        .unwrap()
        .to_string();
    ok(
        transport.send(&envelope(
            Some(AGENT_A),
            "set-e1-version",
            None,
            "set_execution_current_version",
            Some(json!({
                "execution_id": E1,
                "version_id": source_version,
                "lease": lease_a,
                "expected_execution_revision": 1,
                "expected_workspace_revision": 2
            })),
        )),
        "execution",
    );
    ok(
        transport.send(&envelope(
            Some(AGENT_A),
            "checkpoint-c1",
            None,
            "create_checkpoint",
            Some(json!({
                "checkpoint_id": "checkpoint:transport:c1",
                "task_id": TASK,
                "execution_id": E1,
                "workspace_id": W1,
                "version_id": source_version,
                "source_operation_id": "operation:transport:e1",
                "reason_code": "handoff_ready"
            })),
        )),
        "checkpoint",
    );
    ok(
        transport.send(&envelope(
            Some(AGENT_A),
            "interrupt-e1",
            None,
            "interrupt_execution",
            Some(json!({
                "execution_id": E1,
                "expected_revision": 2,
                "outcome_code": "runtime_stopped"
            })),
        )),
        "execution",
    );

    ok(
        transport.send(&envelope(
            Some(AGENT_B),
            "resume-e2",
            None,
            "resume_from_checkpoint",
            Some(json!({
                "execution_id": E2,
                "task_id": TASK,
                "parent_execution_id": E1,
                "workspace_id": W2,
                "checkpoint_id": "checkpoint:transport:c1"
            })),
        )),
        "resume",
    );
    ok(
        transport.send(&envelope(
            Some(AGENT_A),
            "handoff-a-b",
            None,
            "create_handoff",
            Some(json!({
                "handoff_id": "handoff:transport:a-b",
                "task_id": TASK,
                "from_execution_id": E1,
                "to_execution_id": E2,
                "source_version_id": source_version,
                "checkpoint_id": "checkpoint:transport:c1",
                "reason_code": "runtime_change"
            })),
        )),
        "handoff",
    );
    ok(
        transport.send(&envelope(
            Some(AGENT_B),
            "start-e2",
            None,
            "start_execution",
            Some(json!({"execution_id": E2, "expected_revision": 0})),
        )),
        "execution",
    );
    let lease_b = acquire(&mut transport, AGENT_B, E2, W2);
    ok(
        transport.send(&envelope(
            Some(AGENT_B),
            "materialize-e2",
            None,
            "materialize_version",
            Some(json!({
                "execution_id": E2,
                "workspace_id": W2,
                "source_version_id": source_version,
                "lease": lease_b,
                "expected_workspace_revision": 0
            })),
        )),
        "snapshot",
    );
    assert_eq!(
        fs::read_to_string(fixture.w2_path.join("work.txt")).unwrap(),
        "runtime A work"
    );
    fs::write(
        fixture.w2_path.join("work.txt"),
        "runtime A work\nruntime B continuation",
    )
    .unwrap();
    let publication_b = ok(
        transport.send(&envelope(
            Some(AGENT_B),
            "publish-e2",
            Some("operation:transport:e2"),
            "publish_version",
            Some(json!({
                "execution_id": E2,
                "workspace_id": W2,
                "lease": lease_b,
                "expected_workspace_revision": 1,
                "parent_version_id": null,
                "update_version_head": true
            })),
        )),
        "version_publication",
    );
    let target_version = publication_b["version"]["version_id"]
        .as_str()
        .unwrap()
        .to_string();
    ok(
        transport.send(&envelope(
            Some(AGENT_B),
            "set-e2-version",
            None,
            "set_execution_current_version",
            Some(json!({
                "execution_id": E2,
                "version_id": target_version,
                "lease": lease_b,
                "expected_execution_revision": 1,
                "expected_workspace_revision": 3
            })),
        )),
        "execution",
    );
    ok(
        transport.send(&envelope(
            Some(AGENT_B),
            "checkpoint-c2",
            None,
            "create_checkpoint",
            Some(json!({
                "checkpoint_id": "checkpoint:transport:c2",
                "task_id": TASK,
                "execution_id": E2,
                "workspace_id": W2,
                "version_id": target_version,
                "source_operation_id": "operation:transport:e2",
                "reason_code": "continuation_saved"
            })),
        )),
        "checkpoint",
    );
    transport.close();

    let mut reconnected =
        Transport::start(fixture.repository_dir.path(), fixture.workspace_root.path());
    let inspection = ok(
        reconnected.send(&envelope(
            Some(AGENT_B),
            "inspect-e2-after-reconnect",
            None,
            "inspect_execution",
            Some(json!({"id": E2})),
        )),
        "execution_inspection",
    );
    assert_eq!(inspection["agent"]["agent_id"], AGENT_B);
    assert_eq!(inspection["execution"]["parent_execution_id"], E1);
    assert_eq!(inspection["execution"]["base_version_id"], source_version);
    assert_eq!(
        inspection["execution"]["current_version_id"],
        target_version
    );
    assert_eq!(
        inspection["resume"]["checkpoint_id"],
        "checkpoint:transport:c1"
    );
    assert_eq!(
        inspection["checkpoints"][0]["checkpoint_id"],
        "checkpoint:transport:c2"
    );
    assert_eq!(
        inspection["handoffs"][0]["handoff_id"],
        "handoff:transport:a-b"
    );
    assert_eq!(
        inspection["operations"][0]["operation_id"],
        "operation:transport:e2"
    );
    let response_text = inspection.to_string();
    assert!(!response_text.contains("locator"));
    assert!(!response_text.contains(fixture.workspace_root.path().to_string_lossy().as_ref()));
    reconnected.close();

    let repository = Repository::open(fixture.repository_dir.path()).expect("core reopen");
    let source = repository.metadata().workspace(W1).unwrap().unwrap();
    let target = repository.metadata().workspace(W2).unwrap().unwrap();
    assert_ne!(source.locator, target.locator);
    assert_ne!(source.head, target.head);
    assert_eq!(
        repository.metadata().list_checkpoints(TASK).unwrap().len(),
        2
    );
}
