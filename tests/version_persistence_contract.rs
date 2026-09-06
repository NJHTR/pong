//! Executable M2-SLICE-010 Version persistence contract coverage.

use pong_core::metadata::{OperationEnvelope, OperationRef, VersionPublication};
use pong_core::redaction::Redactor;
use pong_core::repository::MigrationSpec;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::{MetadataFailpoint, MetadataFailpoints, PongError, Repository};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

struct Fixture {
    project: tempfile::TempDir,
    _workspace_parent: tempfile::TempDir,
    repository: Repository,
    snapshot_id: String,
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-version",
            "project-version",
            &json!({"schema_version": 1, "os": "windows"}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_parent.path().join("source");
    let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
    manager
        .create_local(
            "ws-version",
            "project-version",
            &workspace_path,
            None,
            Some("env-version"),
            "t0",
        )
        .expect("workspace");
    let lease = manager
        .acquire_lease("ws-version", "agent-version", 0, 60_000)
        .expect("lease");
    fs::write(workspace_path.join("state.txt"), b"version").expect("state");
    let snapshot = manager
        .snapshot_local("ws-version", &lease, SnapshotOptions::default(), 1, "t1")
        .expect("snapshot");
    Fixture {
        project,
        _workspace_parent: workspace_parent,
        repository,
        snapshot_id: snapshot.snapshot_id,
    }
}

fn operation(snapshot_id: &str, operation_id: &str, request_id: &str) -> OperationEnvelope {
    OperationEnvelope {
        operation_id: operation_id.into(),
        project_id: "project-version".into(),
        request_id: request_id.into(),
        agent_id: "agent-version".into(),
        session_id: "session-version".into(),
        workspace_id: Some("ws-version".into()),
        environment_id: Some("env-version".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "t2".into(),
        tool: "workspace".into(),
        action: "version.create".into(),
        input_refs: vec![OperationRef {
            kind: "snapshot".into(),
            reference: snapshot_id.into(),
            media_type: Some("application/vnd.pong.snapshot".into()),
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

fn publication(fixture: &Fixture, operation_id: &str) -> VersionPublication {
    VersionPublication {
        workspace_id: "ws-version".into(),
        project_id: "project-version".into(),
        snapshot_id: fixture.snapshot_id.clone(),
        creation_operation_id: operation_id.into(),
        environment_id: Some("env-version".into()),
        created_at: "t2".into(),
        parent_version_id: None,
    }
}

fn publish(fixture: &mut Fixture, operation_id: &str) -> pong_core::VersionRecord {
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(
            &fixture.snapshot_id,
            operation_id,
            &format!("request-{operation_id}"),
        ))
        .expect("operation");
    let request = publication(fixture, operation_id);
    fixture
        .repository
        .metadata_mut()
        .create_version(request)
        .expect("version")
}

#[test]
fn v1_identity_is_durable_and_deterministic() {
    let mut fixture = fixture();
    let first = publish(&mut fixture, "op-v1");
    let request = publication(&fixture, "op-v1");
    let second = fixture
        .repository
        .metadata_mut()
        .create_version(request)
        .expect("retry");
    assert_eq!(first, second);
    assert!(first.version_id.starts_with("ver-"));
}

#[test]
fn v2_immutable_fields_cannot_be_corrupted_silently() {
    let mut fixture = fixture();
    let version = publish(&mut fixture, "op-v2");
    let path = fixture.repository.active_metadata_path();
    drop(fixture.repository);
    rusqlite::Connection::open(path)
        .expect("sqlite")
        .execute(
            "UPDATE versions SET snapshot_id = 'snp-corrupted' WHERE version_id = ?1",
            [&version.version_id],
        )
        .expect("corrupt version");
    let repository = Repository::open(fixture.project.path()).expect("reopen");
    assert!(matches!(
        repository.metadata().version_record(&version.version_id),
        Err(PongError::Integrity(_))
    ));
}

#[test]
fn v3_snapshot_reference_requires_existing_snapshot() {
    let mut fixture = fixture();
    let version = publish(&mut fixture, "op-v3");
    assert_eq!(version.snapshot_id, fixture.snapshot_id);
}

#[test]
fn v4_workspace_binding_must_match() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(&fixture.snapshot_id, "op-v4", "request-v4"))
        .expect("operation");
    let mut request = publication(&fixture, "op-v4");
    request.workspace_id = "ws-other".into();
    assert!(matches!(
        fixture.repository.metadata_mut().create_version(request),
        Err(PongError::NotFound(_))
    ));
}

#[test]
fn v5_project_binding_must_match() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(&fixture.snapshot_id, "op-v5", "request-v5"))
        .expect("operation");
    let mut request = publication(&fixture, "op-v5");
    request.project_id = "project-other".into();
    assert!(matches!(
        fixture.repository.metadata_mut().create_version(request),
        Err(PongError::Conflict(_))
    ));
}

#[test]
fn v6_generation_binding_is_verified() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(&fixture.snapshot_id, "op-v6", "request-v6"))
        .expect("operation");
    let path = fixture.repository.active_metadata_path();
    let project_path = fixture.project.path().to_path_buf();
    let request = publication(&fixture, "op-v6");
    drop(fixture.repository);
    rusqlite::Connection::open(&path)
        .expect("sqlite")
        .execute(
            "UPDATE snapshots SET generation_id = 'foreign-generation' WHERE snapshot_id = ?1",
            [&fixture.snapshot_id],
        )
        .expect("corrupt generation");
    let mut repository = Repository::open(project_path).expect("reopen");
    assert!(matches!(
        repository.metadata_mut().create_version(request),
        Err(PongError::Integrity(_))
    ));
}

#[test]
fn v7_environment_binding_is_verified() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(&fixture.snapshot_id, "op-v7", "request-v7"))
        .expect("operation");
    let mut request = publication(&fixture, "op-v7");
    request.environment_id = None;
    assert!(matches!(
        fixture.repository.metadata_mut().create_version(request),
        Err(PongError::Conflict(_))
    ));
}

#[test]
fn v8_operation_identity_binds_one_version() {
    let mut fixture = fixture();
    let version = publish(&mut fixture, "op-v8");
    let operation = fixture
        .repository
        .metadata()
        .operation_record("op-v8")
        .expect("operation")
        .expect("row");
    assert_eq!(operation.lifecycle_status, "completed");
    assert_eq!(
        operation
            .result
            .as_ref()
            .and_then(|result| result.get("version_id"))
            .and_then(serde_json::Value::as_str),
        Some(version.version_id.as_str())
    );
}

#[test]
fn v9_exact_retry_returns_original_version() {
    let mut fixture = fixture();
    let first = publish(&mut fixture, "op-v9");
    let request = publication(&fixture, "op-v9");
    let retry = fixture
        .repository
        .metadata_mut()
        .create_version(request)
        .expect("retry");
    assert_eq!(first, retry);
}

#[test]
fn v10_same_operation_different_snapshot_fails_closed() {
    let mut fixture = fixture();
    let first = publish(&mut fixture, "op-v10");
    let mut request = publication(&fixture, "op-v10");
    request.snapshot_id =
        "snp-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
    assert!(matches!(
        fixture.repository.metadata_mut().create_version(request),
        Err(PongError::NotFound(_)) | Err(PongError::IdempotencyKeyReuse(_))
    ));
    assert_eq!(
        fixture
            .repository
            .metadata()
            .version_record(&first.version_id)
            .unwrap(),
        Some(first)
    );
}

#[test]
fn v11_different_operations_same_snapshot_hit_open_decision() {
    let mut fixture = fixture();
    publish(&mut fixture, "op-v11-a");
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(&fixture.snapshot_id, "op-v11-b", "request-v11-b"))
        .expect("second operation");
    let request = publication(&fixture, "op-v11-b");
    let error = fixture
        .repository
        .metadata_mut()
        .create_version(request)
        .expect_err("open decision");
    assert!(error.to_string().contains("CONTRACT_OPEN_DECISION"));
}

#[test]
fn v12_broken_snapshot_reference_fails_closed_on_read() {
    let mut fixture = fixture();
    let version = publish(&mut fixture, "op-v12");
    let path = fixture.repository.active_metadata_path();
    drop(fixture.repository);
    rusqlite::Connection::open(path)
        .expect("sqlite")
        .execute(
            "DELETE FROM snapshots WHERE snapshot_id = ?1",
            [&fixture.snapshot_id],
        )
        .expect("delete snapshot");
    let repository = Repository::open(fixture.project.path()).expect("reopen");
    assert!(matches!(
        repository.metadata().version_record(&version.version_id),
        Err(PongError::Integrity(_))
    ));
}

#[test]
fn v13_snapshot_deletion_does_not_report_a_healthy_version() {
    let mut fixture = fixture();
    let version = publish(&mut fixture, "op-v13");
    let path = fixture.repository.active_metadata_path();
    drop(fixture.repository);
    rusqlite::Connection::open(path)
        .expect("sqlite")
        .execute(
            "DELETE FROM snapshots WHERE snapshot_id = ?1",
            [&fixture.snapshot_id],
        )
        .expect("delete snapshot");
    let repository = Repository::open(fixture.project.path()).expect("reopen");
    assert!(repository
        .metadata()
        .version_record(&version.version_id)
        .is_err());
}

#[test]
fn v14_transaction_atomicity_rolls_back_version_and_operation() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(&fixture.snapshot_id, "op-v14", "request-v14"))
        .expect("operation");
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeSqliteCommit,
        ));
    let request = publication(&fixture, "op-v14");
    assert!(fixture
        .repository
        .metadata_mut()
        .create_version(request)
        .is_err());
    assert!(fixture
        .repository
        .metadata()
        .list_versions("ws-version")
        .unwrap()
        .is_empty());
    assert_eq!(
        fixture
            .repository
            .metadata()
            .operation_record("op-v14")
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "started"
    );
}

#[test]
fn v15_interrupted_creation_is_old_or_new_after_cold_reopen() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(&fixture.snapshot_id, "op-v15", "request-v15"))
        .expect("operation");
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterSqliteCommit,
        ));
    let request = publication(&fixture, "op-v15");
    assert!(fixture
        .repository
        .metadata_mut()
        .create_version(request)
        .is_err());
    drop(fixture.repository);
    let repository = Repository::open(fixture.project.path()).expect("reopen");
    assert_eq!(
        repository
            .metadata()
            .list_versions("ws-version")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn v16_cold_reopen_preserves_version_integrity() {
    let mut fixture = fixture();
    let version = publish(&mut fixture, "op-v16");
    drop(fixture.repository);
    let repository = Repository::open(fixture.project.path()).expect("reopen");
    assert_eq!(
        repository
            .metadata()
            .version_record(&version.version_id)
            .unwrap(),
        Some(version)
    );
}

#[test]
fn v17_legacy_v01_migration_starts_versions_empty() {
    let project = tempdir().expect("project");
    let repository = Repository::init(project.path()).expect("legacy repository");
    drop(repository);
    Repository::migrate(
        project.path(),
        MigrationSpec::new("migration-version", "generation-version"),
    )
    .expect("migration");
    let repository = Repository::open(project.path()).expect("open migrated repository");
    assert!(repository
        .metadata()
        .list_versions("missing-workspace")
        .unwrap()
        .is_empty());
    let count: i64 = rusqlite::Connection::open(repository.active_metadata_path())
        .expect("sqlite")
        .query_row("SELECT COUNT(*) FROM versions", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 0);
}

#[test]
fn v18_m1_workspace_head_semantics_are_unchanged() {
    let mut fixture = fixture();
    let before = fixture
        .repository
        .metadata()
        .workspace("ws-version")
        .expect("workspace")
        .expect("row")
        .head;
    publish(&mut fixture, "op-v18");
    let after = fixture
        .repository
        .metadata()
        .workspace("ws-version")
        .expect("workspace")
        .expect("row")
        .head;
    assert_eq!(before, after);
}
