use pong_core::metadata::{OperationEnvelope, OperationRef};
use pong_core::redaction::Redactor;
use pong_core::workspace::{RestoreOptions, SnapshotOptions, WorkspaceManager};
use pong_core::{PongError, Repository};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

struct Fixture {
    project: tempfile::TempDir,
    _workspace_parent: tempfile::TempDir,
    repository: Repository,
    workspace_path: std::path::PathBuf,
    lease: pong_core::LeaseToken,
    snapshot_id: Option<String>,
}

fn fixture(with_snapshot: bool) -> Fixture {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-status",
            "project-status",
            &json!({"schema_version": 1, "os": "test"}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_parent.path().join("source");
    let lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws-status",
                "project-status",
                &workspace_path,
                Some("refs/heads/main"),
                Some("env-status"),
                "t0",
            )
            .expect("workspace");
        manager
            .acquire_lease("ws-status", "agent-status", 0, 10_000)
            .expect("lease")
    };
    fs::write(workspace_path.join("state.txt"), b"status state").expect("state");
    let snapshot_id = if with_snapshot {
        Some(
            WorkspaceManager::new(&mut repository, Redactor::default())
                .snapshot_local("ws-status", &lease, SnapshotOptions::default(), 10, "t1")
                .expect("snapshot")
                .snapshot_id,
        )
    } else {
        None
    };
    Fixture {
        project,
        _workspace_parent: workspace_parent,
        repository,
        workspace_path,
        lease,
        snapshot_id,
    }
}

fn operation(workspace_id: &str) -> OperationEnvelope {
    OperationEnvelope {
        operation_id: "op-status-unknown".into(),
        project_id: "project-status".into(),
        request_id: "req-status-unknown".into(),
        agent_id: "agent-status".into(),
        session_id: "session-status".into(),
        workspace_id: Some(workspace_id.into()),
        environment_id: Some("env-status".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "t2".into(),
        tool: "workspace".into(),
        action: "snapshot.restore".into(),
        input_refs: vec![OperationRef {
            kind: "snapshot".into(),
            reference: "snp-status".into(),
            media_type: None,
        }],
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
fn status_before_snapshot_has_no_comparison_baseline_and_is_read_only() {
    let mut fixture = fixture(false);
    let before = fixture
        .repository
        .metadata()
        .workspace("ws-status")
        .unwrap()
        .unwrap();
    let status = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 100)
        .expect("status");
    assert_eq!(status.workspace_id, "ws-status");
    assert_eq!(status.project_id, "project-status");
    assert_eq!(status.revision, 0);
    assert_eq!(status.head_snapshot_id, None);
    assert_eq!(status.changed, None);
    assert_eq!(status.change_state, "no_snapshot");
    assert_eq!(status.lease.epoch, Some(fixture.lease.epoch));
    assert!(status.lease.active);
    assert_eq!(status.environment.status, "bound");
    assert!(!status.execution_ready);
    assert!(status.healthy);
    let after = fixture
        .repository
        .metadata()
        .workspace("ws-status")
        .unwrap()
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn status_compares_filesystem_to_published_head() {
    let mut fixture = fixture(true);
    let unchanged = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10)
        .expect("status");
    assert_eq!(unchanged.head_snapshot_id, fixture.snapshot_id);
    assert_eq!(unchanged.changed, Some(false));
    assert_eq!(unchanged.change_state, "unchanged");
    assert!(unchanged.execution_ready);
    assert!(unchanged.healthy);
    fs::write(fixture.workspace_path.join("state.txt"), b"changed").expect("change");
    let changed = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10)
        .expect("changed status");
    assert_eq!(changed.changed, Some(true));
    assert_eq!(changed.change_state, "changed");
    assert!(changed.execution_ready);
    assert!(changed.healthy);
}

#[test]
fn status_reports_latest_operation_and_unresolved_state() {
    let mut fixture = fixture(true);
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation("ws-status"))
        .expect("operation");
    let status = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10)
        .expect("status");
    let operation = status.latest_operation.expect("latest operation");
    assert_eq!(operation.operation_id, "op-status-unknown");
    assert_eq!(operation.lifecycle_status, "started");
    assert!(status.recovery_required);
    assert!(!status.healthy);
    assert!(!status.execution_ready);
}

#[test]
fn status_after_restore_keeps_head_and_reports_completed_restore() {
    let mut fixture = fixture(true);
    let destination = fixture._workspace_parent.path().join("restored");
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-status",
            fixture.snapshot_id.as_deref().expect("snapshot"),
            &destination,
            RestoreOptions {
                operation_id: "op-status-restore".into(),
                request_id: "req-status-restore".into(),
                agent_id: "agent-status".into(),
                now: "t2".into(),
            },
        )
        .expect("restore");
    let status = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10)
        .expect("status");
    assert_eq!(status.head_snapshot_id, fixture.snapshot_id);
    assert_eq!(status.changed, Some(false));
    assert_eq!(
        status.latest_operation.as_ref().expect("operation").action,
        "snapshot.restore"
    );
    assert_eq!(
        status
            .latest_operation
            .as_ref()
            .expect("operation")
            .lifecycle_status,
        "completed"
    );
    assert!(!status.recovery_required);
}

#[test]
fn status_lease_expiry_is_not_active_and_does_not_mutate() {
    let mut fixture = fixture(true);
    let status = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10_001)
        .expect("expired status");
    assert!(!status.lease.active);
    assert_eq!(status.lease.agent_id.as_deref(), Some("agent-status"));
    assert_eq!(status.lease.epoch, Some(fixture.lease.epoch));
}

#[test]
fn status_lease_takeover_reports_the_new_epoch_and_owner() {
    let mut fixture = fixture(false);
    let replacement = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .acquire_lease("ws-status", "agent-replacement", 10_001, 10_000)
        .expect("takeover");
    let status = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10_002)
        .expect("status");
    assert_eq!(status.lease.epoch, Some(replacement.epoch));
    assert_eq!(status.lease.agent_id.as_deref(), Some("agent-replacement"));
    assert!(status.lease.active);
}

#[test]
fn status_fails_closed_for_missing_head_snapshot() {
    let mut fixture = fixture(true);
    let head = fixture
        .repository
        .metadata()
        .workspace("ws-status")
        .unwrap()
        .unwrap()
        .head
        .expect("head");
    let connection = Connection::open(fixture.repository.active_metadata_path()).expect("sqlite");
    connection
        .execute("DELETE FROM snapshots WHERE root_digest = ?1", [&head])
        .expect("delete snapshot");
    drop(connection);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10)
        .expect_err("missing snapshot");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}

#[test]
fn status_fails_closed_for_cross_project_environment() {
    let mut fixture = fixture(false);
    let connection = Connection::open(fixture.repository.active_metadata_path()).expect("sqlite");
    connection
        .execute(
            "UPDATE environments SET project_id = 'other-project' WHERE environment_id = 'env-status'",
            [],
        )
        .expect("corrupt environment");
    drop(connection);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10)
        .expect_err("cross-project environment");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}

#[test]
fn status_survives_cold_reopen_with_same_authoritative_state() {
    let mut fixture = fixture(true);
    let first = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10)
        .expect("status");
    drop(fixture.repository);
    let mut reopened = Repository::open(fixture.project.path()).expect("reopen");
    let second = WorkspaceManager::new(&mut reopened, Redactor::default())
        .status("ws-status", 10)
        .expect("reopened status");
    assert_eq!(second, first);
}

#[test]
fn status_serializes_as_a_stable_json_view() {
    let mut fixture = fixture(true);
    let status = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10)
        .expect("status");
    let value = serde_json::to_value(status).expect("serialize");
    assert_eq!(value["workspace_id"], "ws-status");
    assert_eq!(value["changed"], false);
    assert_eq!(value["lease"]["active"], true);
    assert_eq!(value["environment"]["status"], "bound");
}

#[test]
fn status_does_not_require_a_snapshot_for_a_legacy_style_workspace() {
    let mut fixture = fixture(false);
    let status = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-status", 10)
        .expect("legacy-style status");
    assert_eq!(status.change_state, "no_snapshot");
    assert_eq!(status.head_snapshot_id, None);
    assert!(!status.recovery_required);
}

#[test]
fn status_opens_a_raw_v01_repository_without_a_snapshot_head() {
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
    let connection = Connection::open(&metadata_path).expect("v0.1 metadata");
    connection
        .execute_batch(include_str!("fixtures/CT-09-v01-metadata.sql"))
        .expect("v0.1 schema");
    drop(connection);
    let mut repository = Repository::open(project.path()).expect("open v0.1 repository");
    let workspace_parent = tempdir().expect("workspace parent");
    let workspace_path = workspace_parent.path().join("legacy-workspace");
    WorkspaceManager::new(&mut repository, Redactor::default())
        .create_local(
            "ws-legacy-status",
            "legacy-project",
            &workspace_path,
            None,
            None,
            "t0",
        )
        .expect("workspace");
    let status = WorkspaceManager::new(&mut repository, Redactor::default())
        .status("ws-legacy-status", 10)
        .expect("legacy status");
    assert_eq!(status.change_state, "no_snapshot");
    assert_eq!(status.changed, None);
    assert_eq!(status.head_snapshot_id, None);
    assert_eq!(status.environment.status, "unbound");
}

#[test]
fn status_unknown_workspace_is_not_found() {
    let mut fixture = fixture(false);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("missing", 10)
        .expect_err("missing workspace");
    assert!(matches!(error, PongError::NotFound(_)));
}
