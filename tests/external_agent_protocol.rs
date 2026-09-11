//! Transport-neutral contract coverage for the external Agent protocol.

use pong_core::protocol::*;
use pong_core::Repository;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "project-protocol";
const ENVIRONMENT: &str = "environment-protocol";
const AGENT_A: &str = "agent-runtime-a";
const AGENT_B: &str = "agent-runtime-b";
const TASK: &str = "task-protocol";
const W1: &str = "workspace-runtime-a";
const W2: &str = "workspace-runtime-b";
const E1: &str = "execution-runtime-a";
const E2: &str = "execution-runtime-b";

struct Bindings {
    paths: HashMap<String, PathBuf>,
}

impl WorkspaceBindingResolver for Bindings {
    fn resolve(&self, binding_ref: &str) -> Result<PathBuf, WorkspaceBindingError> {
        self.paths
            .get(binding_ref)
            .cloned()
            .ok_or(WorkspaceBindingError::NotFound)
    }
}

struct Fixture {
    repository_dir: TempDir,
    workspace_dir: TempDir,
    repository: Repository,
    bindings: Bindings,
    w1_path: PathBuf,
    w2_path: PathBuf,
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
            &json!({"schema_version": 1, "test": "external-agent-protocol"}),
            "t0",
        )
        .expect("environment");
    let w1_path = workspace_dir.path().join("runtime-a");
    let w2_path = workspace_dir.path().join("runtime-b");
    let bindings = Bindings {
        paths: HashMap::from([
            ("binding:runtime-a".into(), w1_path.clone()),
            ("binding:runtime-b".into(), w2_path.clone()),
        ]),
    };
    Fixture {
        repository_dir,
        workspace_dir,
        repository,
        bindings,
        w1_path,
        w2_path,
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

fn send(fixture: &mut Fixture, request: ProtocolRequest, now_ms: i64) -> ProtocolResponse {
    ExternalAgentProtocol::new(&mut fixture.repository, &fixture.bindings).handle(request, now_ms)
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

fn register(fixture: &mut Fixture, agent_id: &str, provider: &str) -> AgentResource {
    match success(send(
        fixture,
        request(
            Some(agent_id),
            &format!("request:register:{agent_id}"),
            None,
            "t1",
            ProtocolCall::RegisterAgent(RegisterAgentCommand {
                agent_id: agent_id.into(),
                provider_metadata: Some(provider.into()),
                display_name: None,
            }),
        ),
        1,
    )) {
        ProtocolResult::Agent(agent) => agent,
        result => panic!("unexpected result: {result:?}"),
    }
}

fn bootstrap(fixture: &mut Fixture) {
    register(fixture, AGENT_A, "runtime-a");
    register(fixture, AGENT_B, "runtime-b");
    success(send(
        fixture,
        request(
            Some(AGENT_A),
            "request:create-task",
            None,
            "t2",
            ProtocolCall::CreateTask(CreateTaskCommand {
                task_id: TASK.into(),
                project_id: PROJECT.into(),
                goal_ref: "goal:protocol-handoff".into(),
                context_ref: Some("context:protocol-contract".into()),
            }),
        ),
        2,
    ));
    for (caller, workspace_id, binding_ref) in [
        (AGENT_A, W1, "binding:runtime-a"),
        (AGENT_B, W2, "binding:runtime-b"),
    ] {
        success(send(
            fixture,
            request(
                Some(caller),
                &format!("request:create:{workspace_id}"),
                None,
                "t3",
                ProtocolCall::CreateWorkspace(CreateWorkspaceCommand {
                    workspace_id: workspace_id.into(),
                    project_id: PROJECT.into(),
                    binding_ref: binding_ref.into(),
                    branch_ref: None,
                    environment_id: Some(ENVIRONMENT.into()),
                }),
            ),
            3,
        ));
    }
    success(send(
        fixture,
        request(
            Some(AGENT_A),
            "request:create-e1",
            None,
            "t4",
            ProtocolCall::CreateExecution(CreateExecutionCommand {
                execution_id: E1.into(),
                task_id: TASK.into(),
                parent_execution_id: None,
                workspace_id: Some(W1.into()),
                base_version_id: None,
            }),
        ),
        4,
    ));
}

fn start_execution(
    fixture: &mut Fixture,
    agent: &str,
    execution: &str,
    expected_revision: i64,
    request_id: &str,
) -> ExecutionResource {
    match success(send(
        fixture,
        request(
            Some(agent),
            request_id,
            None,
            "t5",
            ProtocolCall::StartExecution(ExecutionRevisionCommand {
                execution_id: execution.into(),
                expected_revision,
            }),
        ),
        5,
    )) {
        ProtocolResult::Execution(execution) => execution,
        result => panic!("unexpected result: {result:?}"),
    }
}

fn acquire(
    fixture: &mut Fixture,
    agent: &str,
    execution: &str,
    workspace: &str,
    now_ms: i64,
) -> LeaseAuthority {
    match success(send(
        fixture,
        request(
            Some(agent),
            &format!("request:lease:{execution}"),
            None,
            "t6",
            ProtocolCall::AcquireWorkspaceLease(AcquireLeaseCommand {
                execution_id: execution.into(),
                workspace_id: workspace.into(),
                ttl_ms: 100_000,
            }),
        ),
        now_ms,
    )) {
        ProtocolResult::Lease(lease) => lease.authority,
        result => panic!("unexpected result: {result:?}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn publish(
    fixture: &mut Fixture,
    agent: &str,
    execution: &str,
    workspace: &str,
    lease: &LeaseAuthority,
    expected_workspace_revision: i64,
    request_id: &str,
    operation_id: &str,
    issued_at: &str,
) -> VersionPublicationResource {
    match success(send(
        fixture,
        request(
            Some(agent),
            request_id,
            Some(operation_id),
            issued_at,
            ProtocolCall::PublishVersion(PublishVersionCommand {
                execution_id: execution.into(),
                workspace_id: workspace.into(),
                lease: lease.clone(),
                expected_workspace_revision,
                parent_version_id: None,
                update_version_head: true,
            }),
        ),
        20,
    )) {
        ProtocolResult::VersionPublication(publication) => *publication,
        result => panic!("unexpected result: {result:?}"),
    }
}

#[test]
fn hello_negotiates_one_transport_and_provider_neutral_version() {
    let mut fixture = fixture();
    let result = success(send(
        &mut fixture,
        request(None, "request:hello", None, "t0", ProtocolCall::Hello),
        0,
    ));
    let ProtocolResult::Hello(hello) = result else {
        panic!("unexpected result: {result:?}");
    };
    assert_eq!(hello.protocol_versions, ["1.0"]);
    assert!(hello.commands.contains(&"create_handoff".into()));
    assert!(hello.queries.contains(&"inspect_execution".into()));
    assert_eq!(hello.mutation_acknowledgement, "durable_or_error");
    assert!(hello.provider_neutral);
    assert!(hello.transport_neutral);

    let mut unsupported = request(None, "request:old", None, "t0", ProtocolCall::Hello);
    unsupported.protocol_version = "0.9".into();
    failure(
        send(&mut fixture, unsupported, 0),
        ProtocolErrorCode::UnsupportedVersion,
    );
}

#[test]
fn json_schema_is_explicit_strict_and_round_trips() {
    let value = json!({
        "protocol_version": "1.0",
        "request_id": "request:json",
        "caller_agent_id": null,
        "issued_at": "t0",
        "operation_id": null,
        "operation": "hello"
    });
    let decoded: ProtocolRequest = serde_json::from_value(value.clone()).expect("request JSON");
    assert_eq!(decoded.call, ProtocolCall::Hello);
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);

    let mut unknown = value;
    unknown
        .as_object_mut()
        .unwrap()
        .insert("sqlite_path".into(), json!("secret.db"));
    assert!(serde_json::from_value::<ProtocolRequest>(unknown).is_err());

    let invalid = ProtocolResponse::invalid_envelope(None);
    assert_eq!(invalid.request_id, "unknown");
    assert_eq!(invalid.status, ProtocolStatus::Error);
    assert_eq!(
        invalid.error.unwrap().code,
        ProtocolErrorCode::ValidationError
    );
}

#[test]
fn asserted_identity_rejects_unknown_callers_and_cross_agent_writes() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    failure(
        send(
            &mut fixture,
            request(
                Some("agent-unknown"),
                "request:unknown",
                None,
                "t5",
                ProtocolCall::GetTask(EntityQuery { id: TASK.into() }),
            ),
            5,
        ),
        ProtocolErrorCode::Unauthorized,
    );
    let error = failure(
        send(
            &mut fixture,
            request(
                Some(AGENT_B),
                "request:foreign-start",
                None,
                "t5",
                ProtocolCall::StartExecution(ExecutionRevisionCommand {
                    execution_id: E1.into(),
                    expected_revision: 0,
                }),
            ),
            5,
        ),
        ProtocolErrorCode::Forbidden,
    );
    assert_eq!(error.details.unwrap().entity_id.as_deref(), Some(E1));
    failure(
        send(
            &mut fixture,
            request(
                Some(AGENT_B),
                "request:foreign-lease",
                None,
                "t5",
                ProtocolCall::AcquireWorkspaceLease(AcquireLeaseCommand {
                    execution_id: E1.into(),
                    workspace_id: W1.into(),
                    ttl_ms: 100,
                }),
            ),
            5,
        ),
        ProtocolErrorCode::Forbidden,
    );
}

#[test]
fn revision_lease_and_state_failures_have_stable_error_codes() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    start_execution(&mut fixture, AGENT_A, E1, 0, "request:start-e1");
    let lease = acquire(&mut fixture, AGENT_A, E1, W1, 10);

    let revision_error = failure(
        send(
            &mut fixture,
            request(
                Some(AGENT_A),
                "request:stale-pause",
                None,
                "t6",
                ProtocolCall::PauseExecution(ExecutionOutcomeCommand {
                    execution_id: E1.into(),
                    expected_revision: 0,
                    outcome_code: None,
                }),
            ),
            11,
        ),
        ProtocolErrorCode::RevisionConflict,
    );
    let details = revision_error.details.unwrap();
    assert_eq!(details.expected_revision, Some(0));
    assert_eq!(details.actual_revision, Some(1));

    let mut stale_lease = lease;
    stale_lease.epoch += 1;
    failure(
        send(
            &mut fixture,
            request(
                Some(AGENT_A),
                "request:stale-renew",
                None,
                "t6",
                ProtocolCall::RenewWorkspaceLease(RenewLeaseCommand {
                    lease: stale_lease,
                    ttl_ms: 100,
                }),
            ),
            11,
        ),
        ProtocolErrorCode::LeaseConflict,
    );

    success(send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "request:complete",
            None,
            "t7",
            ProtocolCall::CompleteExecution(ExecutionOutcomeCommand {
                execution_id: E1.into(),
                expected_revision: 1,
                outcome_code: Some("done".into()),
            }),
        ),
        12,
    ));
    failure(
        send(
            &mut fixture,
            request(
                Some(AGENT_A),
                "request:invalid-pause",
                None,
                "t8",
                ProtocolCall::PauseExecution(ExecutionOutcomeCommand {
                    execution_id: E1.into(),
                    expected_revision: 2,
                    outcome_code: None,
                }),
            ),
            13,
        ),
        ProtocolErrorCode::InvalidState,
    );
}

#[test]
fn exact_retries_replay_lifecycle_lease_publication_and_version_binding() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    let first_start = start_execution(&mut fixture, AGENT_A, E1, 0, "request:start");
    let second_start = start_execution(&mut fixture, AGENT_A, E1, 0, "request:start");
    assert_eq!(first_start, second_start);

    let first_lease = acquire(&mut fixture, AGENT_A, E1, W1, 10);
    let second_lease = acquire(&mut fixture, AGENT_A, E1, W1, 11);
    assert_eq!(first_lease, second_lease);
    fs::write(fixture.w1_path.join("work.txt"), "runtime A work").unwrap();

    let first = publish(
        &mut fixture,
        AGENT_A,
        E1,
        W1,
        &first_lease,
        0,
        "request:publish-e1",
        "operation:publish-e1",
        "t7",
    );
    let replay = publish(
        &mut fixture,
        AGENT_A,
        E1,
        W1,
        &first_lease,
        0,
        "request:publish-e1",
        "operation:publish-e1",
        "t7",
    );
    assert_eq!(first, replay);

    let set_request = request(
        Some(AGENT_A),
        "request:set-version-e1",
        None,
        "t8",
        ProtocolCall::SetExecutionCurrentVersion(SetExecutionVersionCommand {
            execution_id: E1.into(),
            version_id: first.version.version_id.clone(),
            lease: first_lease.clone(),
            expected_execution_revision: 1,
            expected_workspace_revision: 2,
        }),
    );
    let first_set = success(send(&mut fixture, set_request.clone(), 21));
    let replay_set = success(send(&mut fixture, set_request, 22));
    assert_eq!(first_set, replay_set);

    let release_request = request(
        Some(AGENT_A),
        "request:release-e1",
        None,
        "t9",
        ProtocolCall::ReleaseWorkspaceLease(ReleaseLeaseCommand { lease: first_lease }),
    );
    let first_release = success(send(&mut fixture, release_request.clone(), 23));
    let replay_release = success(send(&mut fixture, release_request, 24));
    assert_eq!(first_release, replay_release);
}

#[test]
fn wire_resources_do_not_expose_workspace_locators_or_internal_errors() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    let response = send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "request:get-workspace",
            None,
            "t5",
            ProtocolCall::GetWorkspace(EntityQuery { id: W1.into() }),
        ),
        5,
    );
    let json = serde_json::to_string(&response).unwrap();
    assert!(!json.contains("locator"));
    assert!(!json.contains(fixture.w1_path.to_string_lossy().as_ref()));
    assert!(!json.contains(fixture.repository_dir.path().to_string_lossy().as_ref()));

    let missing = send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "request:missing-binding",
            None,
            "t6",
            ProtocolCall::CreateWorkspace(CreateWorkspaceCommand {
                workspace_id: "workspace-missing-binding".into(),
                project_id: PROJECT.into(),
                binding_ref: "binding:missing".into(),
                branch_ref: None,
                environment_id: Some(ENVIRONMENT.into()),
            }),
        ),
        6,
    );
    let error_json = serde_json::to_string(&missing).unwrap();
    failure(missing, ProtocolErrorCode::NotFound);
    assert!(!error_json.contains("sqlite"));
    assert!(!error_json.contains("binding:missing"));
    assert!(!error_json.contains(fixture.workspace_dir.path().to_string_lossy().as_ref()));
}

#[test]
fn publication_operation_is_durably_owned_and_visible_in_execution_inspection() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    start_execution(&mut fixture, AGENT_A, E1, 0, "request:start");
    let lease = acquire(&mut fixture, AGENT_A, E1, W1, 10);
    fs::write(fixture.w1_path.join("work.txt"), "operation provenance").unwrap();
    publish(
        &mut fixture,
        AGENT_A,
        E1,
        W1,
        &lease,
        0,
        "request:publish",
        "operation:publish",
        "t7",
    );

    let result = success(send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "request:inspect",
            None,
            "t8",
            ProtocolCall::InspectExecution(EntityQuery { id: E1.into() }),
        ),
        21,
    ));
    let ProtocolResult::ExecutionInspection(inspection) = result else {
        panic!("unexpected result: {result:?}");
    };
    assert_eq!(inspection.operations.len(), 1);
    assert_eq!(inspection.operations[0].operation_id, "operation:publish");
    assert_eq!(inspection.operations[0].action, "version.create");
    assert_eq!(inspection.operations[0].state, "completed");
}

#[test]
fn provider_neutral_handoff_resume_materialization_survives_cold_reopen() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    start_execution(&mut fixture, AGENT_A, E1, 0, "request:start-e1");
    let lease_a = acquire(&mut fixture, AGENT_A, E1, W1, 10);
    fs::write(fixture.w1_path.join("work.txt"), "runtime A checkpoint").unwrap();
    let publication_a = publish(
        &mut fixture,
        AGENT_A,
        E1,
        W1,
        &lease_a,
        0,
        "request:publish-e1",
        "operation:publish-e1",
        "t7",
    );
    let version_a = publication_a.version.version_id.clone();
    success(send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "request:set-e1-version",
            None,
            "t8",
            ProtocolCall::SetExecutionCurrentVersion(SetExecutionVersionCommand {
                execution_id: E1.into(),
                version_id: version_a.clone(),
                lease: lease_a,
                expected_execution_revision: 1,
                expected_workspace_revision: 2,
            }),
        ),
        21,
    ));
    success(send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "request:checkpoint-c1",
            None,
            "t9",
            ProtocolCall::CreateCheckpoint(CreateCheckpointCommand {
                checkpoint_id: "checkpoint:c1".into(),
                task_id: TASK.into(),
                execution_id: E1.into(),
                workspace_id: W1.into(),
                version_id: version_a.clone(),
                source_operation_id: Some("operation:publish-e1".into()),
                reason_code: "handoff_ready".into(),
            }),
        ),
        22,
    ));
    success(send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "request:interrupt-e1",
            None,
            "t10",
            ProtocolCall::InterruptExecution(ExecutionOutcomeCommand {
                execution_id: E1.into(),
                expected_revision: 2,
                outcome_code: Some("runtime_stopped".into()),
            }),
        ),
        23,
    ));

    success(send(
        &mut fixture,
        request(
            Some(AGENT_B),
            "request:resume-e2",
            None,
            "t11",
            ProtocolCall::ResumeFromCheckpoint(ResumeCheckpointCommand {
                execution_id: E2.into(),
                task_id: TASK.into(),
                parent_execution_id: Some(E1.into()),
                workspace_id: W2.into(),
                checkpoint_id: "checkpoint:c1".into(),
            }),
        ),
        24,
    ));
    success(send(
        &mut fixture,
        request(
            Some(AGENT_A),
            "request:handoff",
            None,
            "t12",
            ProtocolCall::CreateHandoff(CreateHandoffCommand {
                handoff_id: "handoff:a-to-b".into(),
                task_id: TASK.into(),
                from_execution_id: E1.into(),
                to_execution_id: E2.into(),
                source_version_id: Some(version_a.clone()),
                checkpoint_id: Some("checkpoint:c1".into()),
                reason_code: "runtime_change".into(),
            }),
        ),
        25,
    ));
    start_execution(&mut fixture, AGENT_B, E2, 0, "request:start-e2");
    let lease_b = acquire(&mut fixture, AGENT_B, E2, W2, 30);
    let materialized = match success(send(
        &mut fixture,
        request(
            Some(AGENT_B),
            "request:materialize-e2",
            None,
            "t13",
            ProtocolCall::MaterializeVersion(MaterializeVersionCommand {
                execution_id: E2.into(),
                workspace_id: W2.into(),
                source_version_id: version_a.clone(),
                lease: lease_b.clone(),
                expected_workspace_revision: 0,
            }),
        ),
        31,
    )) {
        ProtocolResult::Snapshot(snapshot) => snapshot,
        result => panic!("unexpected result: {result:?}"),
    };
    assert_eq!(materialized.workspace_id, W2);
    assert_eq!(
        fs::read_to_string(fixture.w2_path.join("work.txt")).unwrap(),
        "runtime A checkpoint"
    );
    fs::write(
        fixture.w2_path.join("work.txt"),
        "runtime A checkpoint\nruntime B continuation",
    )
    .unwrap();
    let publication_b = publish(
        &mut fixture,
        AGENT_B,
        E2,
        W2,
        &lease_b,
        1,
        "request:publish-e2",
        "operation:publish-e2",
        "t14",
    );
    assert_eq!(publication_b.version.workspace_id, W2);
    assert_eq!(publication_b.version.parent_version_id, None);
    success(send(
        &mut fixture,
        request(
            Some(AGENT_B),
            "request:checkpoint-c2",
            None,
            "t15",
            ProtocolCall::CreateCheckpoint(CreateCheckpointCommand {
                checkpoint_id: "checkpoint:c2".into(),
                task_id: TASK.into(),
                execution_id: E2.into(),
                workspace_id: W2.into(),
                version_id: publication_b.version.version_id.clone(),
                source_operation_id: Some("operation:publish-e2".into()),
                reason_code: "continuation_saved".into(),
            }),
        ),
        32,
    ));

    let root = fixture.repository_dir.path().to_path_buf();
    drop(fixture.repository);
    fixture.repository = Repository::open(&root).expect("cold reopen");
    let result = success(send(
        &mut fixture,
        request(
            Some(AGENT_B),
            "request:inspect-after-reopen",
            None,
            "t16",
            ProtocolCall::InspectExecution(EntityQuery { id: E2.into() }),
        ),
        33,
    ));
    let ProtocolResult::ExecutionInspection(inspection) = result else {
        panic!("unexpected result: {result:?}");
    };
    assert_eq!(inspection.agent.agent_id, AGENT_B);
    assert_eq!(
        inspection.agent.provider_metadata.as_deref(),
        Some("runtime-b")
    );
    assert_eq!(
        inspection.execution.parent_execution_id.as_deref(),
        Some(E1)
    );
    assert_eq!(
        inspection.execution.base_version_id.as_deref(),
        Some(version_a.as_str())
    );
    assert_eq!(
        inspection.resume.unwrap().checkpoint_id.as_deref(),
        Some("checkpoint:c1")
    );
    assert_eq!(inspection.checkpoints.len(), 1);
    assert_eq!(inspection.checkpoints[0].checkpoint_id, "checkpoint:c2");
    assert_eq!(inspection.handoffs.len(), 1);
    assert_eq!(inspection.operations.len(), 1);

    let source = fixture
        .repository
        .metadata()
        .workspace(W1)
        .unwrap()
        .unwrap();
    let target = fixture
        .repository
        .metadata()
        .workspace(W2)
        .unwrap()
        .unwrap();
    assert_ne!(source.head, target.head);
    assert_ne!(source.locator, target.locator);
    assert_eq!(source.version_head_id.as_deref(), Some(version_a.as_str()));
}

#[test]
fn diff_query_is_read_only_and_cross_workspace() {
    let mut fixture = fixture();
    bootstrap(&mut fixture);
    start_execution(&mut fixture, AGENT_A, E1, 0, "request:start");
    let lease = acquire(&mut fixture, AGENT_A, E1, W1, 10);
    fs::write(fixture.w1_path.join("work.txt"), "source").unwrap();
    let publication = publish(
        &mut fixture,
        AGENT_A,
        E1,
        W1,
        &lease,
        0,
        "request:publish",
        "operation:publish",
        "t7",
    );
    fs::write(fixture.w2_path.join("work.txt"), "target").unwrap();
    let before = fixture
        .repository
        .metadata()
        .workspace(W2)
        .unwrap()
        .unwrap();
    let query = request(
        Some(AGENT_B),
        "request:diff",
        None,
        "t8",
        ProtocolCall::DiffWorkspaceVersion(DiffWorkspaceVersionQuery {
            workspace_id: W2.into(),
            source_version_id: publication.version.version_id,
        }),
    );
    let first = success(send(&mut fixture, query.clone(), 21));
    let second = success(send(&mut fixture, query, 22));
    assert_eq!(first, second);
    let ProtocolResult::Diff(diff) = first else {
        panic!("unexpected result: {first:?}");
    };
    assert_eq!(diff.entries.len(), 1);
    assert_eq!(diff.entries[0].change, "MODIFIED");
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace(W2)
            .unwrap()
            .unwrap(),
        before
    );
}
