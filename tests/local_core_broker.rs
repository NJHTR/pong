//! M4-010 real-process local Core broker and Runtime session contract.

use pong_core::protocol::{
    ExternalAgentProtocol, ProtocolRequest, WorkspaceBindingError, WorkspaceBindingResolver,
};
use pong_core::Repository;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::tempdir;

const CHILD_MODE: &str = "PONG_CORE_BROKER_CHILD_MODE";
const CONTROL_ROOT: &str = "PONG_CORE_BROKER_CONTROL_ROOT";
const REPOSITORY_ROOT: &str = "PONG_CORE_BROKER_REPOSITORY_ROOT";
const WORKSPACE_ROOT: &str = "PONG_CORE_BROKER_WORKSPACE_ROOT";
const RUNTIME_SCENARIO: &str = "PONG_CORE_BROKER_RUNTIME_SCENARIO";
const RUNTIME_SESSION: &str = "PONG_CORE_BROKER_RUNTIME_SESSION";
const RUNTIME_AGENT: &str = "PONG_CORE_BROKER_RUNTIME_AGENT";
const RUNTIME_SUFFIX: &str = "PONG_CORE_BROKER_RUNTIME_SUFFIX";
const RUNTIME_RESULT: &str = "PONG_CORE_BROKER_RUNTIME_RESULT";
const PROJECT: &str = "project-core-broker";

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HarnessCommand {
    Connect { session_id: String },
    Request { session_id: String, request: Value },
    Disconnect { session_id: String },
    Invalidate { session_id: String },
}

struct LocalBindings {
    root: PathBuf,
}

impl WorkspaceBindingResolver for LocalBindings {
    fn resolve(&self, binding_ref: &str) -> Result<PathBuf, WorkspaceBindingError> {
        Ok(self.root.join(binding_ref))
    }
}

fn requests(control: &Path) -> PathBuf {
    control.join("requests")
}

fn responses(control: &Path) -> PathBuf {
    control.join("responses")
}

fn response_path(control: &Path, command_id: &str) -> PathBuf {
    responses(control).join(format!("{command_id}.json"))
}

fn publish_command(control: &Path, command_id: &str, command: &HarnessCommand) {
    let temporary = requests(control).join(format!("{command_id}.{}.tmp", std::process::id()));
    let published = requests(control).join(format!("{command_id}.json"));
    fs::write(&temporary, serde_json::to_vec(command).unwrap()).unwrap();
    fs::rename(temporary, published).unwrap();
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !path.exists() {
        assert!(Instant::now() < deadline, "timed out waiting for {path:?}");
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_response(control: &Path, command_id: &str) -> Value {
    let path = response_path(control, command_id);
    wait_for(&path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn harness_call(control: &Path, command_id: &str, command: HarnessCommand) -> Value {
    publish_command(control, command_id, &command);
    wait_response(control, command_id)
}

fn connect(control: &Path, session_id: &str) {
    let response = harness_call(
        control,
        &format!("connect-{session_id}"),
        HarnessCommand::Connect {
            session_id: session_id.into(),
        },
    );
    assert_eq!(response["harness_status"], "connected", "{response:#}");
}

fn disconnect(control: &Path, session_id: &str) {
    let response = harness_call(
        control,
        &format!("disconnect-{session_id}"),
        HarnessCommand::Disconnect {
            session_id: session_id.into(),
        },
    );
    assert_eq!(response["harness_status"], "disconnected", "{response:#}");
}

fn protocol_call(control: &Path, session_id: &str, command_id: &str, request: Value) -> Value {
    harness_call(
        control,
        command_id,
        HarnessCommand::Request {
            session_id: session_id.into(),
            request,
        },
    )
}

fn wire(
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

fn assert_protocol_ok(response: &Value) {
    assert_eq!(response["status"], "ok", "{response:#}");
}

fn register(control: &Path, session: &str, agent: &str, suffix: &str) {
    let response = protocol_call(
        control,
        session,
        &format!("{suffix}-register"),
        wire(
            Some(agent),
            &format!("request:{suffix}:register"),
            None,
            "register_agent",
            Some(json!({
                "agent_id": agent,
                "provider_metadata": "runtime-neutral",
                "display_name": null
            })),
        ),
    );
    assert_protocol_ok(&response);
}

fn create_running_execution(control: &Path, session: &str, agent: &str, suffix: &str) {
    let task_id = format!("task-{suffix}");
    let execution_id = format!("execution-{suffix}");
    for (stage, request) in [
        (
            "task",
            wire(
                Some(agent),
                &format!("request:{suffix}:task"),
                None,
                "create_task",
                Some(json!({
                    "task_id": task_id,
                    "project_id": PROJECT,
                    "goal_ref": format!("goal:{suffix}"),
                    "context_ref": null
                })),
            ),
        ),
        (
            "execution",
            wire(
                Some(agent),
                &format!("request:{suffix}:execution"),
                None,
                "create_execution",
                Some(json!({
                    "execution_id": execution_id,
                    "task_id": task_id,
                    "parent_execution_id": null,
                    "workspace_id": null,
                    "base_version_id": null
                })),
            ),
        ),
        (
            "start",
            wire(
                Some(agent),
                &format!("request:{suffix}:start"),
                None,
                "start_execution",
                Some(json!({
                    "execution_id": execution_id,
                    "expected_revision": 0
                })),
            ),
        ),
    ] {
        let response = protocol_call(control, session, &format!("{suffix}-{stage}"), request);
        assert_protocol_ok(&response);
    }
}

fn start_operation(control: &Path, session: &str, agent: &str, suffix: &str, wait: bool) {
    let command_id = format!("{suffix}-operation");
    let command = HarnessCommand::Request {
        session_id: session.into(),
        request: wire(
            Some(agent),
            &format!("request:{suffix}:operation"),
            Some(&format!("operation:{suffix}")),
            "start_operation",
            Some(json!({
                "execution_id": format!("execution-{suffix}"),
                "action": "runtime.work"
            })),
        ),
    };
    if wait {
        let response = harness_call(control, &command_id, command);
        assert_protocol_ok(&response);
    } else {
        publish_command(control, &command_id, &command);
    }
}

fn finish_operation(control: &Path, session: &str, agent: &str, suffix: &str) {
    let response = protocol_call(
        control,
        session,
        &format!("{suffix}-operation-finish"),
        wire(
            Some(agent),
            &format!("request:{suffix}:operation-finish"),
            Some(&format!("operation:{suffix}")),
            "finish_operation",
            Some(json!({
                "execution_id": format!("execution-{suffix}"),
                "status": "COMPLETED",
                "failure": null
            })),
        ),
    );
    assert_protocol_ok(&response);
}

#[test]
fn broker_child() {
    if env::var(CHILD_MODE).ok().as_deref() != Some("broker") {
        return;
    }
    let control = PathBuf::from(env::var_os(CONTROL_ROOT).unwrap());
    let repository_root = PathBuf::from(env::var_os(REPOSITORY_ROOT).unwrap());
    let workspace_root = PathBuf::from(env::var_os(WORKSPACE_ROOT).unwrap());
    let mut repository = Repository::open_as_core_owner(repository_root).unwrap();
    let bindings = LocalBindings {
        root: workspace_root,
    };
    let mut sessions = HashSet::new();
    let mut now_ms = 1_i64;
    let mut stopping_since = None;
    fs::write(control.join("broker.ready"), b"ready").unwrap();

    loop {
        let mut pending = fs::read_dir(requests(&control))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        pending.sort();
        for path in pending {
            let command_id = path.file_stem().unwrap().to_string_lossy().into_owned();
            let command: HarnessCommand =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            let reject_for_shutdown = stopping_since.is_some()
                && matches!(
                    &command,
                    HarnessCommand::Connect { .. } | HarnessCommand::Request { .. }
                );
            let response = if reject_for_shutdown {
                json!({"harness_status": "stopping"})
            } else {
                match command {
                    HarnessCommand::Connect { session_id } => {
                        sessions.insert(session_id.clone());
                        json!({"harness_status": "connected", "session_id": session_id})
                    }
                    HarnessCommand::Disconnect { session_id }
                    | HarnessCommand::Invalidate { session_id } => {
                        sessions.remove(&session_id);
                        json!({"harness_status": "disconnected", "session_id": session_id})
                    }
                    HarnessCommand::Request {
                        session_id,
                        request,
                    } => {
                        if !sessions.contains(&session_id) {
                            json!({"harness_status": "invalid_session", "session_id": session_id})
                        } else {
                            match serde_json::from_value::<ProtocolRequest>(request) {
                                Ok(request) => serde_json::to_value(
                                    ExternalAgentProtocol::new(&mut repository, &bindings)
                                        .handle(request, now_ms),
                                )
                                .unwrap(),
                                Err(_) => json!({"harness_status": "invalid_request"}),
                            }
                        }
                    }
                }
            };
            now_ms += 1;
            let temporary =
                responses(&control).join(format!("{command_id}.{}.tmp", std::process::id()));
            fs::write(&temporary, serde_json::to_vec(&response).unwrap()).unwrap();
            fs::rename(temporary, response_path(&control, &command_id)).unwrap();
            fs::remove_file(path).unwrap();
        }

        let has_pending = fs::read_dir(requests(&control)).unwrap().next().is_some();
        if stopping_since.is_none() && control.join("broker.stop").exists() && !has_pending {
            stopping_since = Some(Instant::now());
            fs::write(control.join("broker.stopping"), b"stopping").unwrap();
        }
        if stopping_since.is_some_and(|started| started.elapsed() >= Duration::from_millis(250))
            && !has_pending
        {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn runtime_child() {
    if env::var(CHILD_MODE).ok().as_deref() != Some("runtime") {
        return;
    }
    let control = PathBuf::from(env::var_os(CONTROL_ROOT).unwrap());
    let scenario = env::var(RUNTIME_SCENARIO).unwrap();
    let session = env::var(RUNTIME_SESSION).unwrap();
    let agent = env::var(RUNTIME_AGENT).unwrap();
    let suffix = env::var(RUNTIME_SUFFIX).unwrap();
    connect(&control, &session);
    let hello = protocol_call(
        &control,
        &session,
        &format!("{suffix}-hello"),
        wire(
            None,
            &format!("request:{suffix}:hello"),
            None,
            "hello",
            None,
        ),
    );
    assert_protocol_ok(&hello);

    match scenario.as_str() {
        "independent" => {
            register(&control, &session, &agent, &suffix);
            create_running_execution(&control, &session, &agent, &suffix);
            start_operation(&control, &session, &agent, &suffix, true);
            finish_operation(&control, &session, &agent, &suffix);
            disconnect(&control, &session);
        }
        "existing_agent" => {
            create_running_execution(&control, &session, &agent, &suffix);
            start_operation(&control, &session, &agent, &suffix, true);
            finish_operation(&control, &session, &agent, &suffix);
            disconnect(&control, &session);
        }
        "transition" => {
            let inspection = protocol_call(
                &control,
                &session,
                &format!("{suffix}-inspect"),
                wire(
                    Some(&agent),
                    &format!("request:{suffix}:inspect"),
                    None,
                    "inspect_execution",
                    Some(json!({"id": "execution-shared-race"})),
                ),
            );
            assert_protocol_ok(&inspection);
            let operation = protocol_call(
                &control,
                &session,
                &format!("{suffix}-operation"),
                wire(
                    Some(&agent),
                    &format!("request:{suffix}:operation"),
                    Some(&format!("operation:{suffix}")),
                    "start_operation",
                    Some(json!({
                        "execution_id": "execution-shared-race",
                        "action": "runtime.concurrent"
                    })),
                ),
            );
            assert_protocol_ok(&operation);
            fs::write(control.join(format!("barrier-{suffix}.ready")), b"ready").unwrap();
            wait_for(&control.join("transition.go"));
            let operation = if suffix.ends_with("pause") {
                "pause_execution"
            } else {
                "complete_execution"
            };
            let response = protocol_call(
                &control,
                &session,
                &format!("{suffix}-transition"),
                wire(
                    Some(&agent),
                    &format!("request:{suffix}:transition"),
                    None,
                    operation,
                    Some(json!({
                        "execution_id": "execution-shared-race",
                        "expected_revision": 1,
                        "outcome_code": null
                    })),
                ),
            );
            fs::write(env::var_os(RUNTIME_RESULT).unwrap(), response.to_string()).unwrap();
            disconnect(&control, &session);
        }
        "crash_after_request" => {
            start_operation(&control, &session, &agent, &suffix, false);
        }
        _ => panic!("unknown Runtime scenario"),
    }
}

struct BrokerProcess {
    child: Child,
}

fn spawn_broker(control: &Path, repository: &Path, workspace: &Path) -> BrokerProcess {
    let child = Command::new(env::current_exe().unwrap())
        .arg("--exact")
        .arg("broker_child")
        .arg("--nocapture")
        .env(CHILD_MODE, "broker")
        .env(CONTROL_ROOT, control)
        .env(REPOSITORY_ROOT, repository)
        .env(WORKSPACE_ROOT, workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    BrokerProcess { child }
}

fn spawn_runtime(
    control: &Path,
    scenario: &str,
    session: &str,
    agent: &str,
    suffix: &str,
    result: Option<&Path>,
) -> Child {
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg("runtime_child")
        .arg("--nocapture")
        .env(CHILD_MODE, "runtime")
        .env(CONTROL_ROOT, control)
        .env(RUNTIME_SCENARIO, scenario)
        .env(RUNTIME_SESSION, session)
        .env(RUNTIME_AGENT, agent)
        .env(RUNTIME_SUFFIX, suffix)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    if let Some(result) = result {
        command.env(RUNTIME_RESULT, result);
    }
    command.spawn().unwrap()
}

fn bootstrap_existing_agent(control: &Path, session: &str, agent: &str, suffix: &str) {
    register(control, session, agent, suffix);
}

#[test]
fn real_process_runtimes_share_one_core_and_preserve_session_boundaries() {
    if env::var(CHILD_MODE).is_ok() {
        return;
    }
    let repository_root = tempdir().unwrap();
    let workspace_root = tempdir().unwrap();
    let control = tempdir().unwrap();
    fs::create_dir(requests(control.path())).unwrap();
    fs::create_dir(responses(control.path())).unwrap();
    let repository = Repository::init(repository_root.path()).unwrap();
    drop(repository);

    let mut broker = spawn_broker(
        control.path(),
        repository_root.path(),
        workspace_root.path(),
    );
    wait_for(&control.path().join("broker.ready"));

    let mut independent = Vec::new();
    for suffix in ["runtime-a", "runtime-b", "runtime-c"] {
        independent.push(spawn_runtime(
            control.path(),
            "independent",
            &format!("session-{suffix}"),
            &format!("agent-{suffix}"),
            suffix,
            None,
        ));
    }
    for child in &mut independent {
        assert!(child.wait().unwrap().success());
    }

    let parent_session = "session-parent";
    connect(control.path(), parent_session);
    bootstrap_existing_agent(
        control.path(),
        parent_session,
        "agent-shared",
        "shared-register",
    );
    let mut same_agent_a = spawn_runtime(
        control.path(),
        "existing_agent",
        "session-shared-a",
        "agent-shared",
        "shared-a",
        None,
    );
    let mut same_agent_b = spawn_runtime(
        control.path(),
        "existing_agent",
        "session-shared-b",
        "agent-shared",
        "shared-b",
        None,
    );
    assert!(same_agent_a.wait().unwrap().success());
    assert!(same_agent_b.wait().unwrap().success());
    for suffix in ["shared-a", "shared-b"] {
        let inspection = protocol_call(
            control.path(),
            parent_session,
            &format!("inspect-{suffix}"),
            wire(
                Some("agent-shared"),
                &format!("request:inspect-{suffix}"),
                None,
                "inspect_execution",
                Some(json!({"id": format!("execution-{suffix}")})),
            ),
        );
        assert_protocol_ok(&inspection);
        assert_eq!(
            inspection["result"]["data"]["execution"]["execution_id"],
            format!("execution-{suffix}")
        );
    }

    connect(control.path(), "session-runtime-a-reconnect");
    let reconnected_runtime = protocol_call(
        control.path(),
        "session-runtime-a-reconnect",
        "runtime-a-reconnect-inspect",
        wire(
            Some("agent-runtime-a"),
            "request:runtime-a-reconnect-inspect",
            None,
            "inspect_execution",
            Some(json!({"id": "execution-runtime-a"})),
        ),
    );
    assert_protocol_ok(&reconnected_runtime);
    disconnect(control.path(), "session-runtime-a-reconnect");

    bootstrap_existing_agent(
        control.path(),
        parent_session,
        "agent-race",
        "race-register",
    );
    create_running_execution(control.path(), parent_session, "agent-race", "shared-race");
    let pause_result = control.path().join("pause-result.json");
    let complete_result = control.path().join("complete-result.json");
    let mut pause = spawn_runtime(
        control.path(),
        "transition",
        "session-race-pause",
        "agent-race",
        "race-pause",
        Some(&pause_result),
    );
    let mut complete = spawn_runtime(
        control.path(),
        "transition",
        "session-race-complete",
        "agent-race",
        "race-complete",
        Some(&complete_result),
    );
    wait_for(&control.path().join("barrier-race-pause.ready"));
    wait_for(&control.path().join("barrier-race-complete.ready"));
    fs::write(control.path().join("transition.go"), b"go").unwrap();
    assert!(pause.wait().unwrap().success());
    assert!(complete.wait().unwrap().success());
    let transition_results = [pause_result, complete_result]
        .map(|path| serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap());
    assert_eq!(
        transition_results
            .iter()
            .filter(|response| response["status"] == "ok")
            .count(),
        1
    );
    assert_eq!(
        transition_results
            .iter()
            .filter(|response| response["error"]["code"] == "REVISION_CONFLICT")
            .count(),
        1
    );

    bootstrap_existing_agent(
        control.path(),
        parent_session,
        "agent-crash",
        "crash-register",
    );
    create_running_execution(
        control.path(),
        parent_session,
        "agent-crash",
        "runtime-crash",
    );
    let mut crashed_runtime = spawn_runtime(
        control.path(),
        "crash_after_request",
        "session-runtime-crash",
        "agent-crash",
        "runtime-crash",
        None,
    );
    assert!(crashed_runtime.wait().unwrap().success());
    wait_for(&response_path(control.path(), "runtime-crash-operation"));
    let resolved = protocol_call(
        control.path(),
        parent_session,
        "resolve-runtime-crash",
        wire(
            Some("agent-crash"),
            "request:resolve-runtime-crash",
            None,
            "resolve_operation",
            Some(json!({
                "project_id": PROJECT,
                "request_id": "request:runtime-crash:operation"
            })),
        ),
    );
    assert_protocol_ok(&resolved);
    assert_eq!(
        resolved["result"]["data"]["operation_id"],
        "operation:runtime-crash"
    );
    let invalidated = harness_call(
        control.path(),
        "invalidate-runtime-crash",
        HarnessCommand::Invalidate {
            session_id: "session-runtime-crash".into(),
        },
    );
    assert_eq!(invalidated["harness_status"], "disconnected");
    let rejected_session = protocol_call(
        control.path(),
        "session-runtime-crash",
        "invalidated-session-request",
        wire(
            Some("agent-crash"),
            "request:invalidated-session",
            None,
            "inspect_execution",
            Some(json!({"id": "execution-runtime-crash"})),
        ),
    );
    assert_eq!(rejected_session["harness_status"], "invalid_session");

    publish_command(
        control.path(),
        "drained-inspection",
        &HarnessCommand::Request {
            session_id: parent_session.into(),
            request: wire(
                Some("agent-crash"),
                "request:drained-inspection",
                None,
                "inspect_execution",
                Some(json!({"id": "execution-runtime-crash"})),
            ),
        },
    );
    fs::write(control.path().join("broker.stop"), b"stop").unwrap();
    let drained = wait_response(control.path(), "drained-inspection");
    assert_protocol_ok(&drained);
    wait_for(&control.path().join("broker.stopping"));
    let late = harness_call(
        control.path(),
        "late-connect",
        HarnessCommand::Connect {
            session_id: "session-too-late".into(),
        },
    );
    assert_eq!(late["harness_status"], "stopping");
    assert!(broker.child.wait().unwrap().success());

    let repository = Repository::open(repository_root.path()).unwrap();
    let interrupted_runtime_operation = repository
        .metadata()
        .operation_record("operation:runtime-crash")
        .unwrap()
        .unwrap();
    assert_eq!(interrupted_runtime_operation.lifecycle_status, "started");
    assert_ne!(interrupted_runtime_operation.lifecycle_status, "completed");
    drop(repository);

    fs::remove_file(control.path().join("broker.stop")).unwrap();
    fs::remove_file(control.path().join("broker.ready")).unwrap();
    let mut replacement = spawn_broker(
        control.path(),
        repository_root.path(),
        workspace_root.path(),
    );
    wait_for(&control.path().join("broker.ready"));
    connect(control.path(), "session-reconnected");
    let after_reopen = protocol_call(
        control.path(),
        "session-reconnected",
        "inspect-after-core-reopen",
        wire(
            Some("agent-crash"),
            "request:inspect-after-core-reopen",
            None,
            "inspect_execution",
            Some(json!({"id": "execution-runtime-crash"})),
        ),
    );
    assert_protocol_ok(&after_reopen);
    replacement.child.kill().unwrap();
    assert!(!replacement.child.wait().unwrap().success());
    let repository =
        Repository::open(repository_root.path()).expect("forced Core exit releases ownership");
    drop(repository);

    fs::remove_file(control.path().join("broker.ready")).unwrap();
    let mut final_core = spawn_broker(
        control.path(),
        repository_root.path(),
        workspace_root.path(),
    );
    wait_for(&control.path().join("broker.ready"));
    connect(control.path(), "session-after-forced-exit");
    let final_inspection = protocol_call(
        control.path(),
        "session-after-forced-exit",
        "final-inspection",
        wire(
            Some("agent-crash"),
            "request:final-inspection",
            None,
            "inspect_execution",
            Some(json!({"id": "execution-runtime-crash"})),
        ),
    );
    assert_protocol_ok(&final_inspection);
    final_core.child.kill().unwrap();
    assert!(!final_core.child.wait().unwrap().success());
}
