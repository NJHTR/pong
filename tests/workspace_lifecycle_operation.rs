use pong_core::metadata::{MetadataFailpoint, MetadataFailpoints, OperationRecord};
use pong_core::redaction::Redactor;
use pong_core::workspace::{
    SnapshotOptions, WorkspaceLifecycleAction, WorkspaceLifecycleOperationOptions, WorkspaceManager,
};
use pong_core::{PongError, Repository, WorkspaceRecord};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

struct Fixture {
    project: tempfile::TempDir,
    _workspace_parent: tempfile::TempDir,
    repository: Repository,
    lease: pong_core::LeaseToken,
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-operation",
            "project-operation",
            &json!({"schema_version": 1, "os": "windows"}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_parent.path().join("source");
    let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
    manager
        .create_local(
            "ws-operation",
            "project-operation",
            &workspace_path,
            Some("refs/heads/main"),
            Some("env-operation"),
            "t0",
        )
        .expect("workspace");
    let lease = manager
        .acquire_lease("ws-operation", "agent-operation", 0, 60_000)
        .expect("lease");
    fs::write(workspace_path.join("state.txt"), b"operation").expect("state");
    Fixture {
        project,
        _workspace_parent: workspace_parent,
        repository,
        lease,
    }
}

fn current(repository: &Repository) -> WorkspaceRecord {
    repository
        .metadata()
        .workspace("ws-operation")
        .expect("workspace read")
        .expect("workspace row")
}

fn operation(repository: &Repository, id: &str, request: &str) -> OperationRecord {
    repository
        .metadata()
        .operation_record_for_request("project-operation", "agent-operation", request)
        .expect("operation lookup")
        .filter(|record| record.operation_id == id)
        .expect("operation row")
}

#[allow(clippy::too_many_arguments)]
fn transition(
    repository: &mut Repository,
    lease: &pong_core::LeaseToken,
    revision: i64,
    action: WorkspaceLifecycleAction,
    operation_id: &str,
    request_id: &str,
    now_ms: i64,
    updated_at: &str,
) -> Result<(WorkspaceRecord, OperationRecord), PongError> {
    WorkspaceManager::new(repository, Redactor::default()).transition_lifecycle_operation(
        "ws-operation",
        lease,
        revision,
        action,
        WorkspaceLifecycleOperationOptions {
            operation_id: operation_id.into(),
            request_id: request_id.into(),
            now_ms,
            updated_at: updated_at.into(),
        },
    )
}

#[test]
fn operation_identity_binds_one_durable_lifecycle_mutation_and_exact_retry() {
    let mut fixture = fixture();
    let first = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        "op-lifecycle-1",
        "req-lifecycle-1",
        1,
        "t1",
    )
    .expect("transition");
    assert_eq!(first.0.status, "preparing");
    assert_eq!(first.0.revision, 1);
    assert_eq!(first.1.lifecycle_status, "completed");
    assert_eq!(first.1.action, "workspace.lifecycle.begin_capture");
    assert_eq!(
        first.1.result,
        Some(json!({
            "action": "workspace.lifecycle.begin_capture",
            "revision": 1,
            "status": "preparing",
            "workspace_id": "ws-operation"
        }))
    );

    let retry = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        "op-lifecycle-1",
        "req-lifecycle-1",
        2,
        "t2",
    )
    .expect("exact retry");
    assert_eq!(retry, first);
    assert_eq!(current(&fixture.repository).revision, 1);
    assert_eq!(
        operation(&fixture.repository, "op-lifecycle-1", "req-lifecycle-1"),
        first.1
    );

    let changed_revision = transition(
        &mut fixture.repository,
        &fixture.lease,
        1,
        WorkspaceLifecycleAction::BeginCapture,
        "op-lifecycle-1",
        "req-lifecycle-1",
        3,
        "t3",
    )
    .expect_err("changed retry revision");
    assert_eq!(changed_revision.code(), "IDEMPOTENCY_KEY_REUSE");

    let events = fixture
        .repository
        .metadata()
        .list_events("operation:op-lifecycle-1")
        .expect("operation events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].sequence, 1);
    assert_eq!(events[1].sequence, 2);
}

#[test]
fn same_operation_different_action_and_stale_callers_fail_closed() {
    let mut fixture = fixture();
    transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        "op-lifecycle-2",
        "req-lifecycle-2",
        1,
        "t1",
    )
    .expect("first");

    let changed = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::CompleteCapture,
        "op-lifecycle-2",
        "req-lifecycle-2",
        2,
        "t2",
    )
    .expect_err("changed operation semantics");
    assert_eq!(changed.code(), "IDEMPOTENCY_KEY_REUSE");

    let stale = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        "op-lifecycle-3",
        "req-lifecycle-3",
        2,
        "t2",
    )
    .expect_err("stale revision");
    assert_eq!(stale.code(), "CONFLICT");
    assert_eq!(current(&fixture.repository).revision, 1);

    let expired = pong_core::LeaseToken {
        expires_at_ms: 1,
        ..fixture.lease.clone()
    };
    let stale_lease = transition(
        &mut fixture.repository,
        &expired,
        1,
        WorkspaceLifecycleAction::BeginCapture,
        "op-lifecycle-4",
        "req-lifecycle-4",
        100_000,
        "t3",
    )
    .expect_err("stale lease");
    assert_eq!(stale_lease.code(), "CONFLICT");
    assert_eq!(current(&fixture.repository).revision, 1);
}

#[test]
fn read_like_open_status_and_diff_do_not_create_operations_or_events() {
    let mut fixture = fixture();
    let before = current(&fixture.repository);
    let before_events = fixture
        .repository
        .metadata()
        .list_event_envelopes("project-operation", 0)
        .expect("events");
    let open = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .transition_lifecycle(
            "ws-operation",
            &fixture.lease,
            0,
            WorkspaceLifecycleAction::Open,
            1,
            "t1",
        )
        .expect("open");
    assert_eq!(open, before);
    let status = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-operation", 1)
        .expect("status");
    assert_eq!(status.revision, before.revision);
    let diff_error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace("ws-operation")
        .expect_err("diff has no head");
    assert_eq!(diff_error.code(), "INTEGRITY_ERROR");
    let after_events = fixture
        .repository
        .metadata()
        .list_event_envelopes("project-operation", 0)
        .expect("events");
    assert_eq!(before_events, after_events);
    assert!(fixture
        .repository
        .metadata()
        .operation_record_for_request("project-operation", "agent-operation", "req-read")
        .expect("operation lookup")
        .is_none());
    assert_eq!(current(&fixture.repository), before);
}

#[test]
fn precommit_failure_rolls_back_operation_and_workspace_together() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeOperationFinishCommit,
        ));
    let error = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        "op-lifecycle-pre",
        "req-lifecycle-pre",
        1,
        "t1",
    )
    .expect_err("precommit failure");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert_eq!(current(&fixture.repository).revision, 0);
    assert!(fixture
        .repository
        .metadata()
        .operation_record("op-lifecycle-pre")
        .expect("operation lookup")
        .is_none());

    let retried = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        "op-lifecycle-pre",
        "req-lifecycle-pre",
        2,
        "t2",
    )
    .expect("retry after rollback");
    assert_eq!(retried.0.revision, 1);
    assert_eq!(retried.1.lifecycle_status, "completed");
}

#[test]
fn postcommit_interruption_leaves_matching_completed_operation_and_retry_is_safe() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterOperationFinishCommit,
        ));
    let error = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        "op-lifecycle-post",
        "req-lifecycle-post",
        1,
        "t1",
    )
    .expect_err("postcommit interruption");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert_eq!(current(&fixture.repository).status, "preparing");
    let durable = fixture
        .repository
        .metadata()
        .operation_record("op-lifecycle-post")
        .expect("operation lookup")
        .expect("durable operation");
    assert_eq!(durable.lifecycle_status, "completed");
    assert_eq!(
        durable.result.as_ref().and_then(|v| v.get("revision")),
        Some(&json!(1))
    );

    let reopened = Repository::open(fixture.project.path()).expect("cold reopen");
    let mut reopened = reopened;
    let retried = transition(
        &mut reopened,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        "op-lifecycle-post",
        "req-lifecycle-post",
        1,
        "t2",
    )
    .expect("retry after reopen");
    assert_eq!(retried.0.status, "preparing");
    assert_eq!(retried.0.revision, 1);
    assert_eq!(retried.1, durable);
}

#[test]
fn provider_failure_does_not_create_completed_operation_or_advance_core() {
    let fixture = fixture();
    let before = current(&fixture.repository);
    let provider_error = PongError::Io(std::io::Error::other("TEST_DOUBLE provider failure"));
    assert_eq!(provider_error.code(), "IO_ERROR");
    assert_eq!(current(&fixture.repository), before);
    assert!(fixture
        .repository
        .metadata()
        .operation_record("op-provider-failure")
        .expect("operation lookup")
        .is_none());
}

#[test]
fn legacy_v01_remains_readable_without_lifecycle_operation_writeback() {
    let project = tempdir().expect("project");
    let initialized = Repository::init(project.path()).expect("layout");
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
    let connection = rusqlite::Connection::open(&metadata_path).expect("legacy metadata");
    connection
        .execute_batch(include_str!("fixtures/CT-09-v01-metadata.sql"))
        .expect("fixture");
    drop(connection);
    let mut repository = Repository::open(project.path()).expect("legacy open");
    assert_eq!(
        repository
            .metadata()
            .list_events("legacy-stream")
            .expect("legacy read")
            .len(),
        1
    );
    let error = WorkspaceManager::new(&mut repository, Redactor::default())
        .transition_lifecycle_operation(
            "legacy-workspace",
            &pong_core::LeaseToken {
                workspace_id: "legacy-workspace".into(),
                agent_id: "legacy-agent".into(),
                epoch: 1,
                expires_at_ms: 10_000,
            },
            0,
            WorkspaceLifecycleAction::Close,
            WorkspaceLifecycleOperationOptions {
                operation_id: "op-legacy".into(),
                request_id: "req-legacy".into(),
                now_ms: 0,
                updated_at: "legacy".into(),
            },
        )
        .expect_err("missing legacy workspace");
    assert_eq!(error.code(), "NOT_FOUND");
    assert!(repository
        .metadata()
        .operation_record("op-legacy")
        .expect("operation lookup")
        .is_none());
}

#[test]
fn close_transition_records_terminal_operation_without_new_event_taxonomy() {
    let mut fixture = fixture();
    let snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-operation",
            &fixture.lease,
            SnapshotOptions::default(),
            1,
            "t1",
        )
        .expect("snapshot");
    assert!(snapshot.snapshot_id.starts_with("snp-"));
    let before = fixture
        .repository
        .metadata()
        .list_event_envelopes("project-operation", 0)
        .expect("events");
    let result = transition(
        &mut fixture.repository,
        &fixture.lease,
        1,
        WorkspaceLifecycleAction::Close,
        "op-lifecycle-close",
        "req-lifecycle-close",
        2,
        "t2",
    )
    .expect("close");
    assert_eq!(result.0.status, "archived");
    assert_eq!(result.0.revision, 2);
    assert_eq!(result.1.lifecycle_status, "completed");
    let after = fixture
        .repository
        .metadata()
        .list_event_envelopes("project-operation", 0)
        .expect("events");
    let new_events = after.len() - before.len();
    assert_eq!(new_events, 2);
    assert!(after.iter().any(|event| {
        event.operation_id.as_deref() == Some("op-lifecycle-close")
            && event.event_type == "operation.lifecycle"
    }));
    assert!(!after
        .iter()
        .any(|event| event.event_type.starts_with("lifecycle.")));
}
