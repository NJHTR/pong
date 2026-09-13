//! Lifecycle, uncertain-outcome, and reconnect coverage for protocol 1.0.

use pong_core::protocol::*;
use pong_core::Repository;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "project-lifecycle";
const ENVIRONMENT: &str = "environment-lifecycle";
const AGENT_A: &str = "agent-lifecycle-a";
const AGENT_B: &str = "agent-lifecycle-b";
const TASK: &str = "task-lifecycle";
const WORKSPACE: &str = "workspace-lifecycle";
const EXECUTION: &str = "execution-lifecycle";

struct Bindings(HashMap<String, PathBuf>);

impl WorkspaceBindingResolver for Bindings {
    fn resolve(&self, binding_ref: &str) -> Result<PathBuf, WorkspaceBindingError> {
        self.0
            .get(binding_ref)
            .cloned()
            .ok_or(WorkspaceBindingError::NotFound)
    }
}

struct Fixture {
    repository_dir: TempDir,
    _workspace_dir: TempDir,
    repository: Repository,
    bindings: Bindings,
}

fn fixture() -> Fixture {
    let repository_dir = tempdir().expect("repository directory");
    let workspace_dir = tempdir().expect("workspace directory");
    let mut repository = Repository::init(repository_dir.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            ENVIRONMENT,
            PROJECT,
            &json!({"schema_version": 1, "test": "protocol-lifecycle"}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_dir.path().join("runtime-a");
    Fixture {
        repository_dir,
        _workspace_dir: workspace_dir,
        repository,
        bindings: Bindings(HashMap::from([("runtime-a".into(), workspace_path)])),
    }
}

fn request(
    caller: Option<&str>,
    request_id: &str,
    operation_id: Option<&str>,
    issued_at: &str,
    call: ProtocolCall,
) -> ProtocolRequest {
    ProtocolRequest {
        protocol_version: EXTERNAL_AGENT_PROTOCOL_VERSION.into(),
        request_id: request_id.into(),
        caller_agent_id: caller.map(Into::into),
        issued_at: issued_at.into(),
        operation_id: operation_id.map(Into::into),
        call,
    }
}

fn send(fixture: &mut Fixture, request: ProtocolRequest) -> ProtocolResponse {
    ExternalAgentProtocol::new(&mut fixture.repository, &fixture.bindings).handle(request, 10)
}

fn success(response: ProtocolResponse) -> ProtocolResult {
    assert_eq!(response.status, ProtocolStatus::Ok, "{response:?}");
    assert!(response.error.is_none());
    response.result.expect("protocol result")
}

fn failure(response: ProtocolResponse, code: ProtocolErrorCode) -> ProtocolError {
    assert_eq!(response.status, ProtocolStatus::Error, "{response:?}");
    assert!(response.result.is_none());
    let error = response.error.expect("protocol error");
    assert_eq!(error.code, code);
    error
}

fn operation(result: ProtocolResult) -> OperationResource {
    match result {
        ProtocolResult::Operation(operation) => operation,
        result => panic!("unexpected result: {result:?}"),
    }
}

fn register(fixture: &mut Fixture, agent: &str) {
    success(send(
        fixture,
        request(
            Some(agent),
            &format!("register:{agent}"),
            None,
            "t1",
            ProtocolCall::RegisterAgent(RegisterAgentCommand {
                agent_id: agent.into(),
                provider_metadata: Some("runtime-neutral".into()),
                display_name: None,
            }),
        ),
    ));
}

fn bootstrap(fixture: &mut Fixture) {
    register(fixture, AGENT_A);
    register(fixture, AGENT_B);
    success(send(
        fixture,
        request(
            Some(AGENT_A),
            "create-task",
            None,
            "t2",
            ProtocolCall::CreateTask(CreateTaskCommand {
                task_id: TASK.into(),
                project_id: PROJECT.into(),
                goal_ref: "goal:lifecycle".into(),
                context_ref: None,
            }),
        ),
    ));
    success(send(
        fixture,
        request(
            Some(AGENT_A),
            "create-workspace",
            None,
            "t3",
            ProtocolCall::CreateWorkspace(CreateWorkspaceCommand {
                workspace_id: WORKSPACE.into(),
                project_id: PROJECT.into(),
                binding_ref: "runtime-a".into(),
                branch_ref: None,
                environment_id: Some(ENVIRONMENT.into()),
            }),
        ),
    ));
    success(send(
        fixture,
        request(
            Some(AGENT_A),
            "create-execution",
            None,
            "t4",
            ProtocolCall::CreateExecution(CreateExecutionCommand {
                execution_id: EXECUTION.into(),
                task_id: TASK.into(),
                parent_execution_id: None,
                workspace_id: Some(WORKSPACE.into()),
                base_version_id: None,
            }),
        ),
    ));
    success(send(
        fixture,
        request(
            Some(AGENT_A),
            "start-execution",
            None,
            "t5",
            ProtocolCall::StartExecution(ExecutionRevisionCommand {
                execution_id: EXECUTION.into(),
                expected_revision: 0,
            }),
        ),
    ));
}

fn start_operation_request(request_id: &str, operation_id: &str) -> ProtocolRequest {
    request(
        Some(AGENT_A),
        request_id,
        Some(operation_id),
        "t6",
        ProtocolCall::StartOperation(StartOperationCommand {
            execution_id: EXECUTION.into(),
            action: "agent.work".into(),
        }),
    )
}

fn finish_operation_request(
    request_id: &str,
    operation_id: &str,
    status: OperationTerminalStatus,
    failure: Option<OperationFailureResource>,
) -> ProtocolRequest {
    request(
        Some(AGENT_A),
        request_id,
        Some(operation_id),
        "t7",
        ProtocolCall::FinishOperation(FinishOperationCommand {
            execution_id: EXECUTION.into(),
            status,
            failure,
        }),
    )
}

#[test]
fn hello_advertises_lifecycle_queries_without_changing_protocol_version() {
    let mut fixture = fixture();
    let ProtocolResult::Hello(hello) = success(send(
        &mut fixture,
        request(None, "hello", None, "t0", ProtocolCall::Hello),
    )) else {
        panic!("expected hello");
    };
    assert_eq!(hello.protocol_versions, ["1.0"]);
    assert!(hello.commands.contains(&"start_operation".into()));
    assert!(hello.commands.contains(&"finish_operation".into()));
    assert!(hello.queries.contains(&"resolve_operation".into()));
    assert_eq!(hello.mutation_acknowledgement, "durable_or_error");
}

#[test]
fn started_operation_is_durable_and_attached_to_execution() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    let started = operation(success(send(
        &mut fixture,
        start_operation_request("request:start", "operation:start"),
    )));
    assert_eq!(started.state, "started");
    assert_eq!(started.recording_state, "durable");
    assert_eq!(started.execution_id.as_deref(), Some(EXECUTION));
    assert_eq!(started.agent_id, AGENT_A);
    assert_eq!(started.workspace_id.as_deref(), Some(WORKSPACE));
}

#[test]
fn operation_start_requires_running_owned_execution() {
    let mut fixture = fixture();
    register(&mut fixture, AGENT_A);
    register(&mut fixture, AGENT_B);
    failure(
        send(
            &mut fixture,
            start_operation_request("request:missing", "operation:missing"),
        ),
        ProtocolErrorCode::NotFound,
    );
    bootstrap(&mut fixture);
    let foreign = request(
        Some(AGENT_B),
        "request:foreign",
        Some("operation:foreign"),
        "t6",
        ProtocolCall::StartOperation(StartOperationCommand {
            execution_id: EXECUTION.into(),
            action: "agent.work".into(),
        }),
    );
    failure(send(&mut fixture, foreign), ProtocolErrorCode::Forbidden);
}

#[test]
fn operation_start_validates_identity_and_action() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    let missing_id = request(
        Some(AGENT_A),
        "request:no-operation",
        None,
        "t6",
        ProtocolCall::StartOperation(StartOperationCommand {
            execution_id: EXECUTION.into(),
            action: "agent.work".into(),
        }),
    );
    failure(
        send(&mut fixture, missing_id),
        ProtocolErrorCode::ValidationError,
    );
    let empty_action = request(
        Some(AGENT_A),
        "request:empty-action",
        Some("operation:empty-action"),
        "t6",
        ProtocolCall::StartOperation(StartOperationCommand {
            execution_id: EXECUTION.into(),
            action: "  ".into(),
        }),
    );
    failure(
        send(&mut fixture, empty_action),
        ProtocolErrorCode::ValidationError,
    );
}

#[test]
fn exact_start_retry_returns_one_durable_operation() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    let start_request = start_operation_request("request:retry", "operation:retry");
    let first = operation(success(send(&mut fixture, start_request.clone())));
    let retry = operation(success(send(&mut fixture, start_request)));
    assert_eq!(retry, first);
    let inspection = match success(send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "inspect",
            None,
            "t8",
            ProtocolCall::InspectExecution(EntityQuery {
                id: EXECUTION.into(),
            }),
        ),
    )) {
        ProtocolResult::ExecutionInspection(value) => value,
        result => panic!("unexpected result: {result:?}"),
    };
    assert_eq!(inspection.operations.len(), 1);
}

#[test]
fn changed_retry_is_rejected_as_idempotency_reuse() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    success(send(
        &mut fixture,
        start_operation_request("request:reuse", "operation:reuse"),
    ));
    let changed = request(
        Some(AGENT_A),
        "request:reuse",
        Some("operation:reuse"),
        "t6",
        ProtocolCall::StartOperation(StartOperationCommand {
            execution_id: EXECUTION.into(),
            action: "agent.changed".into(),
        }),
    );
    failure(
        send(&mut fixture, changed),
        ProtocolErrorCode::IdempotencyConflict,
    );
}

#[test]
fn request_resolution_distinguishes_not_started_from_started() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    let query = |request_id: &str| {
        request(
            Some(AGENT_A),
            &format!("resolve:{request_id}"),
            None,
            "t8",
            ProtocolCall::ResolveOperation(OperationRequestQuery {
                project_id: PROJECT.into(),
                request_id: request_id.into(),
            }),
        )
    };
    failure(
        send(&mut fixture, query("request:absent")),
        ProtocolErrorCode::NotFound,
    );
    success(send(
        &mut fixture,
        start_operation_request("request:present", "operation:present"),
    ));
    let found = operation(success(send(&mut fixture, query("request:present"))));
    assert_eq!(found.operation_id, "operation:present");
    assert_eq!(found.state, "started");
}

#[test]
fn operation_queries_enforce_agent_ownership() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    success(send(
        &mut fixture,
        start_operation_request("request:owned", "operation:owned"),
    ));
    let foreign_get = request(
        Some(AGENT_B),
        "foreign-get",
        None,
        "t8",
        ProtocolCall::GetOperation(EntityQuery {
            id: "operation:owned".into(),
        }),
    );
    failure(
        send(&mut fixture, foreign_get),
        ProtocolErrorCode::Forbidden,
    );
    let foreign_inspect = request(
        Some(AGENT_B),
        "foreign-inspect",
        None,
        "t8",
        ProtocolCall::InspectExecution(EntityQuery {
            id: EXECUTION.into(),
        }),
    );
    failure(
        send(&mut fixture, foreign_inspect),
        ProtocolErrorCode::Forbidden,
    );
}

#[test]
fn completed_operation_is_durable_and_exact_finish_is_idempotent() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    success(send(
        &mut fixture,
        start_operation_request("request:complete-start", "operation:complete"),
    ));
    let finish = finish_operation_request(
        "request:complete-finish",
        "operation:complete",
        OperationTerminalStatus::Completed,
        None,
    );
    let first = operation(success(send(&mut fixture, finish.clone())));
    let retry = operation(success(send(&mut fixture, finish)));
    assert_eq!(retry, first);
    assert_eq!(first.state, "completed");
    assert_eq!(first.finished_at.as_deref(), Some("t7"));
}

fn terminal_failure(status: OperationTerminalStatus, expected: &str) {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    success(send(
        &mut fixture,
        start_operation_request("request:terminal-start", "operation:terminal"),
    ));
    let finished = operation(success(send(
        &mut fixture,
        finish_operation_request(
            "request:terminal-finish",
            "operation:terminal",
            status,
            Some(OperationFailureResource {
                code: "RUNTIME_OUTCOME".into(),
                retryable: false,
            }),
        ),
    )));
    assert_eq!(finished.state, expected);
    assert_eq!(finished.failure_code.as_deref(), Some("RUNTIME_OUTCOME"));
}

#[test]
fn failed_operation_is_explicit() {
    terminal_failure(OperationTerminalStatus::Failed, "failed");
}

#[test]
fn cancelled_operation_records_intent_without_claiming_process_termination() {
    terminal_failure(OperationTerminalStatus::Cancelled, "cancelled");
}

#[test]
fn unknown_operation_requires_later_reconciliation() {
    terminal_failure(OperationTerminalStatus::Unknown, "unknown");
}

#[test]
fn finish_payload_enforces_result_error_exclusivity() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    success(send(
        &mut fixture,
        start_operation_request("request:validate-start", "operation:validate"),
    ));
    let completed_with_error = finish_operation_request(
        "request:bad-complete",
        "operation:validate",
        OperationTerminalStatus::Completed,
        Some(OperationFailureResource {
            code: "IMPOSSIBLE".into(),
            retryable: false,
        }),
    );
    failure(
        send(&mut fixture, completed_with_error),
        ProtocolErrorCode::ValidationError,
    );
    let failed_without_error = finish_operation_request(
        "request:bad-failure",
        "operation:validate",
        OperationTerminalStatus::Failed,
        None,
    );
    failure(
        send(&mut fixture, failed_without_error),
        ProtocolErrorCode::ValidationError,
    );
}

#[test]
fn interrupted_execution_and_operation_survive_cold_reopen() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    success(send(
        &mut fixture,
        start_operation_request("request:cold", "operation:cold"),
    ));
    success(send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "interrupt",
            None,
            "t8",
            ProtocolCall::InterruptExecution(ExecutionOutcomeCommand {
                execution_id: EXECUTION.into(),
                expected_revision: 1,
                outcome_code: Some("connection_lost".into()),
            }),
        ),
    ));
    let root = fixture.repository_dir.path().to_path_buf();
    drop(fixture.repository);
    fixture.repository = Repository::open(root).expect("cold reopen");
    let inspection = match success(send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "inspect-after-reopen",
            None,
            "t9",
            ProtocolCall::InspectExecution(EntityQuery {
                id: EXECUTION.into(),
            }),
        ),
    )) {
        ProtocolResult::ExecutionInspection(value) => value,
        result => panic!("unexpected result: {result:?}"),
    };
    assert_eq!(inspection.execution.state, "interrupted");
    assert_eq!(
        inspection.execution.outcome_code.as_deref(),
        Some("connection_lost")
    );
    assert_eq!(inspection.operations.len(), 1);
    assert_eq!(inspection.operations[0].state, "started");
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
        Self {
            stdin: Some(child.stdin.take().expect("transport stdin")),
            stdout: BufReader::new(child.stdout.take().expect("transport stdout")),
            child,
        }
    }

    fn write(&mut self, value: &Value) {
        let stdin = self.stdin.as_mut().expect("open stdin");
        serde_json::to_writer(&mut *stdin, value).expect("write request");
        stdin.write_all(b"\n").expect("write delimiter");
        stdin.flush().expect("flush request");
    }

    fn call(&mut self, value: &Value) -> Value {
        self.write(value);
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read response");
        assert!(!line.is_empty(), "transport exited without response");
        serde_json::from_str(&line).expect("response JSON")
    }

    fn close(mut self) {
        drop(self.stdin.take());
        assert!(self.child.wait().expect("wait transport").success());
    }

    fn disconnect_without_reading(mut self) {
        drop(self.stdin.take());
        drop(self.stdout);
        let status = self.child.wait().expect("wait disconnected transport");
        assert!(status.success() || status.code() == Some(2));
    }
}

fn wire(
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

fn wire_ok(response: Value, kind: &str) -> Value {
    assert_eq!(response["status"], "ok", "{response:#}");
    assert_eq!(response["result"]["kind"], kind);
    response["result"]["data"].clone()
}

fn process_fixture() -> (TempDir, TempDir) {
    let repository_dir = tempdir().expect("repository directory");
    let workspace_root = tempdir().expect("workspace root");
    let mut repository = Repository::init(repository_dir.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            ENVIRONMENT,
            PROJECT,
            &json!({"schema_version": 1, "test": "process-reconnect"}),
            "t0",
        )
        .expect("environment");
    drop(repository);
    (repository_dir, workspace_root)
}

fn process_bootstrap(transport: &mut Transport) {
    wire_ok(
        transport.call(&wire(
            Some(AGENT_A),
            "register",
            None,
            "register_agent",
            Some(json!({
                "agent_id": AGENT_A,
                "provider_metadata": "runtime-neutral",
                "display_name": null
            })),
        )),
        "agent",
    );
    wire_ok(
        transport.call(&wire(
            Some(AGENT_A),
            "task",
            None,
            "create_task",
            Some(json!({
                "task_id": TASK,
                "project_id": PROJECT,
                "goal_ref": "goal:process-reconnect",
                "context_ref": null
            })),
        )),
        "task",
    );
    wire_ok(
        transport.call(&wire(
            Some(AGENT_A),
            "workspace",
            None,
            "create_workspace",
            Some(json!({
                "workspace_id": WORKSPACE,
                "project_id": PROJECT,
                "binding_ref": "runtime-a",
                "branch_ref": null,
                "environment_id": ENVIRONMENT
            })),
        )),
        "workspace",
    );
    wire_ok(
        transport.call(&wire(
            Some(AGENT_A),
            "execution",
            None,
            "create_execution",
            Some(json!({
                "execution_id": EXECUTION,
                "task_id": TASK,
                "parent_execution_id": null,
                "workspace_id": WORKSPACE,
                "base_version_id": null
            })),
        )),
        "execution",
    );
    wire_ok(
        transport.call(&wire(
            Some(AGENT_A),
            "execution-start",
            None,
            "start_execution",
            Some(json!({"execution_id": EXECUTION, "expected_revision": 0})),
        )),
        "execution",
    );
}

#[test]
fn lost_start_response_is_resolved_by_request_on_fresh_process() {
    let (repository_dir, workspace_root) = process_fixture();
    let mut first = Transport::start(repository_dir.path(), workspace_root.path());
    process_bootstrap(&mut first);
    first.write(&wire(
        Some(AGENT_A),
        "request:uncertain-start",
        Some("operation:uncertain-start"),
        "start_operation",
        Some(json!({"execution_id": EXECUTION, "action": "agent.work"})),
    ));
    first.disconnect_without_reading();

    let mut reconnected = Transport::start(repository_dir.path(), workspace_root.path());
    let resolved = wire_ok(
        reconnected.call(&wire(
            Some(AGENT_A),
            "resolve-after-loss",
            None,
            "resolve_operation",
            Some(json!({
                "project_id": PROJECT,
                "request_id": "request:uncertain-start"
            })),
        )),
        "operation",
    );
    assert_eq!(resolved["operation_id"], "operation:uncertain-start");
    assert_eq!(resolved["execution_id"], EXECUTION);
    assert_eq!(resolved["state"], "started");
    let retry = wire_ok(
        reconnected.call(&wire(
            Some(AGENT_A),
            "request:uncertain-start",
            Some("operation:uncertain-start"),
            "start_operation",
            Some(json!({"execution_id": EXECUTION, "action": "agent.work"})),
        )),
        "operation",
    );
    assert_eq!(retry, resolved);
    reconnected.close();
}

#[test]
fn lost_completion_response_is_observed_as_completed_on_fresh_process() {
    let (repository_dir, workspace_root) = process_fixture();
    let mut first = Transport::start(repository_dir.path(), workspace_root.path());
    process_bootstrap(&mut first);
    wire_ok(
        first.call(&wire(
            Some(AGENT_A),
            "request:completion-start",
            Some("operation:completion"),
            "start_operation",
            Some(json!({"execution_id": EXECUTION, "action": "agent.work"})),
        )),
        "operation",
    );
    first.write(&wire(
        Some(AGENT_A),
        "request:completion-finish",
        Some("operation:completion"),
        "finish_operation",
        Some(json!({
            "execution_id": EXECUTION,
            "status": "COMPLETED",
            "failure": null
        })),
    ));
    first.disconnect_without_reading();

    let mut reconnected = Transport::start(repository_dir.path(), workspace_root.path());
    let completed = wire_ok(
        reconnected.call(&wire(
            Some(AGENT_A),
            "get-after-completion-loss",
            None,
            "get_operation",
            Some(json!({"id": "operation:completion"})),
        )),
        "operation",
    );
    assert_eq!(completed["state"], "completed");
    assert_eq!(completed["recording_state"], "durable");
    assert_eq!(completed["execution_id"], EXECUTION);
    reconnected.close();
}
