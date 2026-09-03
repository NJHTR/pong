use pong_core::metadata::OperationEnvelope;
use pong_core::redaction::Redactor;
use pong_core::workspace::{
    require_workspace_capability, transition_workspace_lifecycle, SnapshotOptions,
    WorkspaceCapabilities, WorkspaceCapability, WorkspaceIdentity, WorkspaceLifecycleAction,
    WorkspaceLifecycleState, WorkspaceManager, WorkspaceProviderContext,
};
use pong_core::{PongError, Repository};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

// TEST_DOUBLE: this is deliberately not a provider implementation. It models
// only the boundary outcomes needed to prove the core contract.
#[derive(Clone, Copy)]
enum ProviderResult {
    Success,
    Failure,
    Interrupted,
}

struct TestProvider {
    result: ProviderResult,
    capabilities: WorkspaceCapabilities,
}

impl TestProvider {
    fn call(&self, capability: WorkspaceCapability) -> Result<(), PongError> {
        require_workspace_capability(self.capabilities, capability)?;
        match self.result {
            ProviderResult::Success => Ok(()),
            ProviderResult::Failure => {
                Err(PongError::Io(std::io::Error::other("provider failure")))
            }
            ProviderResult::Interrupted => Err(PongError::RecoveryRequired(
                "provider transition was interrupted".into(),
            )),
        }
    }
}

// TEST_DOUBLE: models the ordering boundary around an external provider and
// a durable core write. It intentionally has no SQLite or filesystem access.
struct DurableLifecycleStore {
    state: WorkspaceLifecycleState,
    fail_persist: bool,
}

impl DurableLifecycleStore {
    fn apply(
        &mut self,
        provider: &TestProvider,
        capability: WorkspaceCapability,
        action: WorkspaceLifecycleAction,
    ) -> Result<WorkspaceLifecycleState, PongError> {
        let next = transition_workspace_lifecycle(self.state, action)?;
        provider.call(capability)?;
        if self.fail_persist {
            return Err(PongError::FaultInjected(
                "core lifecycle persistence".into(),
            ));
        }
        self.state = next;
        Ok(next)
    }
}

fn provider_context() -> WorkspaceProviderContext {
    WorkspaceProviderContext {
        identity: WorkspaceIdentity {
            workspace_id: "ws-contract".into(),
            project_id: "project-contract".into(),
            provider: "TEST_DOUBLE".into(),
        },
        state: WorkspaceLifecycleState::Ready,
        revision: 0,
        capabilities: WorkspaceCapabilities::local(),
    }
}

fn fixture() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Repository,
    pong_core::LeaseToken,
) {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-contract",
            "project-contract",
            &json!({"schema_version": 1, "provider": "local"}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_parent.path().join("source");
    let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
    manager
        .create_local(
            "ws-contract",
            "project-contract",
            &workspace_path,
            None,
            Some("env-contract"),
            "t0",
        )
        .expect("workspace");
    let lease = manager
        .acquire_lease("ws-contract", "agent-contract", 0, 60_000)
        .expect("lease");
    fs::write(workspace_path.join("state.txt"), b"contract").expect("state");
    (project, workspace_parent, repository, lease)
}

fn operation(request_id: &str, operation_id: &str) -> OperationEnvelope {
    OperationEnvelope {
        operation_id: operation_id.into(),
        project_id: "project-contract".into(),
        request_id: request_id.into(),
        agent_id: "agent-contract".into(),
        session_id: "session-contract".into(),
        workspace_id: Some("ws-contract".into()),
        environment_id: Some("env-contract".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "t1".into(),
        tool: "workspace".into(),
        action: "workspace.lifecycle".into(),
        input_refs: Vec::new(),
        output_refs: Vec::new(),
        resource: None,
        before_state: None,
        after_state: None,
        reversibility: "REVERSIBLE".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "WORKSPACE".into(),
        policy_decision: None,
    }
}

#[test]
fn l1_create_to_open_is_idempotent() {
    assert_eq!(
        transition_workspace_lifecycle(
            WorkspaceLifecycleState::Created,
            WorkspaceLifecycleAction::Open
        )
        .expect("open"),
        WorkspaceLifecycleState::Created
    );
}

#[test]
fn l2_open_to_close_archives_logically() {
    assert_eq!(
        transition_workspace_lifecycle(
            WorkspaceLifecycleState::Ready,
            WorkspaceLifecycleAction::Close
        )
        .expect("close"),
        WorkspaceLifecycleState::Archived
    );
}

#[test]
fn l3_invalid_transition_is_rejected() {
    let error = transition_workspace_lifecycle(
        WorkspaceLifecycleState::Created,
        WorkspaceLifecycleAction::Resume,
    )
    .expect_err("invalid transition");
    assert_eq!(error.code(), "CONFLICT");
}

#[test]
fn l4_same_transition_retry_is_deterministic() {
    let first = transition_workspace_lifecycle(
        WorkspaceLifecycleState::Preparing,
        WorkspaceLifecycleAction::BeginCapture,
    )
    .expect("first");
    let second = transition_workspace_lifecycle(
        WorkspaceLifecycleState::Preparing,
        WorkspaceLifecycleAction::BeginCapture,
    )
    .expect("retry");
    assert_eq!(first, second);
}

#[test]
fn l5_provider_success_remains_within_capability_boundary() {
    let provider = TestProvider {
        result: ProviderResult::Success,
        capabilities: WorkspaceCapabilities::local(),
    };
    provider
        .call(WorkspaceCapability::Snapshot)
        .expect("provider success");
}

#[test]
fn l6_provider_failure_is_not_core_success() {
    let provider = TestProvider {
        result: ProviderResult::Failure,
        capabilities: WorkspaceCapabilities::local(),
    };
    let error = provider
        .call(WorkspaceCapability::Restore)
        .expect_err("failure");
    assert_eq!(error.code(), "IO_ERROR");
}

#[test]
fn l7_core_persistence_failure_cannot_be_reported_as_provider_success() {
    let provider = TestProvider {
        result: ProviderResult::Success,
        capabilities: WorkspaceCapabilities::local(),
    };
    let mut store = DurableLifecycleStore {
        state: WorkspaceLifecycleState::Ready,
        fail_persist: true,
    };
    let error = store
        .apply(
            &provider,
            WorkspaceCapability::Status,
            WorkspaceLifecycleAction::Close,
        )
        .expect_err("persistence failure");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert_eq!(store.state, WorkspaceLifecycleState::Ready);
}

#[test]
fn l8_interrupted_transition_reopens_as_reconciling() {
    let provider = TestProvider {
        result: ProviderResult::Interrupted,
        capabilities: WorkspaceCapabilities::local(),
    };
    let error = provider
        .call(WorkspaceCapability::Status)
        .expect_err("interrupted provider");
    assert_eq!(error.code(), "RECOVERY_REQUIRED");
    assert_eq!(
        transition_workspace_lifecycle(
            WorkspaceLifecycleState::Ready,
            WorkspaceLifecycleAction::BeginRecovery,
        )
        .expect("begin recovery"),
        WorkspaceLifecycleState::Reconciling
    );
    assert_eq!(
        transition_workspace_lifecycle(
            WorkspaceLifecycleState::Reconciling,
            WorkspaceLifecycleAction::CompleteRecovery,
        )
        .expect("complete recovery"),
        WorkspaceLifecycleState::Ready
    );
}

#[test]
fn l9_unsupported_capability_is_explicit() {
    let capabilities = WorkspaceCapabilities {
        status: true,
        ..WorkspaceCapabilities::default()
    };
    let error = require_workspace_capability(capabilities, WorkspaceCapability::Snapshot)
        .expect_err("unsupported");
    assert_eq!(error.code(), "UNSUPPORTED");
}

#[test]
fn l10_status_is_read_only() {
    let (_project, _parent, mut repository, _lease) = fixture();
    let before = repository
        .metadata()
        .workspace("ws-contract")
        .expect("read")
        .expect("row");
    let status = WorkspaceManager::new(&mut repository, Redactor::default())
        .status("ws-contract", 1)
        .expect("status");
    let after = repository
        .metadata()
        .workspace("ws-contract")
        .expect("read")
        .expect("row");
    assert_eq!(before, after);
    assert_eq!(status.revision, 0);
}

#[test]
fn l11_diff_is_read_only() {
    let (_project, _parent, mut repository, lease) = fixture();
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local("ws-contract", &lease, SnapshotOptions::default(), 1, "t1")
        .expect("snapshot");
    let before = repository
        .metadata()
        .workspace("ws-contract")
        .expect("read")
        .expect("row");
    let diff = WorkspaceManager::new(&mut repository, Redactor::default())
        .diff_workspace("ws-contract")
        .expect("diff");
    let after = repository
        .metadata()
        .workspace("ws-contract")
        .expect("read")
        .expect("row");
    assert_eq!(before, after);
    assert_eq!(diff.reference_snapshot_id, snapshot.snapshot_id);
    assert!(diff.diff.entries.is_empty());
}

#[test]
fn l12_legacy_v01_repository_remains_readable() {
    let project = tempdir().expect("project");
    let initialized = Repository::init(project.path()).expect("repository layout");
    let metadata_path = initialized.layout().metadata_path().to_path_buf();
    drop(initialized);
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut candidate = metadata_path.as_os_str().to_os_string();
        candidate.push(suffix);
        let candidate = std::path::PathBuf::from(candidate);
        match fs::remove_file(candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("remove initialized metadata: {error}"),
        }
    }
    let connection = Connection::open(&metadata_path).expect("legacy metadata");
    connection
        .execute_batch(include_str!("fixtures/CT-09-v01-metadata.sql"))
        .expect("legacy fixture");
    drop(connection);
    let mut repository = Repository::open(project.path()).expect("legacy open");
    assert_eq!(
        repository.metadata().repository_format().expect("format"),
        "0.1"
    );
    assert_eq!(
        repository
            .metadata()
            .list_events("legacy-stream")
            .expect("legacy events")
            .len(),
        1
    );
    let status_error = WorkspaceManager::new(&mut repository, Redactor::default())
        .status("missing-legacy-workspace", 0)
        .expect_err("no legacy workspace row");
    assert_eq!(status_error.code(), "NOT_FOUND");
}

#[test]
fn l13_provider_specific_metadata_does_not_leak_into_context() {
    let context = provider_context();
    let encoded = serde_json::to_string(&context).expect("context json");
    assert!(!encoded.contains("/tmp"));
    assert!(!encoded.contains("provider_secret"));
    assert!(encoded.contains("TEST_DOUBLE"));
}

#[test]
fn l14_same_operation_retry_returns_deterministic_record() {
    let (_project, _parent, mut repository, _lease) = fixture();
    let envelope = operation("request-retry", "operation-retry");
    let first = repository
        .metadata_mut()
        .start_operation(envelope.clone())
        .expect("start");
    let second = repository
        .metadata_mut()
        .start_operation(envelope)
        .expect("retry");
    assert_eq!(first, second);
}

#[test]
fn l15_provider_boundary_cannot_change_generation_or_event_order() {
    let (_project, _parent, repository, _lease) = fixture();
    let before_generation = (
        repository.active_generation_id().map(str::to_owned),
        repository.generation_manifest().cloned(),
    );
    let before_events = repository
        .metadata()
        .list_event_envelopes("project-contract", 0)
        .expect("events");
    let provider = TestProvider {
        result: ProviderResult::Success,
        capabilities: WorkspaceCapabilities::local(),
    };
    provider
        .call(WorkspaceCapability::Status)
        .expect("provider");
    let after_generation = (
        repository.active_generation_id().map(str::to_owned),
        repository.generation_manifest().cloned(),
    );
    let after_events = repository
        .metadata()
        .list_event_envelopes("project-contract", 0)
        .expect("events");
    assert_eq!(before_generation, after_generation);
    assert_eq!(before_events, after_events);
}
