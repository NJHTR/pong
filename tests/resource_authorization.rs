//! Protocol v1.0 authorization within one trusted collaboration domain.

use pong_core::protocol::{
    AcquireLeaseCommand, CreateCheckpointCommand, CreateExecutionCommand, CreateHandoffCommand,
    CreateTaskCommand, CreateWorkspaceCommand, DiffWorkspaceVersionQuery, EntityQuery,
    ExecutionRevisionCommand, ExternalAgentProtocol, FinishOperationCommand,
    MaterializeVersionCommand, OperationTerminalStatus, ProtocolCall, ProtocolErrorCode,
    ProtocolRequest, ProtocolResponse, ProtocolResult, ProtocolStatus, PublishVersionCommand,
    RegisterAgentCommand, ResumeCheckpointCommand, StartOperationCommand, TaskQuery,
    WorkspaceBindingError, WorkspaceBindingResolver, EXTERNAL_AGENT_PROTOCOL_VERSION,
};
use pong_core::Repository;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

const A: &str = "agent-a";
const B: &str = "agent-b";
const TASK: &str = "task-a";
const WORKSPACE: &str = "workspace-a";
const ENVIRONMENT: &str = "environment-a";
const EXECUTION: &str = "execution-a";
const OPERATION: &str = "operation-a";
const CHECKPOINT: &str = "checkpoint-a";

struct Bindings(PathBuf);

impl WorkspaceBindingResolver for Bindings {
    fn resolve(&self, reference: &str) -> Result<PathBuf, WorkspaceBindingError> {
        if reference == WORKSPACE {
            Ok(self.0.join(reference))
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
    version_id: String,
    lease: pong_core::protocol::LeaseAuthority,
}

fn request(
    caller: &str,
    id: &str,
    operation_id: Option<&str>,
    call: ProtocolCall,
) -> ProtocolRequest {
    ProtocolRequest {
        protocol_version: EXTERNAL_AGENT_PROTOCOL_VERSION.into(),
        request_id: id.into(),
        caller_agent_id: Some(caller.into()),
        issued_at: format!("test:{id}"),
        operation_id: operation_id.map(str::to_owned),
        call,
    }
}

fn ok(response: ProtocolResponse) -> ProtocolResult {
    assert_eq!(response.status, ProtocolStatus::Ok, "{response:?}");
    response.result.unwrap()
}

fn denied(response: ProtocolResponse, expected: ProtocolErrorCode) {
    assert_eq!(response.status, ProtocolStatus::Error, "{response:?}");
    assert_eq!(response.error.unwrap().code, expected);
}

impl Fixture {
    fn new() -> Self {
        let repository_dir = tempdir().unwrap();
        let workspace_dir = tempdir().unwrap();
        let mut repository = Repository::init(repository_dir.path()).unwrap();
        repository
            .metadata_mut()
            .record_environment(
                ENVIRONMENT,
                "project",
                &json!({"schema_version": 1, "test": "resource-authorization"}),
                "resource-authorization",
            )
            .unwrap();
        let mut fixture = Self {
            _repository_dir: repository_dir,
            bindings: Bindings(workspace_dir.path().to_owned()),
            _workspace_dir: workspace_dir,
            repository,
            version_id: String::new(),
            lease: pong_core::protocol::LeaseAuthority {
                workspace_id: WORKSPACE.into(),
                agent_id: A.into(),
                epoch: 0,
                expires_at_ms: 0,
            },
        };
        for agent in [A, B] {
            ok(fixture.send(
                agent,
                &format!("register-{agent}"),
                None,
                ProtocolCall::RegisterAgent(RegisterAgentCommand {
                    agent_id: agent.into(),
                    provider_metadata: None,
                    display_name: None,
                }),
            ));
        }
        ok(fixture.send(
            A,
            "task",
            None,
            ProtocolCall::CreateTask(CreateTaskCommand {
                task_id: TASK.into(),
                project_id: "project".into(),
                goal_ref: "goal".into(),
                context_ref: None,
            }),
        ));
        ok(fixture.send(
            A,
            "workspace",
            None,
            ProtocolCall::CreateWorkspace(CreateWorkspaceCommand {
                workspace_id: WORKSPACE.into(),
                project_id: "project".into(),
                binding_ref: WORKSPACE.into(),
                branch_ref: None,
                environment_id: Some(ENVIRONMENT.into()),
            }),
        ));
        ok(fixture.send(
            A,
            "execution",
            None,
            ProtocolCall::CreateExecution(CreateExecutionCommand {
                execution_id: EXECUTION.into(),
                task_id: TASK.into(),
                parent_execution_id: None,
                workspace_id: Some(WORKSPACE.into()),
                base_version_id: None,
            }),
        ));
        ok(fixture.send(
            A,
            "start",
            None,
            ProtocolCall::StartExecution(ExecutionRevisionCommand {
                execution_id: EXECUTION.into(),
                expected_revision: 0,
            }),
        ));
        ok(fixture.send(
            A,
            "operation",
            Some(OPERATION),
            ProtocolCall::StartOperation(StartOperationCommand {
                execution_id: EXECUTION.into(),
                action: "test".into(),
            }),
        ));
        ok(fixture.send(
            A,
            "finish-operation",
            Some(OPERATION),
            ProtocolCall::FinishOperation(FinishOperationCommand {
                execution_id: EXECUTION.into(),
                status: OperationTerminalStatus::Completed,
                failure: None,
            }),
        ));
        let ProtocolResult::Lease(lease) = ok(fixture.send(
            A,
            "lease",
            None,
            ProtocolCall::AcquireWorkspaceLease(AcquireLeaseCommand {
                execution_id: EXECUTION.into(),
                workspace_id: WORKSPACE.into(),
                ttl_ms: 60_000,
            }),
        )) else {
            panic!("lease expected");
        };
        fixture.lease = lease.authority;
        fs::write(
            fixture.workspace_path().join("document.txt"),
            "private bytes",
        )
        .unwrap();
        let ProtocolResult::VersionPublication(publication) = ok(fixture.send(
            A,
            "publication",
            Some("publication-operation"),
            ProtocolCall::PublishVersion(PublishVersionCommand {
                execution_id: EXECUTION.into(),
                workspace_id: WORKSPACE.into(),
                lease: fixture.lease.clone(),
                expected_workspace_revision: 0,
                parent_version_id: None,
                update_version_head: true,
            }),
        )) else {
            panic!("publication expected");
        };
        fixture.version_id = publication.version.version_id;
        ok(fixture.send(
            A,
            "checkpoint",
            None,
            ProtocolCall::CreateCheckpoint(CreateCheckpointCommand {
                checkpoint_id: CHECKPOINT.into(),
                task_id: TASK.into(),
                execution_id: EXECUTION.into(),
                workspace_id: WORKSPACE.into(),
                version_id: fixture.version_id.clone(),
                source_operation_id: Some("publication-operation".into()),
                reason_code: "ready".into(),
            }),
        ));
        fixture
    }

    fn workspace_path(&self) -> PathBuf {
        self.bindings.0.join(WORKSPACE)
    }

    fn send(
        &mut self,
        caller: &str,
        id: &str,
        operation_id: Option<&str>,
        call: ProtocolCall,
    ) -> ProtocolResponse {
        ExternalAgentProtocol::new(&mut self.repository, &self.bindings)
            .handle(request(caller, id, operation_id, call), 10)
    }
}

fn query(id: &str) -> EntityQuery {
    EntityQuery { id: id.into() }
}

fn assert_no_host_path(result: &ProtocolResult, fixture: &Fixture) {
    let text = serde_json::to_string(result).unwrap();
    for path in [
        fixture._repository_dir.path(),
        fixture._workspace_dir.path(),
        fixture.workspace_path().as_path(),
    ] {
        assert!(
            !text.contains(&path.to_string_lossy().to_string()),
            "{text}"
        );
    }
    for field in ["\"locator\"", "\"driver\"", "private bytes"] {
        assert!(!text.contains(field), "{text}");
    }
}

#[test]
fn shared_workspace_read_is_detailed_but_not_a_lease_or_filesystem_grant() {
    let mut fixture = Fixture::new();
    for caller in [A, B] {
        let response = ok(fixture.send(
            caller,
            &format!("workspace-{caller}"),
            None,
            ProtocolCall::GetWorkspace(query(WORKSPACE)),
        ));
        let ProtocolResult::WorkspaceInspection(inspection) = &response else {
            panic!("workspace inspection expected");
        };
        assert_eq!(inspection.workspace.revision, 2);
        assert!(inspection.workspace.head.is_some());
        assert!(inspection.workspace.version_head_id.is_some());
        assert_eq!(inspection.lease.as_ref().unwrap().authority.agent_id, A);
        assert_no_host_path(&response, &fixture);
    }
}

#[test]
fn shared_task_version_checkpoint_and_lists_are_readable_but_not_private_execution() {
    let mut fixture = Fixture::new();
    for (id, call) in [
        ("task", ProtocolCall::GetTask(query(TASK))),
        (
            "version",
            ProtocolCall::GetVersion(query(&fixture.version_id)),
        ),
        ("checkpoint", ProtocolCall::GetCheckpoint(query(CHECKPOINT))),
        (
            "checkpoints",
            ProtocolCall::ListCheckpoints(TaskQuery {
                task_id: TASK.into(),
            }),
        ),
        (
            "handoffs",
            ProtocolCall::ListHandoffs(TaskQuery {
                task_id: TASK.into(),
            }),
        ),
    ] {
        let result = ok(fixture.send(B, id, None, call));
        assert_no_host_path(&result, &fixture);
    }
    for (id, call) in [
        ("execution", ProtocolCall::GetExecution(query(EXECUTION))),
        ("inspect", ProtocolCall::InspectExecution(query(EXECUTION))),
        ("operation", ProtocolCall::GetOperation(query(OPERATION))),
    ] {
        denied(
            fixture.send(B, id, None, call),
            ProtocolErrorCode::Forbidden,
        );
    }
    ok(fixture.send(
        A,
        "own-inspect",
        None,
        ProtocolCall::InspectExecution(query(EXECUTION)),
    ));
    ok(fixture.send(
        A,
        "own-operation",
        None,
        ProtocolCall::GetOperation(query(OPERATION)),
    ));
}

#[test]
fn diff_is_shared_metadata_with_relative_paths_and_no_file_contents() {
    let mut fixture = Fixture::new();
    fs::write(
        fixture.workspace_path().join("document.txt"),
        "changed bytes",
    )
    .unwrap();
    for caller in [A, B] {
        let version_id = fixture.version_id.clone();
        let result = ok(fixture.send(
            caller,
            &format!("diff-{caller}"),
            None,
            ProtocolCall::DiffWorkspaceVersion(DiffWorkspaceVersionQuery {
                workspace_id: WORKSPACE.into(),
                source_version_id: version_id,
            }),
        ));
        let ProtocolResult::Diff(diff) = &result else {
            panic!("diff expected");
        };
        assert_eq!(diff.entries.len(), 1);
        assert_eq!(diff.entries[0].path, "document.txt");
        assert_no_host_path(&result, &fixture);
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("changed bytes"));
    }
}

#[test]
fn foreign_execution_control_rejection_does_not_acquire_lease_or_mutate_state() {
    let mut fixture = Fixture::new();
    let before = fixture
        .repository
        .metadata()
        .execution(EXECUTION)
        .unwrap()
        .unwrap();
    let workspace_before = fixture
        .repository
        .metadata()
        .workspace(WORKSPACE)
        .unwrap()
        .unwrap();
    let lease_before = fixture
        .repository
        .metadata()
        .workspace_lease(WORKSPACE)
        .unwrap();
    for (id, call) in [
        (
            "transition",
            ProtocolCall::StartExecution(ExecutionRevisionCommand {
                execution_id: EXECUTION.into(),
                expected_revision: before.revision,
            }),
        ),
        (
            "lease",
            ProtocolCall::AcquireWorkspaceLease(AcquireLeaseCommand {
                execution_id: EXECUTION.into(),
                workspace_id: WORKSPACE.into(),
                ttl_ms: 1000,
            }),
        ),
        (
            "checkpoint",
            ProtocolCall::CreateCheckpoint(CreateCheckpointCommand {
                checkpoint_id: "foreign-checkpoint".into(),
                task_id: TASK.into(),
                execution_id: EXECUTION.into(),
                workspace_id: WORKSPACE.into(),
                version_id: fixture.version_id.clone(),
                source_operation_id: None,
                reason_code: "denied".into(),
            }),
        ),
        (
            "materialize",
            ProtocolCall::MaterializeVersion(MaterializeVersionCommand {
                execution_id: EXECUTION.into(),
                workspace_id: WORKSPACE.into(),
                source_version_id: fixture.version_id.clone(),
                lease: fixture.lease.clone(),
                expected_workspace_revision: 1,
            }),
        ),
    ] {
        denied(
            fixture.send(B, id, None, call),
            ProtocolErrorCode::Forbidden,
        );
    }
    assert_eq!(
        fixture
            .repository
            .metadata()
            .execution(EXECUTION)
            .unwrap()
            .unwrap(),
        before
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace_lease(WORKSPACE)
            .unwrap(),
        lease_before
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace(WORKSPACE)
            .unwrap()
            .unwrap(),
        workspace_before
    );
    assert!(fixture
        .repository
        .metadata()
        .checkpoint("foreign-checkpoint")
        .unwrap()
        .is_none());
    ok(fixture.send(
        A,
        "after-forbidden",
        None,
        ProtocolCall::GetExecution(query(EXECUTION)),
    ));
}

#[test]
fn checkpoint_resume_is_cross_agent_continuation_not_foreign_execution_control() {
    let mut fixture = Fixture::new();
    let result = ok(fixture.send(
        B,
        "resume-b",
        None,
        ProtocolCall::ResumeFromCheckpoint(ResumeCheckpointCommand {
            execution_id: "execution-b".into(),
            task_id: TASK.into(),
            parent_execution_id: Some(EXECUTION.into()),
            workspace_id: WORKSPACE.into(),
            checkpoint_id: CHECKPOINT.into(),
        }),
    ));
    assert!(matches!(result, ProtocolResult::Resume(_)));
    denied(
        fixture.send(
            B,
            "foreign-inspect",
            None,
            ProtocolCall::InspectExecution(query(EXECUTION)),
        ),
        ProtocolErrorCode::Forbidden,
    );
    ok(fixture.send(
        B,
        "own-resume",
        None,
        ProtocolCall::GetExecution(query("execution-b")),
    ));
}

#[test]
fn handoff_is_shared_observation_but_source_execution_controls_creation() {
    let mut fixture = Fixture::new();
    denied(
        fixture.send(
            B,
            "foreign-handoff",
            None,
            ProtocolCall::CreateHandoff(CreateHandoffCommand {
                handoff_id: "foreign".into(),
                task_id: TASK.into(),
                from_execution_id: EXECUTION.into(),
                to_execution_id: "missing".into(),
                source_version_id: None,
                checkpoint_id: None,
                reason_code: "denied".into(),
            }),
        ),
        ProtocolErrorCode::Forbidden,
    );
    assert!(fixture
        .repository
        .metadata()
        .handoff("foreign")
        .unwrap()
        .is_none());
    ok(fixture.send(
        B,
        "resume-for-handoff",
        None,
        ProtocolCall::ResumeFromCheckpoint(ResumeCheckpointCommand {
            execution_id: "execution-b".into(),
            task_id: TASK.into(),
            parent_execution_id: Some(EXECUTION.into()),
            workspace_id: WORKSPACE.into(),
            checkpoint_id: CHECKPOINT.into(),
        }),
    ));
    let version = fixture.version_id.clone();
    ok(fixture.send(
        A,
        "owner-handoff",
        None,
        ProtocolCall::CreateHandoff(CreateHandoffCommand {
            handoff_id: "handoff-a-b".into(),
            task_id: TASK.into(),
            from_execution_id: EXECUTION.into(),
            to_execution_id: "execution-b".into(),
            source_version_id: Some(version),
            checkpoint_id: Some(CHECKPOINT.into()),
            reason_code: "continuation".into(),
        }),
    ));
    let observed = ok(fixture.send(
        B,
        "read-handoff",
        None,
        ProtocolCall::GetHandoff(query("handoff-a-b")),
    ));
    assert!(matches!(observed, ProtocolResult::Handoff(_)));
    assert_no_host_path(&observed, &fixture);
    let listed = ok(fixture.send(
        B,
        "list-handoffs",
        None,
        ProtocolCall::ListHandoffs(TaskQuery {
            task_id: TASK.into(),
        }),
    ));
    let ProtocolResult::Handoffs(listed) = listed else {
        panic!("handoff list expected");
    };
    assert_eq!(listed.items.len(), 1);
}

#[test]
fn hello_is_support_discovery_not_an_authorization_grant() {
    let mut fixture = Fixture::new();
    let hello = ExternalAgentProtocol::new(&mut fixture.repository, &fixture.bindings).handle(
        ProtocolRequest {
            caller_agent_id: None,
            call: ProtocolCall::Hello,
            ..request(A, "hello", None, ProtocolCall::Hello)
        },
        10,
    );
    let ProtocolResult::Hello(hello) = ok(hello) else {
        panic!("hello expected");
    };
    assert!(hello.queries.contains(&"get_execution".into()));
    assert!(hello.queries.contains(&"diff_workspace_version".into()));
    assert!(!hello.commands.contains(&"rollback".into()));
    assert!(!hello.commands.contains(&"restore".into()));
    denied(
        fixture.send(
            B,
            "not-granted",
            None,
            ProtocolCall::GetExecution(query(EXECUTION)),
        ),
        ProtocolErrorCode::Forbidden,
    );
}

#[test]
fn unregistered_agent_cannot_observe_shared_resources() {
    let mut fixture = Fixture::new();
    for call in [
        ProtocolCall::GetWorkspace(query(WORKSPACE)),
        ProtocolCall::GetVersion(query(&fixture.version_id)),
        ProtocolCall::GetCheckpoint(query(CHECKPOINT)),
        ProtocolCall::DiffWorkspaceVersion(DiffWorkspaceVersionQuery {
            workspace_id: WORKSPACE.into(),
            source_version_id: fixture.version_id.clone(),
        }),
    ] {
        denied(
            fixture.send("unknown", "unregistered", None, call),
            ProtocolErrorCode::Unauthorized,
        );
    }
}

#[test]
fn wire_read_does_not_expose_repository_or_workspace_locator() {
    let mut fixture = Fixture::new();
    let workspace = fixture.workspace_path();
    assert!(Path::new(&workspace).is_absolute());
    for call in [
        ProtocolCall::GetWorkspace(query(WORKSPACE)),
        ProtocolCall::GetVersion(query(&fixture.version_id)),
        ProtocolCall::GetCheckpoint(query(CHECKPOINT)),
        ProtocolCall::InspectExecution(query(EXECUTION)),
    ] {
        let result = ok(fixture.send(A, "path-audit", None, call));
        assert_no_host_path(&result, &fixture);
    }
}
