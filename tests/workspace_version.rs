use pong_core::metadata::{OperationEnvelope, OperationRef, VersionPublication};
use pong_core::redaction::Redactor;
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

fn version_operation(snapshot_id: &str, operation_id: &str, request_id: &str) -> OperationEnvelope {
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

fn publish(
    fixture: &mut Fixture,
    operation_id: &str,
    request_id: &str,
) -> pong_core::VersionRecord {
    fixture
        .repository
        .metadata_mut()
        .start_operation(version_operation(
            &fixture.snapshot_id,
            operation_id,
            request_id,
        ))
        .expect("operation");
    fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-version".into(),
            project_id: "project-version".into(),
            snapshot_id: fixture.snapshot_id.clone(),
            creation_operation_id: operation_id.into(),
            environment_id: Some("env-version".into()),
            created_at: "t2".into(),
            parent_version_id: None,
        })
        .expect("version")
}

#[test]
fn creates_deterministic_immutable_version_with_bindings() {
    let mut fixture = fixture();
    let version = publish(&mut fixture, "op-version-1", "req-version-1");
    assert!(version.version_id.starts_with("ver-"));
    assert_eq!(version.workspace_id, "ws-version");
    assert_eq!(version.project_id, "project-version");
    assert_eq!(version.snapshot_id, fixture.snapshot_id);
    assert_eq!(version.creation_operation_id, "op-version-1");
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_versions("ws-version")
            .unwrap(),
        vec![version.clone()]
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .version_record(&version.version_id)
            .unwrap(),
        Some(version)
    );
}

#[test]
fn exact_retry_returns_same_row_and_does_not_duplicate() {
    let mut fixture = fixture();
    let first = publish(&mut fixture, "op-version-2", "req-version-2");
    let second = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-version".into(),
            project_id: "project-version".into(),
            snapshot_id: fixture.snapshot_id.clone(),
            creation_operation_id: "op-version-2".into(),
            environment_id: Some("env-version".into()),
            created_at: "t2".into(),
            parent_version_id: None,
        })
        .expect("retry");
    assert_eq!(first, second);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_versions("ws-version")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn same_operation_changed_snapshot_fails_closed() {
    let mut fixture = fixture();
    let first = publish(&mut fixture, "op-version-3", "req-version-3");
    let error = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: first.workspace_id.clone(),
            project_id: first.project_id.clone(),
            snapshot_id: "snp-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .into(),
            creation_operation_id: first.creation_operation_id.clone(),
            environment_id: first.environment_id.clone(),
            created_at: first.created_at.clone(),
            parent_version_id: first.parent_version_id.clone(),
        })
        .expect_err("changed snapshot");
    assert!(matches!(
        error,
        PongError::NotFound(_) | PongError::IdempotencyKeyReuse(_)
    ));
}

#[test]
fn different_operation_same_snapshot_hits_open_decision_boundary() {
    let mut fixture = fixture();
    publish(&mut fixture, "op-version-4a", "req-version-4a");
    fixture
        .repository
        .metadata_mut()
        .start_operation(version_operation(
            &fixture.snapshot_id,
            "op-version-4b",
            "req-version-4b",
        ))
        .expect("second operation");
    let error = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-version".into(),
            project_id: "project-version".into(),
            snapshot_id: fixture.snapshot_id.clone(),
            creation_operation_id: "op-version-4b".into(),
            environment_id: Some("env-version".into()),
            created_at: "t3".into(),
            parent_version_id: None,
        })
        .expect_err("open decision");
    assert!(error.to_string().contains("CONTRACT_OPEN_DECISION"));
}

#[test]
fn precommit_failure_rolls_back_version_and_operation() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .start_operation(version_operation(
            &fixture.snapshot_id,
            "op-version-5",
            "req-version-5",
        ))
        .expect("operation");
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeSqliteCommit,
        ));
    let error = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-version".into(),
            project_id: "project-version".into(),
            snapshot_id: fixture.snapshot_id.clone(),
            creation_operation_id: "op-version-5".into(),
            environment_id: Some("env-version".into()),
            created_at: "t2".into(),
            parent_version_id: None,
        })
        .expect_err("fault");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_versions("ws-version")
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .operation_record("op-version-5")
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "started"
    );
}

#[test]
fn postcommit_failure_is_recoverable_after_cold_reopen() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .start_operation(version_operation(
            &fixture.snapshot_id,
            "op-version-6",
            "req-version-6",
        ))
        .expect("operation");
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterSqliteCommit,
        ));
    let error = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-version".into(),
            project_id: "project-version".into(),
            snapshot_id: fixture.snapshot_id.clone(),
            creation_operation_id: "op-version-6".into(),
            environment_id: Some("env-version".into()),
            created_at: "t2".into(),
            parent_version_id: None,
        })
        .expect_err("postcommit fault");
    assert_eq!(error.code(), "FAULT_INJECTED");
    drop(fixture.repository);
    let reopened = Repository::open(fixture.project.path()).expect("reopen");
    let version = reopened
        .metadata()
        .list_versions("ws-version")
        .expect("versions")
        .pop()
        .expect("version");
    assert_eq!(
        reopened
            .metadata()
            .operation_record("op-version-6")
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "completed"
    );
    assert_eq!(
        reopened
            .metadata()
            .version_record(&version.version_id)
            .unwrap(),
        Some(version)
    );
}

#[test]
fn missing_operation_and_snapshot_are_rejected_without_rows() {
    let mut fixture = fixture();
    let missing = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-version".into(),
            project_id: "project-version".into(),
            snapshot_id: fixture.snapshot_id.clone(),
            creation_operation_id: "missing-operation".into(),
            environment_id: Some("env-version".into()),
            created_at: "t2".into(),
            parent_version_id: None,
        })
        .expect_err("missing op");
    assert_eq!(missing.code(), "NOT_FOUND");
    assert!(fixture
        .repository
        .metadata()
        .list_versions("ws-version")
        .unwrap()
        .is_empty());
}

#[test]
fn migration_creates_empty_versions_table_without_synthetic_rows() {
    let project = tempdir().expect("project");
    let repository = Repository::init(project.path()).expect("repository");
    let count: i64 = rusqlite::Connection::open(repository.layout().metadata_path())
        .expect("sqlite")
        .query_row("SELECT COUNT(*) FROM versions", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 0);
}
