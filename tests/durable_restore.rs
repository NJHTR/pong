use pong_core::redaction::Redactor;
use pong_core::workspace::{
    RestoreOptions, SnapshotOptions, WorkspaceFailPoint, WorkspaceFailpoints, WorkspaceFaultAction,
    WorkspaceManager,
};
use pong_core::{MetadataFailpoint, MetadataFailpoints, Repository};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

struct Fixture {
    project: tempfile::TempDir,
    workspace_parent: tempfile::TempDir,
    repository: Repository,
    workspace_path: std::path::PathBuf,
    snapshot_id: String,
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-restore",
            "project-restore",
            &json!({"schema_version": 1}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_parent.path().join("source");
    let lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws-restore",
                "project-restore",
                &workspace_path,
                None,
                Some("env-restore"),
                "t0",
            )
            .expect("workspace");
        manager
            .acquire_lease("ws-restore", "agent-restore", 0, 10_000)
            .expect("lease")
    };
    fs::create_dir(workspace_path.join("nested")).expect("nested");
    fs::write(workspace_path.join("state.txt"), b"durable restore state").expect("state");
    fs::write(workspace_path.join("nested/data.bin"), [1_u8, 2, 3, 4]).expect("data");
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local("ws-restore", &lease, SnapshotOptions::default(), 10, "t1")
        .expect("snapshot");
    Fixture {
        project,
        workspace_parent,
        repository,
        workspace_path,
        snapshot_id: snapshot.snapshot_id,
    }
}

fn options(operation_id: &str, request_id: &str, now: &str) -> RestoreOptions {
    RestoreOptions {
        operation_id: operation_id.into(),
        request_id: request_id.into(),
        agent_id: "agent-restore".into(),
        now: now.into(),
    }
}

#[test]
fn restore_new_destination_persists_operation_and_event() {
    let mut fixture = fixture();
    fs::write(fixture.workspace_path.join("state.txt"), b"changed source").expect("change");
    let destination = fixture.workspace_parent.path().join("restored");
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-1", "req-restore-1", "t2"),
        )
        .expect("restore");
    assert_eq!(result.status, "completed");
    assert_eq!(
        fs::read(destination.join("state.txt")).unwrap(),
        b"durable restore state"
    );
    assert_eq!(
        fs::read(destination.join("nested/data.bin")).unwrap(),
        [1, 2, 3, 4]
    );
    let operation = fixture
        .repository
        .metadata()
        .operation_record("op-restore-1")
        .expect("operation read")
        .expect("operation");
    assert_eq!(operation.action, "snapshot.restore");
    assert_eq!(operation.lifecycle_status, "completed");
    assert_eq!(
        operation.result.as_ref().unwrap()["snapshot_id"],
        fixture.snapshot_id
    );
    let events = fixture
        .repository
        .metadata()
        .list_event_envelopes("project-restore", 0)
        .expect("events");
    let event = events
        .iter()
        .find(|event| event.event_type == "snapshot.restore.completed")
        .expect("restore event");
    assert_eq!(event.operation_id.as_deref(), Some("op-restore-1"));
    assert_eq!(event.workspace_id.as_deref(), Some("ws-restore"));
    assert!(event.payload_json.contains(&fixture.snapshot_id));
}

#[test]
fn restore_cold_reopen_and_retry_are_idempotent() {
    let mut fixture = fixture();
    let destination = fixture.workspace_parent.path().join("cold-restored");
    let first = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-2", "req-restore-2", "t2"),
        )
        .expect("restore");
    let event_count = fixture
        .repository
        .metadata()
        .list_event_envelopes("project-restore", 0)
        .unwrap()
        .iter()
        .filter(|event| event.operation_id.as_deref() == Some("op-restore-2"))
        .count();
    drop(fixture.repository);
    let mut reopened = Repository::open(fixture.project.path()).expect("cold reopen");
    let retry = WorkspaceManager::new(&mut reopened, Redactor::default())
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-2", "req-restore-2", "t3"),
        )
        .expect("retry");
    assert_eq!(retry, first);
    let events = reopened
        .metadata()
        .list_event_envelopes("project-restore", 0)
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.operation_id.as_deref() == Some("op-restore-2"))
            .count(),
        event_count
    );
    assert_eq!(
        fs::read(destination.join("state.txt")).unwrap(),
        b"durable restore state"
    );
}

#[test]
fn existing_destination_is_rejected_without_overwrite() {
    let mut fixture = fixture();
    let destination = fixture.workspace_parent.path().join("existing");
    fs::create_dir(&destination).expect("destination");
    fs::write(destination.join("keep.txt"), b"keep me").expect("keep");
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-3", "req-restore-3", "t2"),
        )
        .expect_err("existing destination");
    assert_eq!(error.code(), "CONFLICT");
    assert_eq!(fs::read(destination.join("keep.txt")).unwrap(), b"keep me");
    let operation = fixture
        .repository
        .metadata()
        .operation_record("op-restore-3")
        .unwrap()
        .unwrap();
    assert_eq!(operation.lifecycle_status, "failed");
    assert_eq!(operation.error.as_ref().unwrap().code, "CONFLICT");
}

#[test]
fn materialization_failure_is_durable_and_retry_can_converge() {
    let mut fixture = fixture();
    let destination = fixture.workspace_parent.path().join("retry-restored");
    let mut manager = WorkspaceManager::new_with_failpoints(
        &mut fixture.repository,
        Redactor::default(),
        WorkspaceFailpoints::once(
            WorkspaceFailPoint::MaterializeFileWrite,
            WorkspaceFaultAction::Fail,
        ),
    );
    let error = manager
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-4", "req-restore-4", "t2"),
        )
        .expect_err("materialization failure");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert!(!destination.exists());
    drop(manager);
    let operation = fixture
        .repository
        .metadata()
        .operation_record("op-restore-4")
        .unwrap()
        .unwrap();
    assert_eq!(operation.lifecycle_status, "failed");
    let retry_error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-4", "req-restore-4", "t3"),
        )
        .expect_err("terminal failed operation is not rewritten");
    assert_eq!(retry_error.code(), "CONFLICT");
}

#[test]
fn parent_sync_failure_is_unknown_and_retry_is_safe() {
    let mut fixture = fixture();
    let destination = fixture.workspace_parent.path().join("unknown-restored");
    let mut manager = WorkspaceManager::new_with_failpoints(
        &mut fixture.repository,
        Redactor::default(),
        WorkspaceFailpoints::once(
            WorkspaceFailPoint::MaterializeParentDirectorySync,
            WorkspaceFaultAction::Fail,
        ),
    );
    let error = manager
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-5", "req-restore-5", "t2"),
        )
        .expect_err("unknown parent sync");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert!(destination.exists());
    let operation = fixture
        .repository
        .metadata()
        .operation_record("op-restore-5")
        .unwrap()
        .unwrap();
    assert_eq!(operation.lifecycle_status, "unknown");
    let retry = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-5", "req-restore-5", "t3"),
        )
        .expect("unknown retry remains inspectable");
    assert_eq!(retry.status, "unknown");
}

#[test]
fn persistence_failure_after_materialization_reconciles_on_retry() {
    let mut fixture = fixture();
    let destination = fixture.workspace_parent.path().join("persistence-retry");
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeOperationFinishCommit,
        ));
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-6", "req-restore-6", "t2"),
        )
        .expect_err("persistence interruption");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert!(destination.exists());
    assert_eq!(
        fixture
            .repository
            .metadata()
            .operation_record("op-restore-6")
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "started"
    );
    let retry = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-6", "req-restore-6", "t3"),
        )
        .expect("reconcile");
    assert_eq!(retry.status, "completed");
    assert_eq!(
        fixture
            .repository
            .metadata()
            .operation_record("op-restore-6")
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "completed"
    );
}

#[test]
fn missing_blob_fails_closed_without_successful_restore() {
    let mut fixture = fixture();
    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.snapshot_id)
        .unwrap()
        .unwrap();
    let digest = pong_core::cas::Digest::from_hex(&snapshot.root_digest[7..]).unwrap();
    let local = pong_core::workspace::LocalWorkspace::open(
        "ws-restore",
        "project-restore",
        &fixture.workspace_path,
        Redactor::default(),
    )
    .unwrap();
    let manifest = local
        .read_manifest(fixture.repository.cas(), digest)
        .unwrap();
    let blob = manifest
        .entries
        .iter()
        .find(|entry| entry.kind == "file")
        .unwrap()
        .digest
        .as_ref()
        .unwrap();
    let blob_digest = pong_core::cas::Digest::from_hex(&blob[7..]).unwrap();
    fs::remove_file(fixture.repository.cas().object_path(blob_digest)).expect("remove blob");
    let destination = fixture.workspace_parent.path().join("missing-blob");
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-7", "req-restore-7", "t2"),
        )
        .expect_err("missing blob");
    assert_eq!(error.code(), "NOT_FOUND");
    assert!(!destination.exists());
    assert_eq!(
        fixture
            .repository
            .metadata()
            .operation_record("op-restore-7")
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "failed"
    );
}

#[test]
fn restore_result_payload_is_structured() {
    let mut fixture = fixture();
    let destination = fixture.workspace_parent.path().join("structured");
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-restore",
            &fixture.snapshot_id,
            &destination,
            options("op-restore-8", "req-restore-8", "t2"),
        )
        .expect("restore");
    let operation = fixture
        .repository
        .metadata()
        .operation_record("op-restore-8")
        .unwrap()
        .unwrap();
    assert_eq!(
        operation.result.as_ref().unwrap()["status"],
        json!("completed")
    );
    assert_eq!(operation.output_refs.len(), 1);
}
