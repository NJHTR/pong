use pong_core::bootstrap::{
    discover, resolve_endpoint, BootstrapMetadata, BOOTSTRAP_PROTOCOL_VERSION,
};
use pong_core::Repository;
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "project-bootstrap";
const AGENT: &str = "agent-bootstrap";
const TASK: &str = "task-bootstrap";
const WORKSPACE: &str = "workspace-bootstrap";
const EXECUTION: &str = "execution-bootstrap";

struct Transport {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Transport {
    fn start(project_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-protocol"))
            .current_dir(project_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start bootstrap transport");
        let stdin = child.stdin.take().expect("transport stdin");
        let stdout = BufReader::new(child.stdout.take().expect("transport stdout"));
        Self {
            child,
            stdin: Some(stdin),
            stdout,
        }
    }

    fn send(&mut self, request: Value) -> Value {
        let stdin = self.stdin.as_mut().expect("open transport stdin");
        serde_json::to_writer(&mut *stdin, &request).expect("write request");
        stdin.write_all(b"\n").expect("write request delimiter");
        stdin.flush().expect("flush request");
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read response");
        assert!(!line.is_empty(), "transport exited without a response");
        serde_json::from_str(&line).expect("response JSON")
    }

    fn close(mut self) {
        drop(self.stdin.take());
        let status = self.child.wait().expect("wait for transport");
        assert!(status.success(), "transport exit: {status}");
    }
}

fn envelope(request_id: &str, operation: &str, payload: Value) -> Value {
    json!({
        "protocol_version": BOOTSTRAP_PROTOCOL_VERSION,
        "request_id": request_id,
        "caller_agent_id": AGENT,
        "issued_at": format!("time:{request_id}"),
        "operation_id": null,
        "operation": operation,
        "payload": payload
    })
}

fn data(response: Value, kind: &str) -> Value {
    assert_eq!(response["status"], "ok", "{response:#}");
    assert!(response["error"].is_null());
    assert_eq!(response["result"]["kind"], kind);
    response["result"]["data"].clone()
}

fn setup_project() -> TempDir {
    let project = tempdir().expect("project root");
    let workspace_root = project.path().join("bindings");
    fs::create_dir_all(&workspace_root).expect("workspace binding root");
    let repository = Repository::init(project.path()).expect("repository");
    drop(repository);
    BootstrapMetadata::new(".", "bindings", Some(WORKSPACE.into()), None)
        .write(project.path())
        .expect("bootstrap metadata");
    project
}

#[test]
fn zero_explicit_path_bootstrap_reconnects_after_core_restart() {
    let project = setup_project();
    let mut first = Transport::start(project.path());
    data(
        first.send(envelope(
            "register",
            "register_agent",
            json!({
                "agent_id": AGENT,
                "provider_metadata": "test",
                "display_name": null
            }),
        )),
        "agent",
    );
    data(
        first.send(envelope(
            "task",
            "create_task",
            json!({
                "task_id": TASK,
                "project_id": PROJECT,
                "goal_ref": "goal:bootstrap",
                "context_ref": null
            }),
        )),
        "task",
    );
    data(
        first.send(envelope(
            "workspace",
            "create_workspace",
            json!({
                "workspace_id": WORKSPACE,
                "project_id": PROJECT,
                "binding_ref": "runtime",
                "branch_ref": null,
                "environment_id": null
            }),
        )),
        "workspace",
    );
    data(
        first.send(envelope(
            "execution",
            "create_execution",
            json!({
                "execution_id": EXECUTION,
                "task_id": TASK,
                "parent_execution_id": null,
                "workspace_id": WORKSPACE,
                "base_version_id": null
            }),
        )),
        "execution",
    );
    data(
        first.send(envelope(
            "start",
            "start_execution",
            json!({"execution_id": EXECUTION, "expected_revision": 0}),
        )),
        "execution",
    );
    first.close();

    let mut second = Transport::start(project.path());
    data(
        second.send(envelope(
            "register",
            "register_agent",
            json!({
                "agent_id": AGENT,
                "provider_metadata": "test",
                "display_name": null
            }),
        )),
        "agent",
    );
    let workspace = data(
        second.send(envelope(
            "inspect-workspace",
            "get_workspace",
            json!({"id": WORKSPACE}),
        )),
        "workspace_inspection",
    );
    assert_eq!(workspace["workspace"]["workspace_id"], WORKSPACE);
    let execution = data(
        second.send(envelope(
            "inspect-execution",
            "inspect_execution",
            json!({"id": EXECUTION}),
        )),
        "execution_inspection",
    );
    assert_eq!(execution["execution"]["execution_id"], EXECUTION);
    assert_eq!(execution["execution"]["agent_id"], AGENT);
    second.close();
}

#[test]
fn bootstrap_failures_are_distinct_and_endpoint_precedence_is_deterministic() {
    let missing = tempdir().expect("missing project");
    let error = discover(missing.path()).expect_err("missing .pong must fail");
    assert_eq!(error.code(), "BOOTSTRAP_PONG_DIRECTORY_MISSING");

    let malformed = tempdir().expect("malformed project");
    fs::create_dir(malformed.path().join(".pong")).expect(".pong");
    fs::write(BootstrapMetadata::path(malformed.path()), b"{ not-json")
        .expect("malformed bootstrap");
    let error = discover(malformed.path()).expect_err("malformed bootstrap must fail");
    assert_eq!(error.code(), "BOOTSTRAP_METADATA_INVALID");

    let repository_missing = tempdir().expect("repository-missing project");
    fs::create_dir(repository_missing.path().join("bindings")).expect("bindings");
    BootstrapMetadata::new(
        "missing-repository",
        "bindings",
        None,
        Some("http://127.0.0.1:8743".into()),
    )
    .write(repository_missing.path())
    .expect("bootstrap metadata");
    let error = discover(repository_missing.path()).expect_err("missing repository must fail");
    assert_eq!(error.code(), "BOOTSTRAP_REPOSITORY_MISSING");

    let incompatible = tempdir().expect("incompatible project");
    fs::create_dir(incompatible.path().join(".pong")).expect(".pong");
    let mut metadata = BootstrapMetadata::new(".", ".", None, None);
    metadata.protocol_version = "2.0".into();
    metadata
        .write(incompatible.path())
        .expect("bootstrap metadata");
    let error = discover(incompatible.path()).expect_err("protocol mismatch must fail");
    assert_eq!(error.code(), "BOOTSTRAP_PROTOCOL_INCOMPATIBLE");

    assert_eq!(
        resolve_endpoint(
            Some("explicit:8743"),
            Some("environment:8743"),
            Some("metadata:8743"),
        )
        .expect("explicit endpoint"),
        Some("explicit:8743".into())
    );
    assert_eq!(
        resolve_endpoint(None, Some("environment:8743"), Some("metadata:8743"))
            .expect("environment endpoint"),
        Some("environment:8743".into())
    );
    assert_eq!(
        resolve_endpoint(None, None, Some("metadata:8743")).expect("metadata endpoint"),
        Some("metadata:8743".into())
    );
    let error = resolve_endpoint(None, Some("\n"), None).expect_err("invalid endpoint");
    assert_eq!(error.code(), "BOOTSTRAP_ENDPOINT_INVALID");
}

#[test]
fn bootstrap_metadata_is_non_secret_and_round_trips() {
    let project = setup_project();
    let path = BootstrapMetadata::path(project.path());
    let value: Value = serde_json::from_slice(&fs::read(path).expect("read bootstrap")).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["protocol_version"], BOOTSTRAP_PROTOCOL_VERSION);
    assert_eq!(value["repository_root"], ".");
    assert_eq!(value["workspace_root"], "bindings");
    assert_eq!(value["workspace_id"], WORKSPACE);
    assert!(value.get("token").is_none());
    assert!(value.get("secret").is_none());
    assert!(value.get("execution_id").is_none());
    assert!(value.get("metadata").is_none());
    assert!(value.get("cas").is_none());
}
