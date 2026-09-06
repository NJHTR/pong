//! M2-SLICE-012B durable Version Head integration coverage.

use pong_core::canonical::canonical_digest;
use pong_core::metadata::{OperationEnvelope, OperationRef, VersionPublication, WorkspaceRecord};
use pong_core::redaction::Redactor;
use pong_core::repository::{MigrationSpec, Repository};
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::{MetadataFailpoint, MetadataFailpoints, PongError, VersionRecord};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

struct Fixture {
    project: tempfile::TempDir,
    workspace_parent: tempfile::TempDir,
    repository: Repository,
    workspace_path: PathBuf,
    lease: pong_core::LeaseToken,
    snapshot_id: String,
    version: VersionRecord,
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-reference",
            "project-reference",
            &json!({"schema_version": 1, "os": "windows"}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_parent.path().join("source");
    let lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws-reference",
                "project-reference",
                &workspace_path,
                None,
                Some("env-reference"),
                "t0",
            )
            .expect("workspace");
        manager
            .acquire_lease("ws-reference", "agent-reference", 0, 1_000_000)
            .expect("lease")
    };
    fs::write(workspace_path.join("state.txt"), b"reference").expect("state");
    let snapshot_id = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local("ws-reference", &lease, SnapshotOptions::default(), 1, "t1")
        .expect("snapshot")
        .snapshot_id;
    let version = publish(
        &mut repository,
        VersionPublication {
            workspace_id: "ws-reference".into(),
            project_id: "project-reference".into(),
            snapshot_id: snapshot_id.clone(),
            creation_operation_id: "op-reference-root".into(),
            environment_id: Some("env-reference".into()),
            created_at: "t2".into(),
            parent_version_id: None,
        },
    );
    Fixture {
        project,
        workspace_parent,
        repository,
        workspace_path,
        lease,
        snapshot_id,
        version,
    }
}

fn operation(publication: &VersionPublication) -> OperationEnvelope {
    let mut input_refs = vec![OperationRef {
        kind: "snapshot".into(),
        reference: publication.snapshot_id.clone(),
        media_type: Some("application/vnd.pong.snapshot".into()),
    }];
    if let Some(parent) = publication.parent_version_id.as_deref() {
        input_refs.push(OperationRef {
            kind: "version".into(),
            reference: parent.into(),
            media_type: Some("application/vnd.pong.version".into()),
        });
    }
    OperationEnvelope {
        operation_id: publication.creation_operation_id.clone(),
        project_id: publication.project_id.clone(),
        request_id: format!("request-{}", publication.creation_operation_id),
        agent_id: "agent-reference".into(),
        session_id: "session-reference".into(),
        workspace_id: Some(publication.workspace_id.clone()),
        environment_id: publication.environment_id.clone(),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: format!("start-{}", publication.creation_operation_id),
        tool: "workspace".into(),
        action: "version.create".into(),
        input_refs,
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

fn publish(repository: &mut Repository, publication: VersionPublication) -> VersionRecord {
    repository
        .metadata_mut()
        .start_operation(operation(&publication))
        .expect("version operation");
    repository
        .metadata_mut()
        .create_version(publication)
        .expect("version")
}

fn additional_version(
    fixture: &mut Fixture,
    value: &str,
    index: i64,
    parent: &VersionRecord,
) -> VersionRecord {
    fs::write(fixture.workspace_path.join("state.txt"), value).expect("state");
    let snapshot_id = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-reference",
            &fixture.lease,
            SnapshotOptions::default(),
            index,
            &format!("t{index}"),
        )
        .expect("snapshot")
        .snapshot_id;
    publish(
        &mut fixture.repository,
        VersionPublication {
            workspace_id: "ws-reference".into(),
            project_id: "project-reference".into(),
            snapshot_id,
            creation_operation_id: format!("op-reference-{index}"),
            environment_id: Some("env-reference".into()),
            created_at: format!("t{index}"),
            parent_version_id: Some(parent.version_id.clone()),
        },
    )
}

fn current(fixture: &Fixture) -> WorkspaceRecord {
    fixture
        .repository
        .metadata()
        .workspace("ws-reference")
        .expect("workspace read")
        .expect("workspace")
}

fn set_head(
    fixture: &mut Fixture,
    version_id: Option<String>,
    expected_revision: i64,
    now_ms: i64,
) -> Result<WorkspaceRecord, PongError> {
    fixture.repository.metadata_mut().set_version_head(
        "ws-reference",
        version_id.as_deref(),
        &fixture.lease,
        expected_revision,
        "head-update",
        now_ms,
    )
}

fn set_root_head(
    fixture: &mut Fixture,
    expected_revision: i64,
    now_ms: i64,
) -> Result<WorkspaceRecord, PongError> {
    let version_id = fixture.version.version_id.clone();
    set_head(fixture, Some(version_id), expected_revision, now_ms)
}

fn active_path(fixture: &Fixture) -> PathBuf {
    fixture.repository.active_metadata_path()
}

fn count_rows(fixture: &Fixture, table: &str) -> i64 {
    Connection::open(active_path(fixture))
        .expect("sqlite")
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count")
}

fn expected_version_id(snapshot_id: &str, operation_id: &str) -> String {
    format!(
        "ver-{}",
        canonical_digest(
            "version/identity/v1",
            &json!({
                "project_id": "project-reference",
                "workspace_id": "ws-reference",
                "snapshot_id": snapshot_id,
                "creation_operation_id": operation_id,
            }),
        )
        .expect("digest")
        .to_hex()
    )
}

#[test]
fn r1_version_head_creation() {
    let mut fixture = fixture();
    let before = current(&fixture);
    let selected = set_root_head(&mut fixture, before.revision, 10).expect("head");
    assert_eq!(
        selected.version_head_id,
        Some(fixture.version.version_id.clone())
    );
    assert_eq!(selected.revision, before.revision + 1);
}

#[test]
fn r2_version_head_read() {
    let mut fixture = fixture();
    let revision = current(&fixture).revision;
    set_root_head(&mut fixture, revision, 10).expect("head");
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_current_version("ws-reference")
            .unwrap(),
        Some(fixture.version.clone())
    );
}

#[test]
fn r3_durable_persistence_and_cold_reopen() {
    let mut fixture = fixture();
    let revision = current(&fixture).revision;
    set_root_head(&mut fixture, revision, 10).expect("head");
    let project_path = fixture.project.path().to_path_buf();
    drop(fixture.repository);
    let repository = Repository::open(project_path).expect("reopen");
    assert_eq!(
        repository
            .metadata()
            .get_current_version("ws-reference")
            .unwrap(),
        Some(fixture.version)
    );
}

#[test]
fn r4_missing_version_fails_closed() {
    let fixture = fixture();
    Connection::open(active_path(&fixture))
        .unwrap()
        .execute("UPDATE workspaces SET version_head_id = 'ver-missing' WHERE workspace_id = 'ws-reference'", [])
        .unwrap();
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_current_version("ws-reference")
            .unwrap_err()
            .code(),
        "INTEGRITY_ERROR"
    );
}

#[test]
fn r5_broken_version_fails_closed() {
    let mut fixture = fixture();
    let revision = current(&fixture).revision;
    set_root_head(&mut fixture, revision, 10).expect("head");
    Connection::open(active_path(&fixture))
        .unwrap()
        .execute(
            "DELETE FROM snapshots WHERE snapshot_id = ?1",
            [&fixture.snapshot_id],
        )
        .unwrap();
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_current_version("ws-reference")
            .unwrap_err()
            .code(),
        "INTEGRITY_ERROR"
    );
}

#[test]
fn r6_workspace_mismatch_fails_closed() {
    let mut fixture = fixture();
    let version_id = fixture.version.version_id.clone();
    let other_path = fixture.workspace_parent.path().join("other");
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .create_local(
            "ws-other",
            "project-reference",
            other_path,
            None,
            Some("env-reference"),
            "t0",
        )
        .expect("other workspace");
    let other_lease = fixture
        .repository
        .metadata_mut()
        .acquire_workspace_lease("ws-other", "agent-other", 0, 1_000)
        .unwrap();
    let other = fixture
        .repository
        .metadata()
        .workspace("ws-other")
        .unwrap()
        .unwrap();
    let error = fixture
        .repository
        .metadata_mut()
        .set_version_head(
            "ws-other",
            Some(&version_id),
            &other_lease,
            other.revision,
            "t",
            10,
        )
        .unwrap_err();
    assert_eq!(error.code(), "CONFLICT");
}

#[test]
fn r7_project_mismatch_fails_closed() {
    let mut fixture = fixture();
    let version_id = fixture.version.version_id.clone();
    let other_path = fixture.workspace_parent.path().join("project-other");
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .create_local(
            "ws-project-other",
            "project-other",
            other_path,
            None,
            None,
            "t0",
        )
        .expect("other workspace");
    let lease = fixture
        .repository
        .metadata_mut()
        .acquire_workspace_lease("ws-project-other", "agent-other", 0, 1_000)
        .unwrap();
    let workspace = fixture
        .repository
        .metadata()
        .workspace("ws-project-other")
        .unwrap()
        .unwrap();
    let error = fixture
        .repository
        .metadata_mut()
        .set_version_head(
            "ws-project-other",
            Some(&version_id),
            &lease,
            workspace.revision,
            "t",
            10,
        )
        .unwrap_err();
    assert_eq!(error.code(), "CONFLICT");
}

#[test]
fn r8_environment_mismatch_fails_closed() {
    let mut fixture = fixture();
    Connection::open(active_path(&fixture)).unwrap().execute(
        "UPDATE workspaces SET environment_id = 'env-other' WHERE workspace_id = 'ws-reference'",
        [],
    ).unwrap();
    let revision = current(&fixture).revision;
    let error = set_root_head(&mut fixture, revision, 10).unwrap_err();
    assert_eq!(error.code(), "CONFLICT");
}

#[test]
fn r9_generation_mismatch_fails_closed() {
    let mut fixture = fixture();
    let revision = current(&fixture).revision;
    set_root_head(&mut fixture, revision, 10).expect("head");
    Connection::open(active_path(&fixture))
        .unwrap()
        .execute(
            "UPDATE versions SET generation_id = 'generation-other' WHERE version_id = ?1",
            [&fixture.version.version_id],
        )
        .unwrap();
    let error = fixture
        .repository
        .metadata()
        .get_current_version("ws-reference")
        .unwrap_err();
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}

#[test]
fn r10_migration_mismatch_fails_closed() {
    let mut fixture = fixture();
    let revision = current(&fixture).revision;
    set_root_head(&mut fixture, revision, 10).expect("head");
    Connection::open(active_path(&fixture))
        .unwrap()
        .execute(
            "UPDATE versions SET migration_id = 'migration-other' WHERE version_id = ?1",
            [&fixture.version.version_id],
        )
        .unwrap();
    let error = fixture
        .repository
        .metadata()
        .get_current_version("ws-reference")
        .unwrap_err();
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}

#[test]
fn r11_snapshot_integrity_mismatch_fails_closed() {
    let fixture = fixture();
    Connection::open(active_path(&fixture))
        .unwrap()
        .execute(
            "UPDATE snapshots SET workspace_id = 'ws-other' WHERE snapshot_id = ?1",
            [&fixture.snapshot_id],
        )
        .unwrap();
    let error = fixture
        .repository
        .metadata()
        .version_record(&fixture.version.version_id)
        .unwrap_err();
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}

#[test]
fn r12_detached_version_can_be_selected() {
    let mut fixture = fixture();
    let root = fixture.version.clone();
    let middle = additional_version(&mut fixture, "middle", 2, &root);
    let leaf = additional_version(&mut fixture, "leaf", 3, &middle);
    let before = current(&fixture);
    set_head(
        &mut fixture,
        Some(middle.version_id.clone()),
        before.revision,
        10,
    )
    .expect("head");
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_current_version("ws-reference")
            .unwrap(),
        Some(middle)
    );
    assert_ne!(
        fixture
            .repository
            .metadata()
            .get_current_version("ws-reference")
            .unwrap(),
        Some(leaf)
    );
}

#[test]
fn r13_snapshot_head_is_independent() {
    let mut fixture = fixture();
    let before = current(&fixture);
    set_root_head(&mut fixture, before.revision, 10).expect("head");
    let after = current(&fixture);
    assert_eq!(
        after.head,
        Some(
            fixture
                .repository
                .metadata()
                .snapshot_record(&fixture.snapshot_id)
                .unwrap()
                .unwrap()
                .root_digest,
        )
    );
    assert_eq!(after.version_head_id, Some(fixture.version.version_id));
}

#[test]
fn r14_create_version_does_not_implicitly_select() {
    let fixture = fixture();
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_current_version("ws-reference")
            .unwrap(),
        None
    );
}

#[test]
fn r15_setting_version_head_does_not_change_snapshot_head() {
    let mut fixture = fixture();
    let before = current(&fixture);
    set_root_head(&mut fixture, before.revision, 10).expect("head");
    assert_eq!(current(&fixture).head, before.head);
}

#[test]
fn r16_stale_revision_is_deterministic() {
    let mut fixture = fixture();
    let before = current(&fixture);
    set_root_head(&mut fixture, before.revision, 10).expect("writer B");
    let error = set_head(&mut fixture, None, before.revision, 10).unwrap_err();
    assert_eq!(error.code(), "CONFLICT");
    assert_eq!(
        current(&fixture).version_head_id,
        Some(fixture.version.version_id)
    );
}

#[test]
fn r17_stale_lease_is_rejected() {
    let mut fixture = fixture();
    let revision = current(&fixture).revision;
    let error = set_root_head(&mut fixture, revision, 1_000_001).unwrap_err();
    assert_eq!(error.code(), "CONFLICT");
}

#[test]
fn r18_reads_have_no_side_effects() {
    let mut fixture = fixture();
    let revision = current(&fixture).revision;
    set_root_head(&mut fixture, revision, 10).expect("head");
    let before = current(&fixture);
    let lease_before = fixture
        .repository
        .metadata()
        .workspace_lease("ws-reference")
        .unwrap();
    let operations = count_rows(&fixture, "operations");
    let events = count_rows(&fixture, "event_envelopes");
    let _ = fixture
        .repository
        .metadata()
        .get_current_version("ws-reference")
        .unwrap();
    let _ = fixture
        .repository
        .metadata()
        .version_record(&fixture.version.version_id)
        .unwrap();
    let _ = fixture
        .repository
        .metadata()
        .get_parent(&fixture.version.version_id)
        .unwrap();
    let _ = fixture
        .repository
        .metadata()
        .get_children(&fixture.version.version_id)
        .unwrap();
    assert_eq!(current(&fixture), before);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace_lease("ws-reference")
            .unwrap(),
        lease_before
    );
    assert_eq!(count_rows(&fixture, "operations"), operations);
    assert_eq!(count_rows(&fixture, "event_envelopes"), events);
}

#[test]
fn r19_before_commit_failure_is_atomic() {
    let mut fixture = fixture();
    let before = current(&fixture);
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeSqliteCommit,
        ));
    let error = set_root_head(&mut fixture, before.revision, 10).unwrap_err();
    assert_eq!(error.code(), "FAULT_INJECTED");
    let after = current(&fixture);
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.version_head_id, before.version_head_id);
}

#[test]
fn r20_post_commit_interruption_is_recoverable() {
    let mut fixture = fixture();
    let version_id = fixture.version.version_id.clone();
    let before = current(&fixture);
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterSqliteCommit,
        ));
    let error = set_root_head(&mut fixture, before.revision, 10).unwrap_err();
    assert_eq!(error.code(), "FAULT_INJECTED");
    let path = fixture.project.path().to_path_buf();
    drop(fixture.repository);
    let mut repository = Repository::open(path).expect("reopen");
    assert_eq!(
        repository
            .metadata()
            .get_current_version("ws-reference")
            .unwrap(),
        Some(fixture.version.clone())
    );
    let retry = repository
        .metadata_mut()
        .set_version_head(
            "ws-reference",
            Some(&version_id),
            &fixture.lease,
            before.revision,
            "retry",
            10,
        )
        .expect("retry");
    assert_eq!(retry.revision, before.revision + 1);
}

#[test]
fn r21_cold_reopen_preserves_head_and_snapshot() {
    let mut fixture = fixture();
    let before = current(&fixture);
    set_root_head(&mut fixture, before.revision, 10).expect("head");
    let path = fixture.project.path().to_path_buf();
    drop(fixture.repository);
    let repository = Repository::open(path).expect("reopen");
    let workspace = repository
        .metadata()
        .workspace("ws-reference")
        .unwrap()
        .unwrap();
    assert_eq!(
        workspace.head,
        Some(
            repository
                .metadata()
                .snapshot_record(&fixture.snapshot_id)
                .unwrap()
                .unwrap()
                .root_digest
        )
    );
    assert_eq!(
        repository
            .metadata()
            .get_current_version("ws-reference")
            .unwrap(),
        Some(fixture.version)
    );
}

#[test]
fn r22_exact_retry_returns_same_durable_result() {
    let mut fixture = fixture();
    let before = current(&fixture);
    let first = set_root_head(&mut fixture, before.revision, 10).expect("first");
    let retry = set_root_head(&mut fixture, before.revision, 10).expect("retry");
    assert_eq!(retry, first);
}

#[test]
fn r23_legacy_migration_starts_with_null_version_head() {
    let project = tempdir().expect("project");
    let mut repository = Repository::init(project.path()).expect("legacy");
    let workspace_parent = tempdir().expect("workspace parent");
    let workspace_path = workspace_parent.path().join("legacy");
    let workspace = WorkspaceManager::new(&mut repository, Redactor::default())
        .create_local(
            "ws-legacy-reference",
            "project-legacy-reference",
            &workspace_path,
            None,
            None,
            "t0",
        )
        .expect("workspace");
    let old_head = workspace.head;
    drop(repository);
    Repository::migrate(
        project.path(),
        MigrationSpec::new("migration-reference", "generation-reference"),
    )
    .expect("migration");
    let repository = Repository::open(project.path()).expect("target");
    assert_eq!(
        repository
            .metadata()
            .get_current_version("ws-legacy-reference")
            .unwrap(),
        None
    );
    assert_eq!(
        repository
            .metadata()
            .workspace("ws-legacy-reference")
            .unwrap()
            .unwrap()
            .head,
        old_head
    );
}

#[test]
fn r24_existing_version_remains_unselected() {
    let fixture = fixture();
    let workspace = current(&fixture);
    assert_eq!(workspace.version_head_id, None);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_current_version("ws-reference")
            .unwrap(),
        None
    );
}

#[test]
fn r25_workspace_head_keeps_snapshot_root_meaning() {
    let fixture = fixture();
    let workspace = current(&fixture);
    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.snapshot_id)
        .unwrap()
        .unwrap();
    assert_eq!(workspace.head, Some(snapshot.root_digest));
    assert!(!workspace.head.as_deref().unwrap().starts_with("ver-"));
}

#[test]
fn r26_version_identity_is_unchanged() {
    let fixture = fixture();
    assert_eq!(
        fixture.version.version_id,
        expected_version_id(&fixture.snapshot_id, "op-reference-root")
    );
}

#[test]
fn r27_parent_is_unchanged() {
    let mut fixture = fixture();
    let root = fixture.version.clone();
    let child = additional_version(&mut fixture, "child", 2, &root);
    let revision = current(&fixture).revision;
    set_head(&mut fixture, Some(child.version_id.clone()), revision, 10).expect("head");
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_parent(&child.version_id)
            .unwrap()
            .unwrap()
            .version_id,
        fixture.version.version_id
    );
}

#[test]
fn r28_children_are_unchanged() {
    let mut fixture = fixture();
    let root = fixture.version.clone();
    let child = additional_version(&mut fixture, "child", 2, &root);
    let revision = current(&fixture).revision;
    set_root_head(&mut fixture, revision, 10).expect("head");
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_children(&fixture.version.version_id)
            .unwrap(),
        vec![child]
    );
}

#[test]
fn r29_broken_head_never_auto_repairs() {
    let fixture = fixture();
    Connection::open(active_path(&fixture)).unwrap().execute(
        "UPDATE workspaces SET version_head_id = 'ver-missing' WHERE workspace_id = 'ws-reference'",
        [],
    ).unwrap();
    let error = fixture
        .repository
        .metadata()
        .get_current_version("ws-reference")
        .unwrap_err();
    assert_eq!(error.code(), "INTEGRITY_ERROR");
    let stored: Option<String> = Connection::open(active_path(&fixture))
        .unwrap()
        .query_row(
            "SELECT version_head_id FROM workspaces WHERE workspace_id = 'ws-reference'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored.as_deref(), Some("ver-missing"));
}

#[test]
fn additive_workspace_migration_defaults_version_head_to_null() {
    let project = tempdir().expect("project");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .create_workspace(&WorkspaceRecord {
            workspace_id: "ws-additive".into(),
            project_id: "project-additive".into(),
            driver: "local".into(),
            locator: "C:/additive".into(),
            branch_ref: None,
            head: None,
            version_head_id: None,
            environment_id: None,
            status: "created".into(),
            revision: 0,
            created_at: "t0".into(),
            updated_at: "t0".into(),
        })
        .expect("workspace");
    let path = repository.active_metadata_path();
    drop(repository);
    let connection = Connection::open(&path).expect("sqlite");
    connection
        .execute_batch(
            "ALTER TABLE workspaces RENAME TO workspaces_old;
         CREATE TABLE workspaces (
            workspace_id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            driver TEXT NOT NULL,
            locator TEXT NOT NULL,
            branch_ref TEXT,
            head TEXT,
            environment_id TEXT,
            status TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );
         INSERT INTO workspaces SELECT workspace_id, project_id, driver, locator,
            branch_ref, head, environment_id, status, revision, created_at, updated_at
            FROM workspaces_old;
         DROP TABLE workspaces_old;",
        )
        .expect("old workspace schema");
    drop(connection);
    let repository = Repository::open(project.path()).expect("additive reopen");
    assert_eq!(
        repository
            .metadata()
            .workspace("ws-additive")
            .unwrap()
            .unwrap()
            .version_head_id,
        None
    );
}

#[test]
fn same_snapshot_different_operation_remains_open_decision() {
    let mut fixture = fixture();
    let publication = VersionPublication {
        workspace_id: "ws-reference".into(),
        project_id: "project-reference".into(),
        snapshot_id: fixture.snapshot_id.clone(),
        creation_operation_id: "op-reference-duplicate".into(),
        environment_id: Some("env-reference".into()),
        created_at: "t3".into(),
        parent_version_id: None,
    };
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(&publication))
        .expect("duplicate operation");
    let error = fixture
        .repository
        .metadata_mut()
        .create_version(publication)
        .unwrap_err();
    assert!(error.to_string().contains("CONTRACT_OPEN_DECISION"));
}
