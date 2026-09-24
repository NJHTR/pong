//! The external capability list is an allowlist, not an export of Rust APIs.

use pong_core::protocol::{
    AcquireLeaseCommand, CreateExecutionCommand, CreateTaskCommand, CreateWorkspaceCommand,
    EntityQuery, ExecutionRevisionCommand, ExternalAgentProtocol, ProtocolCall, ProtocolErrorCode,
    ProtocolRequest, ProtocolResponse, ProtocolResult, ProtocolStatus, RegisterAgentCommand,
    WorkspaceBindingError, WorkspaceBindingResolver, EXTERNAL_AGENT_PROTOCOL_VERSION,
};
use pong_core::{RemoteAccessBoundary, Repository};
use serde_json::{json, Value};
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};

const COMMANDS: &[&str] = &[
    "register_agent",
    "create_task",
    "create_workspace",
    "create_execution",
    "start_execution",
    "pause_execution",
    "complete_execution",
    "fail_execution",
    "interrupt_execution",
    "acquire_workspace_lease",
    "renew_workspace_lease",
    "release_workspace_lease",
    "publish_version",
    "set_execution_current_version",
    "create_checkpoint",
    "resume_from_checkpoint",
    "create_handoff",
    "materialize_version",
    "start_operation",
    "finish_operation",
];

const QUERIES: &[&str] = &[
    "hello",
    "get_agent",
    "get_task",
    "get_execution",
    "get_workspace",
    "get_version",
    "get_checkpoint",
    "get_handoff",
    "get_operation",
    "resolve_operation",
    "inspect_execution",
    "list_checkpoints",
    "list_handoffs",
    "diff_workspace_version",
];

struct Bindings(PathBuf);

impl WorkspaceBindingResolver for Bindings {
    fn resolve(&self, binding_ref: &str) -> Result<PathBuf, WorkspaceBindingError> {
        if binding_ref == "workspace-a" {
            Ok(self.0.clone())
        } else {
            Err(WorkspaceBindingError::NotFound)
        }
    }
}

struct Fixture {
    _repository_dir: TempDir,
    _workspace_dir: TempDir,
    repository: Repository,
    bindings: Bindings,
}

impl Fixture {
    fn new() -> Self {
        let repository_dir = tempdir().unwrap();
        let workspace_dir = tempdir().unwrap();
        let repository = Repository::init(repository_dir.path()).unwrap();
        let bindings = Bindings(workspace_dir.path().join("workspace-a"));
        Self {
            _repository_dir: repository_dir,
            _workspace_dir: workspace_dir,
            repository,
            bindings,
        }
    }

    fn send(&mut self, request: ProtocolRequest) -> ProtocolResponse {
        ExternalAgentProtocol::new(&mut self.repository, &self.bindings).handle(request, 10)
    }

    fn register(&mut self, agent: &str) {
        let response = self.send(request(
            Some(agent),
            &format!("register-{agent}"),
            ProtocolCall::RegisterAgent(RegisterAgentCommand {
                agent_id: agent.into(),
                provider_metadata: None,
                display_name: None,
            }),
        ));
        assert_eq!(response.status, ProtocolStatus::Ok, "{response:?}");
    }
}

fn request(caller: Option<&str>, id: &str, call: ProtocolCall) -> ProtocolRequest {
    ProtocolRequest {
        protocol_version: EXTERNAL_AGENT_PROTOCOL_VERSION.into(),
        request_id: id.into(),
        caller_agent_id: caller.map(str::to_owned),
        issued_at: "2026-09-24T00:00:00Z".into(),
        operation_id: None,
        call,
    }
}

fn error_code(response: ProtocolResponse) -> ProtocolErrorCode {
    assert_eq!(response.status, ProtocolStatus::Error);
    response.error.unwrap().code
}

#[test]
fn hello_advertises_exactly_the_v1_external_surface() {
    let mut fixture = Fixture::new();
    let response = fixture.send(request(None, "hello", ProtocolCall::Hello));
    let ProtocolResult::Hello(hello) = response.result.unwrap() else {
        panic!("expected hello capability result");
    };
    assert_eq!(hello.protocol_versions, ["1.0"]);
    assert_eq!(hello.commands, COMMANDS);
    assert_eq!(hello.queries, QUERIES);
    assert!(hello.provider_neutral && hello.transport_neutral);
    assert_eq!(hello.mutation_acknowledgement, "durable_or_error");

    let advertised = serde_json::to_string(&hello).unwrap();
    for internal in [
        "rollback",
        "restore",
        "recover_unfinished_operations",
        "cancel",
        "sdk",
        "mcp",
        "tls",
    ] {
        assert!(
            !advertised.contains(internal),
            "internal/deferred capability advertised: {internal}"
        );
    }
}

#[test]
fn unknown_command_and_unknown_field_fail_closed_without_a_v2_upgrade() {
    let old_style = json!({
        "protocol_version": "1.0",
        "request_id": "old-client",
        "caller_agent_id": null,
        "issued_at": "2026-09-24T00:00:00Z",
        "operation": "hello"
    });
    let old_request: ProtocolRequest = serde_json::from_value(old_style.clone()).unwrap();
    let mut fixture = Fixture::new();
    assert_eq!(fixture.send(old_request).status, ProtocolStatus::Ok);

    for command in ["rollback", "restore", "cancel_execution", "future_command"] {
        let mut value = old_style.clone();
        value["operation"] = Value::String(command.into());
        assert!(
            serde_json::from_value::<ProtocolRequest>(value).is_err(),
            "{command} was unexpectedly accepted"
        );
    }
    let mut unknown_field = old_style;
    unknown_field["internal_authority"] = json!(true);
    assert!(serde_json::from_value::<ProtocolRequest>(unknown_field).is_err());
    assert_eq!(
        ProtocolResponse::invalid_envelope(Some("future-client".into()))
            .error
            .unwrap()
            .code,
        ProtocolErrorCode::ValidationError
    );

    let mut future_version = request(None, "future-version", ProtocolCall::Hello);
    future_version.protocol_version = "2.0".into();
    assert_eq!(
        error_code(fixture.send(future_version)),
        ProtocolErrorCode::UnsupportedVersion
    );
}

#[test]
fn principal_binding_and_execution_ownership_guard_control_capabilities() {
    let mut fixture = Fixture::new();
    assert_eq!(
        error_code(fixture.send(request(
            Some("unknown"),
            "unknown-agent",
            ProtocolCall::GetAgent(EntityQuery {
                id: "agent-a".into()
            })
        ))),
        ProtocolErrorCode::Unauthorized
    );
    fixture.register("agent-a");
    fixture.register("agent-b");
    for (id, call) in [
        (
            "task",
            ProtocolCall::CreateTask(CreateTaskCommand {
                task_id: "task-a".into(),
                project_id: "project-a".into(),
                goal_ref: "goal:a".into(),
                context_ref: None,
            }),
        ),
        (
            "workspace",
            ProtocolCall::CreateWorkspace(CreateWorkspaceCommand {
                workspace_id: "workspace-a".into(),
                project_id: "project-a".into(),
                binding_ref: "workspace-a".into(),
                branch_ref: None,
                environment_id: None,
            }),
        ),
        (
            "execution",
            ProtocolCall::CreateExecution(CreateExecutionCommand {
                execution_id: "execution-a".into(),
                task_id: "task-a".into(),
                parent_execution_id: None,
                workspace_id: Some("workspace-a".into()),
                base_version_id: None,
            }),
        ),
    ] {
        let response = fixture.send(request(Some("agent-a"), id, call));
        assert_eq!(response.status, ProtocolStatus::Ok, "{response:?}");
    }
    assert_eq!(
        error_code(fixture.send(request(
            Some("agent-b"),
            "foreign-execution",
            ProtocolCall::StartExecution(ExecutionRevisionCommand {
                execution_id: "execution-a".into(),
                expected_revision: 0,
            })
        ))),
        ProtocolErrorCode::Forbidden
    );
    assert_eq!(
        error_code(fixture.send(request(
            Some("agent-b"),
            "foreign-lease",
            ProtocolCall::AcquireWorkspaceLease(AcquireLeaseCommand {
                execution_id: "execution-a".into(),
                workspace_id: "workspace-a".into(),
                ttl_ms: 1000,
            })
        ))),
        ProtocolErrorCode::Forbidden
    );
    let workspace = fixture
        .repository
        .metadata()
        .workspace("workspace-a")
        .unwrap()
        .unwrap();
    assert_eq!(workspace.revision, 0);
    assert!(fixture
        .repository
        .metadata()
        .workspace_lease("workspace-a")
        .unwrap()
        .is_none());
}

#[test]
fn registered_agent_read_scope_is_broader_than_execution_write_scope() {
    let mut fixture = Fixture::new();
    fixture.register("agent-a");
    fixture.register("agent-b");
    assert_eq!(
        fixture
            .send(request(
                Some("agent-a"),
                "create-workspace",
                ProtocolCall::CreateWorkspace(CreateWorkspaceCommand {
                    workspace_id: "workspace-a".into(),
                    project_id: "project-a".into(),
                    binding_ref: "workspace-a".into(),
                    branch_ref: None,
                    environment_id: None,
                })
            ))
            .status,
        ProtocolStatus::Ok
    );
    let response = fixture.send(request(
        Some("agent-b"),
        "read-foreign-workspace",
        ProtocolCall::GetWorkspace(EntityQuery {
            id: "workspace-a".into(),
        }),
    ));
    assert_eq!(response.status, ProtocolStatus::Ok, "{response:?}");
    assert!(matches!(
        response.result,
        Some(ProtocolResult::WorkspaceInspection(_))
    ));
}

#[test]
fn remote_reconciliation_is_not_an_advertised_operator_recovery_command() {
    let capabilities = RemoteAccessBoundary::new().capabilities();
    assert!(capabilities.reauthentication_required_on_reconnect);
    assert!(capabilities.core_restart_invalidates_sessions);
    assert!(!capabilities.disconnect_cancels_operations);
    assert!(capabilities.durable_state_reconciliation);

    let mut fixture = Fixture::new();
    let ProtocolResult::Hello(hello) = fixture
        .send(request(None, "hello", ProtocolCall::Hello))
        .result
        .unwrap()
    else {
        panic!("expected hello");
    };
    assert!(hello.queries.contains(&"resolve_operation".into()));
    assert!(!hello
        .commands
        .contains(&"recover_unfinished_operations".into()));
}
