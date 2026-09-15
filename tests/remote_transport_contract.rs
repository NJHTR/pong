//! M4-012 transport-neutral Remote Agent access contract.

use pong_core::protocol::{
    AcquireLeaseCommand, CreateExecutionCommand, CreateTaskCommand, CreateWorkspaceCommand,
    EntityQuery, ExecutionOutcomeCommand, ExecutionRevisionCommand, ExternalAgentProtocol,
    OperationRequestQuery, ProtocolCall, ProtocolErrorCode, ProtocolRequest, ProtocolResponse,
    ProtocolResult, ProtocolStatus, RegisterAgentCommand, StartOperationCommand,
    WorkspaceBindingError, WorkspaceBindingResolver, EXTERNAL_AGENT_PROTOCOL_VERSION,
};
use pong_core::{
    AuthenticatedPrincipal, RemoteAccessBoundary, RemoteAccessError, RemoteAccessErrorCode,
    Repository, REMOTE_ACCESS_CONTRACT_VERSION,
};
use serde_json::json;
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};

const PRINCIPAL_A: &str = "principal-a";
const PRINCIPAL_B: &str = "principal-b";
const AGENT_A: &str = "agent-a";
const AGENT_B: &str = "agent-b";
const PROJECT: &str = "remote-project";
const TASK: &str = "remote-task";
const WORKSPACE: &str = "remote-workspace";
const EXECUTION: &str = "remote-execution";

struct Bindings {
    root: PathBuf,
}

impl WorkspaceBindingResolver for Bindings {
    fn resolve(&self, binding_ref: &str) -> Result<PathBuf, WorkspaceBindingError> {
        Ok(self.root.join(binding_ref))
    }
}

struct Fixture {
    repository_dir: TempDir,
    _workspace_dir: TempDir,
    repository: Repository,
    bindings: Bindings,
    remote: RemoteAccessBoundary,
}

fn fixture() -> Fixture {
    let repository_dir = tempdir().unwrap();
    let workspace_dir = tempdir().unwrap();
    let mut repository = Repository::init(repository_dir.path()).unwrap();
    repository
        .metadata_mut()
        .record_environment("remote-env", PROJECT, &json!({"schema_version": 1}), "t0")
        .unwrap();
    Fixture {
        repository_dir,
        bindings: Bindings {
            root: workspace_dir.path().to_path_buf(),
        },
        _workspace_dir: workspace_dir,
        repository,
        remote: RemoteAccessBoundary::new(),
    }
}

fn principal(principal_id: &str, agents: &[&str]) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal {
        principal_id: principal_id.into(),
        agent_ids: agents.iter().map(|value| (*value).into()).collect(),
    }
}

fn request(
    caller: Option<&str>,
    request_id: &str,
    operation_id: Option<&str>,
    call: ProtocolCall,
) -> ProtocolRequest {
    ProtocolRequest {
        protocol_version: EXTERNAL_AGENT_PROTOCOL_VERSION.into(),
        request_id: request_id.into(),
        caller_agent_id: caller.map(Into::into),
        issued_at: format!("time:{request_id}"),
        operation_id: operation_id.map(Into::into),
        call,
    }
}

fn bind(fixture: &mut Fixture, session: &str, principal_id: &str, agents: &[&str], now: i64) {
    fixture
        .remote
        .bind_authenticated(principal(principal_id, agents), session, now, 1_000)
        .unwrap();
}

fn send(
    fixture: &mut Fixture,
    session: &str,
    request: ProtocolRequest,
    now: i64,
) -> Result<ProtocolResponse, RemoteAccessError> {
    let authorized = fixture.remote.authorize(session, request, now)?;
    Ok(
        ExternalAgentProtocol::new(&mut fixture.repository, &fixture.bindings)
            .handle(authorized.into_request(), now),
    )
}

fn ok(response: ProtocolResponse) -> ProtocolResult {
    assert_eq!(response.status, ProtocolStatus::Ok, "{response:#?}");
    response.result.unwrap()
}

fn protocol_error(response: ProtocolResponse, code: ProtocolErrorCode) {
    assert_eq!(response.status, ProtocolStatus::Error, "{response:#?}");
    assert_eq!(response.error.unwrap().code, code);
}

fn bootstrap(fixture: &mut Fixture, session: &str) {
    ok(send(
        fixture,
        session,
        request(
            Some(AGENT_A),
            "register-a",
            None,
            ProtocolCall::RegisterAgent(RegisterAgentCommand {
                agent_id: AGENT_A.into(),
                provider_metadata: Some("runtime-neutral".into()),
                display_name: None,
            }),
        ),
        10,
    )
    .unwrap());
    ok(send(
        fixture,
        session,
        request(
            Some(AGENT_A),
            "create-task",
            None,
            ProtocolCall::CreateTask(CreateTaskCommand {
                task_id: TASK.into(),
                project_id: PROJECT.into(),
                goal_ref: "goal:remote-contract".into(),
                context_ref: None,
            }),
        ),
        11,
    )
    .unwrap());
    ok(send(
        fixture,
        session,
        request(
            Some(AGENT_A),
            "create-workspace",
            None,
            ProtocolCall::CreateWorkspace(CreateWorkspaceCommand {
                workspace_id: WORKSPACE.into(),
                project_id: PROJECT.into(),
                binding_ref: "binding-a".into(),
                branch_ref: None,
                environment_id: Some("remote-env".into()),
            }),
        ),
        12,
    )
    .unwrap());
    ok(send(
        fixture,
        session,
        request(
            Some(AGENT_A),
            "create-execution",
            None,
            ProtocolCall::CreateExecution(CreateExecutionCommand {
                execution_id: EXECUTION.into(),
                task_id: TASK.into(),
                parent_execution_id: None,
                workspace_id: Some(WORKSPACE.into()),
                base_version_id: None,
            }),
        ),
        13,
    )
    .unwrap());
    ok(send(
        fixture,
        session,
        request(
            Some(AGENT_A),
            "start-execution",
            None,
            ProtocolCall::StartExecution(ExecutionRevisionCommand {
                execution_id: EXECUTION.into(),
                expected_revision: 0,
            }),
        ),
        14,
    )
    .unwrap());
}

fn start_operation(request_id: &str, operation_id: &str) -> ProtocolRequest {
    request(
        Some(AGENT_A),
        request_id,
        Some(operation_id),
        ProtocolCall::StartOperation(StartOperationCommand {
            execution_id: EXECUTION.into(),
            action: "remote.work".into(),
        }),
    )
}

#[test]
fn connect_authenticate_bind_and_disconnect_are_ephemeral() {
    let mut fixture = fixture();
    let session = fixture
        .remote
        .bind_authenticated(principal(PRINCIPAL_A, &[AGENT_A]), "session-a", 1, 100)
        .unwrap();
    assert_eq!(session.principal_id, PRINCIPAL_A);
    assert_ne!(session.session_id, PRINCIPAL_A);
    assert_ne!(session.session_id, AGENT_A);
    assert!(fixture.remote.disconnect("session-a"));
    let error = fixture
        .remote
        .authorize(
            "session-a",
            request(None, "hello", None, ProtocolCall::Hello),
            2,
        )
        .unwrap_err();
    assert_eq!(error.code, RemoteAccessErrorCode::AuthenticationRequired);
}

#[test]
fn transport_authentication_failure_never_creates_a_session() {
    let fixture = fixture();
    let rejected = RemoteAccessError::authentication_failed();
    assert_eq!(rejected.code, RemoteAccessErrorCode::AuthenticationFailed);
    assert!(fixture.remote.session("session-rejected").is_none());
}

#[test]
fn invalid_principal_and_session_bindings_are_rejected() {
    let mut fixture = fixture();
    let invalid = fixture
        .remote
        .bind_authenticated(principal("", &[AGENT_A]), "session-a", 1, 100)
        .unwrap_err();
    assert_eq!(invalid.code, RemoteAccessErrorCode::InvalidPrincipal);
    fixture
        .remote
        .bind_authenticated(principal(PRINCIPAL_A, &[AGENT_A]), "session-a", 1, 100)
        .unwrap();
    let duplicate = fixture
        .remote
        .bind_authenticated(principal(PRINCIPAL_B, &[AGENT_B]), "session-a", 2, 100)
        .unwrap_err();
    assert_eq!(duplicate.code, RemoteAccessErrorCode::Forbidden);
}

#[test]
fn non_hello_request_requires_an_agent_binding() {
    let mut fixture = fixture();
    bind(&mut fixture, "session-a", PRINCIPAL_A, &[AGENT_A], 1);
    let error = send(
        &mut fixture,
        "session-a",
        request(
            None,
            "unbound-agent",
            None,
            ProtocolCall::GetAgent(EntityQuery { id: AGENT_A.into() }),
        ),
        2,
    )
    .unwrap_err();
    assert_eq!(error.code, RemoteAccessErrorCode::InvalidAgentBinding);
}

#[test]
fn authorized_context_correlates_principal_session_request_operation_and_agent() {
    let mut fixture = fixture();
    bind(&mut fixture, "session-a", PRINCIPAL_A, &[AGENT_A], 1);
    let authorized = fixture
        .remote
        .authorize(
            "session-a",
            start_operation("request-observed", "operation-observed"),
            2,
        )
        .unwrap();
    assert_eq!(authorized.session().principal_id, PRINCIPAL_A);
    assert_eq!(authorized.session().session_id, "session-a");
    assert_eq!(authorized.request().request_id, "request-observed");
    assert_eq!(
        authorized.request().operation_id.as_deref(),
        Some("operation-observed")
    );
    assert_eq!(
        authorized.request().caller_agent_id.as_deref(),
        Some(AGENT_A)
    );
}

#[test]
fn principal_binding_is_distinct_from_agent_and_enforced_before_protocol() {
    let mut fixture = fixture();
    bind(&mut fixture, "session-a", PRINCIPAL_A, &[AGENT_A], 1);
    let error = send(
        &mut fixture,
        "session-a",
        request(
            Some(AGENT_B),
            "foreign-register",
            None,
            ProtocolCall::RegisterAgent(RegisterAgentCommand {
                agent_id: AGENT_B.into(),
                provider_metadata: None,
                display_name: None,
            }),
        ),
        2,
    )
    .unwrap_err();
    assert_eq!(error.code, RemoteAccessErrorCode::Forbidden);
    assert!(fixture
        .repository
        .metadata()
        .agent_identity(AGENT_B)
        .unwrap()
        .is_none());
}

#[test]
fn capability_discovery_separates_remote_access_from_domain_protocol() {
    let mut fixture = fixture();
    let capabilities = fixture.remote.capabilities();
    assert_eq!(
        capabilities.contract_version,
        REMOTE_ACCESS_CONTRACT_VERSION
    );
    assert!(capabilities.authentication_required);
    assert!(capabilities.reauthentication_required_on_reconnect);
    assert!(capabilities.core_restart_invalidates_sessions);
    assert!(!capabilities.disconnect_cancels_operations);

    bind(&mut fixture, "session-a", PRINCIPAL_A, &[AGENT_A], 1);
    let ProtocolResult::Hello(hello) = ok(send(
        &mut fixture,
        "session-a",
        request(None, "hello", None, ProtocolCall::Hello),
        2,
    )
    .unwrap()) else {
        panic!("expected protocol hello")
    };
    assert_eq!(hello.protocol_versions, ["1.0"]);
    assert!(hello.transport_neutral);
}

#[test]
fn duplicate_request_replays_one_operation_while_new_request_is_distinct() {
    let mut fixture = fixture();
    bind(&mut fixture, "session-a", PRINCIPAL_A, &[AGENT_A], 1);
    bootstrap(&mut fixture, "session-a");
    let first = ok(send(
        &mut fixture,
        "session-a",
        start_operation("request-one", "operation-one"),
        20,
    )
    .unwrap());
    let retry = ok(send(
        &mut fixture,
        "session-a",
        start_operation("request-one", "operation-one"),
        21,
    )
    .unwrap());
    assert_eq!(first, retry);
    ok(send(
        &mut fixture,
        "session-a",
        start_operation("request-two", "operation-two"),
        22,
    )
    .unwrap());
    for operation_id in ["operation-one", "operation-two"] {
        assert!(fixture
            .repository
            .metadata()
            .operation_record(operation_id)
            .unwrap()
            .is_some());
        assert!(fixture
            .repository
            .metadata()
            .execution_operation(EXECUTION, operation_id)
            .unwrap()
            .is_some());
    }
}

#[test]
fn uncertain_outcome_is_resolved_after_reconnect_without_old_session() {
    let mut fixture = fixture();
    bind(&mut fixture, "session-a", PRINCIPAL_A, &[AGENT_A], 1);
    bootstrap(&mut fixture, "session-a");
    ok(send(
        &mut fixture,
        "session-a",
        start_operation("request-uncertain", "operation-uncertain"),
        20,
    )
    .unwrap());
    fixture.remote.disconnect("session-a");

    bind(&mut fixture, "session-b", PRINCIPAL_A, &[AGENT_A], 30);
    let resolved = ok(send(
        &mut fixture,
        "session-b",
        request(
            Some(AGENT_A),
            "resolve-uncertain",
            None,
            ProtocolCall::ResolveOperation(OperationRequestQuery {
                project_id: PROJECT.into(),
                request_id: "request-uncertain".into(),
            }),
        ),
        31,
    )
    .unwrap());
    let ProtocolResult::Operation(operation) = resolved else {
        panic!("expected operation")
    };
    assert_eq!(operation.operation_id, "operation-uncertain");
    assert_eq!(operation.state, "started");
}

#[test]
fn session_expiry_requires_reauthentication_and_does_not_mutate_state() {
    let mut fixture = fixture();
    fixture
        .remote
        .bind_authenticated(principal(PRINCIPAL_A, &[AGENT_A]), "short", 1, 2)
        .unwrap();
    let error = send(
        &mut fixture,
        "short",
        request(
            Some(AGENT_A),
            "expired-register",
            None,
            ProtocolCall::RegisterAgent(RegisterAgentCommand {
                agent_id: AGENT_A.into(),
                provider_metadata: None,
                display_name: None,
            }),
        ),
        3,
    )
    .unwrap_err();
    assert_eq!(error.code, RemoteAccessErrorCode::SessionExpired);
    assert!(fixture
        .repository
        .metadata()
        .agent_identity(AGENT_A)
        .unwrap()
        .is_none());
}

#[test]
fn resource_authorization_revision_and_lease_errors_remain_protocol_semantics() {
    let mut fixture = fixture();
    bind(&mut fixture, "session-a", PRINCIPAL_A, &[AGENT_A], 1);
    bootstrap(&mut fixture, "session-a");
    bind(&mut fixture, "session-b", PRINCIPAL_B, &[AGENT_B], 2);
    ok(send(
        &mut fixture,
        "session-b",
        request(
            Some(AGENT_B),
            "register-b",
            None,
            ProtocolCall::RegisterAgent(RegisterAgentCommand {
                agent_id: AGENT_B.into(),
                provider_metadata: None,
                display_name: None,
            }),
        ),
        15,
    )
    .unwrap());
    protocol_error(
        send(
            &mut fixture,
            "session-b",
            request(
                Some(AGENT_B),
                "inspect-foreign",
                None,
                ProtocolCall::GetExecution(EntityQuery {
                    id: EXECUTION.into(),
                }),
            ),
            16,
        )
        .unwrap(),
        ProtocolErrorCode::Forbidden,
    );
    protocol_error(
        send(
            &mut fixture,
            "session-a",
            request(
                Some(AGENT_A),
                "stale-start",
                None,
                ProtocolCall::PauseExecution(ExecutionOutcomeCommand {
                    execution_id: EXECUTION.into(),
                    expected_revision: 0,
                    outcome_code: None,
                }),
            ),
            17,
        )
        .unwrap(),
        ProtocolErrorCode::RevisionConflict,
    );
    let ProtocolResult::Lease(mut lease) = ok(send(
        &mut fixture,
        "session-a",
        request(
            Some(AGENT_A),
            "acquire-lease",
            None,
            ProtocolCall::AcquireWorkspaceLease(AcquireLeaseCommand {
                execution_id: EXECUTION.into(),
                workspace_id: WORKSPACE.into(),
                ttl_ms: 100,
            }),
        ),
        18,
    )
    .unwrap()) else {
        panic!("expected lease")
    };
    lease.authority.epoch += 1;
    protocol_error(
        send(
            &mut fixture,
            "session-a",
            request(
                Some(AGENT_A),
                "stale-lease",
                None,
                ProtocolCall::ReleaseWorkspaceLease(pong_core::protocol::ReleaseLeaseCommand {
                    lease: lease.authority,
                }),
            ),
            19,
        )
        .unwrap(),
        ProtocolErrorCode::LeaseConflict,
    );
}

#[test]
fn core_restart_invalidates_session_but_preserves_durable_operation() {
    let mut fixture = fixture();
    bind(&mut fixture, "old-session", PRINCIPAL_A, &[AGENT_A], 1);
    bootstrap(&mut fixture, "old-session");
    ok(send(
        &mut fixture,
        "old-session",
        start_operation("request-restart", "operation-restart"),
        20,
    )
    .unwrap());

    let root = fixture.repository_dir.path().to_path_buf();
    drop(fixture.repository);
    fixture.repository = Repository::open(root).unwrap();
    fixture.remote = RemoteAccessBoundary::new();
    let old_error = fixture
        .remote
        .authorize(
            "old-session",
            request(None, "old-hello", None, ProtocolCall::Hello),
            30,
        )
        .unwrap_err();
    assert_eq!(
        old_error.code,
        RemoteAccessErrorCode::AuthenticationRequired
    );

    bind(&mut fixture, "new-session", PRINCIPAL_A, &[AGENT_A], 31);
    let ProtocolResult::Operation(operation) = ok(send(
        &mut fixture,
        "new-session",
        request(
            Some(AGENT_A),
            "get-after-core-restart",
            None,
            ProtocolCall::GetOperation(EntityQuery {
                id: "operation-restart".into(),
            }),
        ),
        32,
    )
    .unwrap()) else {
        panic!("expected operation")
    };
    assert_eq!(operation.state, "started");
}

#[test]
fn client_restart_uses_new_session_and_same_durable_agent_execution() {
    let mut fixture = fixture();
    bind(
        &mut fixture,
        "client-process-one",
        PRINCIPAL_A,
        &[AGENT_A],
        1,
    );
    bootstrap(&mut fixture, "client-process-one");
    fixture.remote.disconnect("client-process-one");
    bind(
        &mut fixture,
        "client-process-two",
        PRINCIPAL_A,
        &[AGENT_A],
        20,
    );
    let ProtocolResult::Execution(execution) = ok(send(
        &mut fixture,
        "client-process-two",
        request(
            Some(AGENT_A),
            "get-after-client-restart",
            None,
            ProtocolCall::GetExecution(EntityQuery {
                id: EXECUTION.into(),
            }),
        ),
        21,
    )
    .unwrap()) else {
        panic!("expected execution")
    };
    assert_eq!(execution.agent_id, AGENT_A);
    assert_eq!(execution.state, "running");
}

#[test]
fn protocol_version_mismatch_remains_a_protocol_error() {
    let mut fixture = fixture();
    bind(&mut fixture, "session-a", PRINCIPAL_A, &[AGENT_A], 1);
    let mut incompatible = request(None, "old-version", None, ProtocolCall::Hello);
    incompatible.protocol_version = "0.9".into();
    protocol_error(
        send(&mut fixture, "session-a", incompatible, 2).unwrap(),
        ProtocolErrorCode::UnsupportedVersion,
    );
}

#[test]
fn transport_timeout_and_disconnect_do_not_cancel_durable_operation() {
    let mut fixture = fixture();
    bind(
        &mut fixture,
        "timed-out-session",
        PRINCIPAL_A,
        &[AGENT_A],
        1,
    );
    bootstrap(&mut fixture, "timed-out-session");
    ok(send(
        &mut fixture,
        "timed-out-session",
        start_operation("request-timeout", "operation-timeout"),
        20,
    )
    .unwrap());
    fixture.remote.disconnect("timed-out-session");
    let operation = fixture
        .repository
        .metadata()
        .operation_record("operation-timeout")
        .unwrap()
        .unwrap();
    assert_eq!(operation.lifecycle_status, "started");
    assert_ne!(operation.lifecycle_status, "cancelled");
}

#[test]
fn remote_error_contract_is_wire_safe_and_core_unavailable_is_retryable() {
    let error = RemoteAccessError::core_unavailable();
    assert_eq!(error.code, RemoteAccessErrorCode::CoreUnavailable);
    assert!(error.retryable);
    let wire = serde_json::to_value(error).unwrap();
    assert_eq!(wire["code"], "CORE_UNAVAILABLE");
    assert!(wire.get("stack_trace").is_none());
    assert!(wire.get("token").is_none());
}

#[test]
fn connection_session_principal_agent_execution_ids_never_collapse() {
    let mut fixture = fixture();
    bind(
        &mut fixture,
        "connection-derived-session",
        PRINCIPAL_A,
        &[AGENT_A],
        1,
    );
    bootstrap(&mut fixture, "connection-derived-session");
    let session = fixture
        .remote
        .session("connection-derived-session")
        .unwrap();
    assert_ne!(session.session_id, session.principal_id);
    assert_ne!(session.principal_id, AGENT_A);
    assert_ne!(AGENT_A, EXECUTION);
    assert_ne!(session.session_id, EXECUTION);
}

#[test]
fn remote_contract_requires_no_network_or_provider_specific_state() {
    let source = include_str!("../src/remote.rs");
    for forbidden in [
        "TcpListener",
        "HttpServer",
        "WebSocket",
        "Jwt",
        "OAuth",
        "CodexAgent",
        "ClaudeAgent",
        "McpServer",
    ] {
        assert!(
            !source.contains(forbidden),
            "unexpected coupling: {forbidden}"
        );
    }
}
