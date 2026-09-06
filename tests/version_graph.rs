//! Durable M2-SLICE-011B Version graph parent integration coverage.
//!
//! These tests exercise the SQLite-backed Version path through Repository and
//! WorkspaceManager. The graph is intentionally bounded to one immutable
//! parent and the existing 010B operation/snapshot semantics.

use pong_core::canonical::canonical_digest;
use pong_core::metadata::{OperationEnvelope, OperationRef, VersionPublication};
use pong_core::redaction::Redactor;
use pong_core::repository::MigrationSpec;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::{MetadataFailpoint, MetadataFailpoints, PongError, Repository, VersionRecord};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

struct Fixture {
    project: tempfile::TempDir,
    _workspace_parent: tempfile::TempDir,
    repository: Repository,
    workspace_path: PathBuf,
    lease: pong_core::LeaseToken,
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-graph",
            "project-graph",
            &json!({"schema_version": 1, "os": "windows"}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_parent.path().join("source");
    let lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws-graph",
                "project-graph",
                &workspace_path,
                None,
                Some("env-graph"),
                "t0",
            )
            .expect("workspace");
        manager
            .acquire_lease("ws-graph", "agent-graph", 0, 1_000_000)
            .expect("lease")
    };
    Fixture {
        project,
        _workspace_parent: workspace_parent,
        repository,
        workspace_path,
        lease,
    }
}

fn snapshot(fixture: &mut Fixture, value: &str, clock: i64) -> String {
    fs::write(fixture.workspace_path.join("state.txt"), value).expect("state");
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-graph",
            &fixture.lease,
            SnapshotOptions::default(),
            clock,
            &format!("t{clock}"),
        )
        .expect("snapshot")
        .snapshot_id
}

fn operation(
    snapshot_id: &str,
    operation_id: &str,
    parent_version_id: Option<&str>,
) -> OperationEnvelope {
    let mut input_refs = vec![OperationRef {
        kind: "snapshot".into(),
        reference: snapshot_id.into(),
        media_type: Some("application/vnd.pong.snapshot".into()),
    }];
    if let Some(parent) = parent_version_id {
        input_refs.push(OperationRef {
            kind: "version".into(),
            reference: parent.into(),
            media_type: Some("application/vnd.pong.version".into()),
        });
    }
    OperationEnvelope {
        operation_id: operation_id.into(),
        project_id: "project-graph".into(),
        request_id: format!("request-{operation_id}"),
        agent_id: "agent-graph".into(),
        session_id: "session-graph".into(),
        workspace_id: Some("ws-graph".into()),
        environment_id: Some("env-graph".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: format!("start-{operation_id}"),
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

fn publish(
    fixture: &mut Fixture,
    snapshot_id: &str,
    operation_id: &str,
    parent_version_id: Option<&str>,
    created_at: &str,
) -> Result<VersionRecord, PongError> {
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(snapshot_id, operation_id, parent_version_id))?;
    fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-graph".into(),
            project_id: "project-graph".into(),
            snapshot_id: snapshot_id.into(),
            creation_operation_id: operation_id.into(),
            environment_id: Some("env-graph".into()),
            created_at: created_at.into(),
            parent_version_id: parent_version_id.map(str::to_owned),
        })
}

fn root(fixture: &mut Fixture, value: &str, index: i64, operation_id: &str) -> VersionRecord {
    let snapshot_id = snapshot(fixture, value, index);
    publish(
        fixture,
        &snapshot_id,
        operation_id,
        None,
        &format!("t{index}"),
    )
    .expect("root version")
}

fn child(
    fixture: &mut Fixture,
    value: &str,
    index: i64,
    operation_id: &str,
    parent: &VersionRecord,
) -> VersionRecord {
    let snapshot_id = snapshot(fixture, value, index);
    publish(
        fixture,
        &snapshot_id,
        operation_id,
        Some(&parent.version_id),
        &format!("t{index}"),
    )
    .expect("child version")
}

fn expected_version_id(
    project_id: &str,
    workspace_id: &str,
    snapshot_id: &str,
    operation_id: &str,
) -> String {
    format!(
        "ver-{}",
        canonical_digest(
            "version/identity/v1",
            &json!({
                "project_id": project_id,
                "workspace_id": workspace_id,
                "snapshot_id": snapshot_id,
                "creation_operation_id": operation_id,
            }),
        )
        .expect("version digest")
        .to_hex()
    )
}

fn assert_code(error: PongError, codes: &[&str]) {
    assert!(
        codes.contains(&error.code()),
        "unexpected {}: {error}",
        error.code()
    );
}

fn corrupt_version_parent(fixture: &Fixture, version_id: &str, parent: Option<&str>) {
    Connection::open(fixture.repository.active_metadata_path())
        .expect("sqlite")
        .execute(
            "UPDATE versions SET parent_version_id = ?1 WHERE version_id = ?2",
            rusqlite::params![parent, version_id],
        )
        .expect("corrupt parent");
}

#[test]
fn root_version_has_null_parent_and_parent_lookup_is_empty() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "root", 1, "op-root");
    assert_eq!(root.parent_version_id, None);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_parent(&root.version_id)
            .unwrap(),
        None
    );
    assert!(fixture
        .repository
        .metadata()
        .get_children(&root.version_id)
        .unwrap()
        .is_empty());
}

#[test]
fn child_version_binds_one_existing_parent() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "root", 1, "op-child-root");
    let child = child(&mut fixture, "child", 2, "op-child", &root);
    assert_eq!(
        child.parent_version_id.as_deref(),
        Some(root.version_id.as_str())
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_parent(&child.version_id)
            .unwrap(),
        Some(root)
    );
}

#[test]
fn three_level_chain_and_children_are_deterministic() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "root", 1, "op-chain-root");
    let middle = child(&mut fixture, "middle", 2, "op-chain-middle", &root);
    let leaf = child(&mut fixture, "leaf", 3, "op-chain-leaf", &middle);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_parent(&leaf.version_id)
            .unwrap(),
        Some(middle.clone())
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_parent(&middle.version_id)
            .unwrap(),
        Some(root.clone())
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_children(&root.version_id)
            .unwrap(),
        vec![middle]
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_children(&leaf.version_id)
            .unwrap(),
        Vec::<VersionRecord>::new()
    );
}

#[test]
fn missing_parent_fails_before_insert_or_operation_completion() {
    let mut fixture = fixture();
    let snapshot_id = snapshot(&mut fixture, "missing-parent", 1);
    let missing = "ver-does-not-exist";
    let error = publish(
        &mut fixture,
        &snapshot_id,
        "op-missing-parent",
        Some(missing),
        "t1",
    )
    .expect_err("missing parent");
    assert_code(error, &["NOT_FOUND"]);
    assert!(fixture
        .repository
        .metadata()
        .list_versions("ws-graph")
        .unwrap()
        .is_empty());
    assert_eq!(
        fixture
            .repository
            .metadata()
            .operation_record("op-missing-parent")
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "started"
    );
}

#[test]
fn self_parent_is_rejected_by_domain_and_schema_guard() {
    let mut fixture = fixture();
    let snapshot_id = snapshot(&mut fixture, "self-parent", 1);
    let operation_id = "op-self-parent";
    let self_id = expected_version_id("project-graph", "ws-graph", &snapshot_id, operation_id);
    let error = publish(
        &mut fixture,
        &snapshot_id,
        operation_id,
        Some(&self_id),
        "t1",
    )
    .expect_err("self parent");
    assert_code(error, &["CONFLICT", "NOT_FOUND"]);
    assert!(fixture
        .repository
        .metadata()
        .version_record(&self_id)
        .unwrap()
        .is_none());
}

#[test]
fn parent_workspace_mismatch_fails_closed() {
    let mut fixture = fixture();
    let parent = root(
        &mut fixture,
        "workspace-mismatch",
        1,
        "op-scope-workspace-root",
    );
    corrupt_version_parent(&fixture, &parent.version_id, None);
    Connection::open(fixture.repository.active_metadata_path())
        .unwrap()
        .execute(
            "UPDATE versions SET workspace_id = 'ws-other' WHERE version_id = ?1",
            [&parent.version_id],
        )
        .unwrap();
    let snapshot_id = snapshot(&mut fixture, "workspace-mismatch-child", 2);
    let error = publish(
        &mut fixture,
        &snapshot_id,
        "op-scope-workspace-child",
        Some(&parent.version_id),
        "t2",
    )
    .expect_err("workspace mismatch");
    assert_code(error, &["CONFLICT"]);
}

#[test]
fn parent_project_mismatch_fails_closed() {
    let mut fixture = fixture();
    let parent = root(&mut fixture, "project-mismatch", 1, "op-scope-project-root");
    Connection::open(fixture.repository.active_metadata_path())
        .unwrap()
        .execute(
            "UPDATE versions SET project_id = 'project-other' WHERE version_id = ?1",
            [&parent.version_id],
        )
        .unwrap();
    let snapshot_id = snapshot(&mut fixture, "project-mismatch-child", 2);
    let error = publish(
        &mut fixture,
        &snapshot_id,
        "op-scope-project-child",
        Some(&parent.version_id),
        "t2",
    )
    .expect_err("project mismatch");
    assert_code(error, &["CONFLICT"]);
}

#[test]
fn parent_environment_mismatch_fails_closed() {
    let mut fixture = fixture();
    let parent = root(&mut fixture, "environment-mismatch", 1, "op-scope-env-root");
    Connection::open(fixture.repository.active_metadata_path())
        .unwrap()
        .execute(
            "UPDATE versions SET environment_id = 'env-other' WHERE version_id = ?1",
            [&parent.version_id],
        )
        .unwrap();
    let snapshot_id = snapshot(&mut fixture, "environment-mismatch-child", 2);
    let error = publish(
        &mut fixture,
        &snapshot_id,
        "op-scope-env-child",
        Some(&parent.version_id),
        "t2",
    )
    .expect_err("environment mismatch");
    assert_code(error, &["CONFLICT"]);
}

#[test]
fn parent_generation_mismatch_fails_closed() {
    let mut fixture = fixture();
    let parent = root(
        &mut fixture,
        "generation-mismatch",
        1,
        "op-scope-generation-root",
    );
    Connection::open(fixture.repository.active_metadata_path())
        .unwrap()
        .execute(
            "UPDATE versions SET generation_id = 'generation-other' WHERE version_id = ?1",
            [&parent.version_id],
        )
        .unwrap();
    let snapshot_id = snapshot(&mut fixture, "generation-mismatch-child", 2);
    let error = publish(
        &mut fixture,
        &snapshot_id,
        "op-scope-generation-child",
        Some(&parent.version_id),
        "t2",
    )
    .expect_err("generation mismatch");
    assert_code(error, &["CONFLICT"]);
}

#[test]
fn parent_migration_mismatch_fails_closed() {
    let mut fixture = fixture();
    let parent = root(
        &mut fixture,
        "migration-mismatch",
        1,
        "op-scope-migration-root",
    );
    Connection::open(fixture.repository.active_metadata_path())
        .unwrap()
        .execute(
            "UPDATE versions SET migration_id = 'migration-other' WHERE version_id = ?1",
            [&parent.version_id],
        )
        .unwrap();
    let snapshot_id = snapshot(&mut fixture, "migration-mismatch-child", 2);
    let error = publish(
        &mut fixture,
        &snapshot_id,
        "op-scope-migration-child",
        Some(&parent.version_id),
        "t2",
    )
    .expect_err("migration mismatch");
    assert_code(error, &["CONFLICT"]);
}

#[test]
fn corrupt_parent_is_never_reported_as_healthy() {
    let mut fixture = fixture();
    let parent = root(&mut fixture, "corrupt-parent", 1, "op-corrupt-parent-root");
    corrupt_version_parent(&fixture, &parent.version_id, Some("ver-missing-ancestor"));
    assert!(matches!(
        fixture
            .repository
            .metadata()
            .version_record(&parent.version_id),
        Err(PongError::Integrity(_)) | Err(PongError::NotFound(_))
    ));
    let snapshot_id = snapshot(&mut fixture, "corrupt-parent-child", 2);
    let error = publish(
        &mut fixture,
        &snapshot_id,
        "op-corrupt-parent-child",
        Some(&parent.version_id),
        "t2",
    )
    .expect_err("corrupt parent");
    assert_code(error, &["INTEGRITY_ERROR", "NOT_FOUND"]);
}

#[test]
fn cycle_in_existing_parent_chain_is_rejected() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "cycle-root", 1, "op-cycle-root");
    let child = child(&mut fixture, "cycle-child", 2, "op-cycle-child", &root);
    corrupt_version_parent(&fixture, &root.version_id, Some(&child.version_id));
    let snapshot_id = snapshot(&mut fixture, "cycle-grandchild", 3);
    let error = publish(
        &mut fixture,
        &snapshot_id,
        "op-cycle-grandchild",
        Some(&child.version_id),
        "t3",
    )
    .expect_err("cycle");
    assert_code(error, &["INTEGRITY_ERROR"]);
}

#[test]
fn parent_binding_is_immutable_after_creation() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "immutable-root", 1, "op-immutable-root");
    let child = child(
        &mut fixture,
        "immutable-child",
        2,
        "op-immutable-child",
        &root,
    );
    Connection::open(fixture.repository.active_metadata_path())
        .unwrap()
        .execute(
            "UPDATE versions SET parent_version_id = NULL WHERE version_id = ?1",
            [&child.version_id],
        )
        .expect("corruption injection");
    assert!(matches!(
        fixture
            .repository
            .metadata()
            .version_record(&child.version_id),
        Err(PongError::Integrity(_))
    ));
    assert_eq!(root.parent_version_id, None);
}

#[test]
fn exact_retry_with_parent_returns_same_version_and_operation_result() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "retry-root", 1, "op-retry-root");
    let child_snapshot = snapshot(&mut fixture, "retry-child", 2);
    let first = publish(
        &mut fixture,
        &child_snapshot,
        "op-retry-child",
        Some(&root.version_id),
        "t2",
    )
    .unwrap();
    let retry = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-graph".into(),
            project_id: "project-graph".into(),
            snapshot_id: child_snapshot,
            creation_operation_id: "op-retry-child".into(),
            environment_id: Some("env-graph".into()),
            created_at: "t2".into(),
            parent_version_id: Some(root.version_id.clone()),
        })
        .unwrap();
    assert_eq!(retry, first);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_versions("ws-graph")
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn wrong_parent_retry_fails_deterministically_without_rewrite() {
    let mut fixture = fixture();
    let root_version = root(&mut fixture, "wrong-parent-root", 1, "op-wrong-parent-root");
    let other = root(
        &mut fixture,
        "wrong-parent-other",
        2,
        "op-wrong-parent-other",
    );
    let snapshot_id = snapshot(&mut fixture, "wrong-parent-child", 3);
    let first = publish(
        &mut fixture,
        &snapshot_id,
        "op-wrong-parent-child",
        Some(&root_version.version_id),
        "t3",
    )
    .unwrap();
    let error = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-graph".into(),
            project_id: "project-graph".into(),
            snapshot_id,
            creation_operation_id: "op-wrong-parent-child".into(),
            environment_id: Some("env-graph".into()),
            created_at: "t3".into(),
            parent_version_id: Some(other.version_id),
        })
        .expect_err("wrong parent retry");
    assert_code(error, &["CONFLICT", "IDEMPOTENCY_KEY_REUSE"]);
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
fn cold_reopen_preserves_parent_edge_and_children() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "reopen-root", 1, "op-reopen-root");
    let child = child(&mut fixture, "reopen-child", 2, "op-reopen-child", &root);
    let project_path = fixture.project.path().to_path_buf();
    drop(fixture.repository);
    let repository = Repository::open(project_path).expect("cold reopen");
    assert_eq!(
        repository.metadata().get_parent(&child.version_id).unwrap(),
        Some(root.clone())
    );
    assert_eq!(
        repository
            .metadata()
            .get_children(&root.version_id)
            .unwrap(),
        vec![child]
    );
}

#[test]
fn duplicate_protection_keeps_one_child_for_one_operation() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "duplicate-root", 1, "op-duplicate-root");
    let snapshot_id = snapshot(&mut fixture, "duplicate-child", 2);
    let first = publish(
        &mut fixture,
        &snapshot_id,
        "op-duplicate-child",
        Some(&root.version_id),
        "t2",
    )
    .unwrap();
    let second = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-graph".into(),
            project_id: "project-graph".into(),
            snapshot_id,
            creation_operation_id: "op-duplicate-child".into(),
            environment_id: Some("env-graph".into()),
            created_at: "t2".into(),
            parent_version_id: Some(root.version_id.clone()),
        })
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .get_children(&root.version_id)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn transaction_failure_rolls_back_parent_version_and_operation_completion() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "atomic-root", 1, "op-atomic-root");
    let snapshot_id = snapshot(&mut fixture, "atomic-child", 2);
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(
            &snapshot_id,
            "op-atomic-child",
            Some(&root.version_id),
        ))
        .unwrap();
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
            workspace_id: "ws-graph".into(),
            project_id: "project-graph".into(),
            snapshot_id,
            creation_operation_id: "op-atomic-child".into(),
            environment_id: Some("env-graph".into()),
            created_at: "t2".into(),
            parent_version_id: Some(root.version_id.clone()),
        })
        .expect_err("transaction fault");
    assert_code(error, &["FAULT_INJECTED"]);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_versions("ws-graph")
            .unwrap(),
        vec![root]
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .operation_record("op-atomic-child")
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "started"
    );
}

#[test]
fn post_commit_interruption_is_old_or_new_and_exact_retry_is_stable() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "post-root", 1, "op-post-root");
    let snapshot_id = snapshot(&mut fixture, "post-child", 2);
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(
            &snapshot_id,
            "op-post-child",
            Some(&root.version_id),
        ))
        .unwrap();
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
            workspace_id: "ws-graph".into(),
            project_id: "project-graph".into(),
            snapshot_id: snapshot_id.clone(),
            creation_operation_id: "op-post-child".into(),
            environment_id: Some("env-graph".into()),
            created_at: "t2".into(),
            parent_version_id: Some(root.version_id.clone()),
        })
        .expect_err("post commit interruption");
    assert_code(error, &["FAULT_INJECTED"]);
    let project_path = fixture.project.path().to_path_buf();
    drop(fixture.repository);
    let mut repository = Repository::open(project_path).expect("reopen");
    let child_id = expected_version_id("project-graph", "ws-graph", &snapshot_id, "op-post-child");
    assert_eq!(
        repository
            .metadata()
            .version_record(&child_id)
            .unwrap()
            .unwrap()
            .parent_version_id,
        Some(root.version_id.clone())
    );
    let retry = repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-graph".into(),
            project_id: "project-graph".into(),
            snapshot_id,
            creation_operation_id: "op-post-child".into(),
            environment_id: Some("env-graph".into()),
            created_at: "t2".into(),
            parent_version_id: Some(root.version_id.clone()),
        })
        .unwrap();
    assert_eq!(retry.version_id, child_id);
}

#[test]
fn parent_operation_reference_is_required_for_non_root() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "operation-ref-root", 1, "op-ref-root");
    let snapshot_id = snapshot(&mut fixture, "operation-ref-child", 2);
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(&snapshot_id, "op-ref-child", None))
        .unwrap();
    let error = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-graph".into(),
            project_id: "project-graph".into(),
            snapshot_id,
            creation_operation_id: "op-ref-child".into(),
            environment_id: Some("env-graph".into()),
            created_at: "t2".into(),
            parent_version_id: Some(root.version_id),
        })
        .expect_err("missing parent ref");
    assert_code(error, &["CONFLICT"]);
}

#[test]
fn different_operation_same_snapshot_remains_open_decision() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "open-decision-root", 1, "op-open-root");
    let snapshot_id = snapshot(&mut fixture, "open-decision-child", 2);
    let child = publish(
        &mut fixture,
        &snapshot_id,
        "op-open-child-a",
        Some(&root.version_id),
        "t2",
    )
    .unwrap();
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(
            &snapshot_id,
            "op-open-child-b",
            Some(&root.version_id),
        ))
        .unwrap();
    let error = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-graph".into(),
            project_id: "project-graph".into(),
            snapshot_id,
            creation_operation_id: "op-open-child-b".into(),
            environment_id: Some("env-graph".into()),
            created_at: "t3".into(),
            parent_version_id: Some(root.version_id),
        })
        .expect_err("open decision");
    assert!(error.to_string().contains("CONTRACT_OPEN_DECISION"));
    assert_eq!(
        fixture
            .repository
            .metadata()
            .version_record(&child.version_id)
            .unwrap(),
        Some(child)
    );
}

#[test]
fn workspace_head_remains_snapshot_root_after_version_graph_writes() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "head-root", 1, "op-head-root");
    let child_snapshot = snapshot(&mut fixture, "head-child", 2);
    let before = fixture
        .repository
        .metadata()
        .workspace("ws-graph")
        .unwrap()
        .unwrap()
        .head;
    let _child = publish(
        &mut fixture,
        &child_snapshot,
        "op-head-child",
        Some(&root.version_id),
        "t2",
    )
    .unwrap();
    let after = fixture
        .repository
        .metadata()
        .workspace("ws-graph")
        .unwrap()
        .unwrap()
        .head;
    assert_eq!(before, after);
    assert_eq!(
        after,
        fixture
            .repository
            .metadata()
            .snapshot_record(&child_snapshot)
            .unwrap()
            .map(|s| s.root_digest)
    );
}

#[test]
fn version_identity_does_not_include_parent() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "identity-root", 1, "op-identity-root");
    let snapshot_id = snapshot(&mut fixture, "identity-child", 2);
    let child = publish(
        &mut fixture,
        &snapshot_id,
        "op-identity-child",
        Some(&root.version_id),
        "t2",
    )
    .unwrap();
    assert_eq!(
        child.version_id,
        expected_version_id(
            "project-graph",
            "ws-graph",
            &snapshot_id,
            "op-identity-child"
        )
    );
}

#[test]
fn v01_migration_starts_with_empty_graph_table() {
    let project = tempdir().expect("project");
    Repository::init(project.path()).expect("legacy repository");
    Repository::migrate(
        project.path(),
        MigrationSpec::new("migration-graph", "generation-graph"),
    )
    .expect("migration");
    let repository = Repository::open(project.path()).expect("open migrated");
    let count: i64 = Connection::open(repository.active_metadata_path())
        .unwrap()
        .query_row("SELECT COUNT(*) FROM versions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn pre_graph_version_row_migrates_to_explicit_root_without_identity_change() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "legacy-row", 1, "op-legacy-row");
    let path = fixture.repository.active_metadata_path();
    let project_path = fixture.project.path().to_path_buf();
    drop(fixture.repository);
    let connection = Connection::open(&path).expect("sqlite");
    connection
        .execute_batch(
            "CREATE TABLE versions_pre_graph (
            version_id TEXT PRIMARY KEY NOT NULL,
            workspace_id TEXT NOT NULL,
            project_id TEXT NOT NULL,
            snapshot_id TEXT NOT NULL,
            creation_operation_id TEXT NOT NULL UNIQUE,
            environment_id TEXT,
            generation_id TEXT NOT NULL,
            migration_id TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        INSERT INTO versions_pre_graph SELECT version_id, workspace_id, project_id,
            snapshot_id, creation_operation_id, environment_id, generation_id,
            migration_id, created_at FROM versions;
        DROP TABLE versions;
        ALTER TABLE versions_pre_graph RENAME TO versions;",
        )
        .expect("old graph schema");
    drop(connection);
    let repository = Repository::open(project_path).expect("additive migration");
    let migrated = repository
        .metadata()
        .version_record(&root.version_id)
        .unwrap()
        .unwrap();
    assert_eq!(migrated.version_id, root.version_id);
    assert_eq!(migrated.parent_version_id, None);
    assert_eq!(migrated.snapshot_id, root.snapshot_id);
}

#[test]
fn existing_version_is_a_root_after_additive_column_migration() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "existing-root", 1, "op-existing-root");
    let path = fixture.repository.active_metadata_path();
    drop(fixture.repository);
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE versions SET parent_version_id = NULL WHERE version_id = ?1",
            [&root.version_id],
        )
        .unwrap();
    drop(connection);
    let repository = Repository::open(fixture.project.path()).unwrap();
    assert_eq!(
        repository.metadata().get_parent(&root.version_id).unwrap(),
        None
    );
}

#[test]
fn chain_depth_sanity_for_ten_nodes() {
    let mut fixture = fixture();
    let mut parent = root(&mut fixture, "depth-0", 1, "op-depth-0");
    for index in 1..10 {
        parent = child(
            &mut fixture,
            &format!("depth-{index}"),
            index + 1,
            &format!("op-depth-{index}"),
            &parent,
        );
    }
    assert!(fixture
        .repository
        .metadata()
        .get_parent(&parent.version_id)
        .unwrap()
        .is_some());
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_versions("ws-graph")
            .unwrap()
            .len(),
        10
    );
}

#[test]
fn chain_depth_sanity_for_one_hundred_nodes() {
    let mut fixture = fixture();
    let mut parent = root(&mut fixture, "depth-0", 1, "op-depth-100-0");
    for index in 1..100 {
        parent = child(
            &mut fixture,
            &format!("depth-{index}"),
            index + 1,
            &format!("op-depth-100-{index}"),
            &parent,
        );
    }
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_versions("ws-graph")
            .unwrap()
            .len(),
        100
    );
}

#[test]
#[ignore = "development-only 1000-node chain sanity; not part of the default regression gate"]
fn chain_depth_sanity_for_one_thousand_nodes() {
    let mut fixture = fixture();
    let mut versions = Vec::with_capacity(1000);
    for index in 0..1000 {
        versions.push(root(
            &mut fixture,
            &format!("depth-{index}"),
            index + 1,
            &format!("op-depth-1000-{index}"),
        ));
    }

    // Build the chain in one direct SQLite transaction only for this
    // development sanity check. Every row was created through the durable
    // root path first; the updates add the same immutable parent binding that
    // the graph path stores, and operation results/refs are kept consistent.
    let connection = Connection::open(fixture.repository.active_metadata_path()).unwrap();
    let transaction = connection.unchecked_transaction().unwrap();
    for (index, version) in versions.iter().enumerate().skip(1) {
        let parent_id = &versions[index - 1].version_id;
        transaction
            .execute(
                "UPDATE versions SET parent_version_id = ?1 WHERE version_id = ?2",
                rusqlite::params![parent_id, version.version_id],
            )
            .unwrap();
        let input_refs_json: String = transaction
            .query_row(
                "SELECT input_refs_json FROM operations WHERE operation_id = ?1",
                [&version.creation_operation_id],
                |row| row.get(0),
            )
            .unwrap();
        let mut input_refs: Vec<OperationRef> = serde_json::from_str(&input_refs_json).unwrap();
        input_refs.push(OperationRef {
            kind: "version".into(),
            reference: parent_id.clone(),
            media_type: Some("application/vnd.pong.version".into()),
        });
        let result_json: String = transaction
            .query_row(
                "SELECT result_json FROM operations WHERE operation_id = ?1",
                [&version.creation_operation_id],
                |row| row.get(0),
            )
            .unwrap();
        let mut result: Value = serde_json::from_str(&result_json).unwrap();
        result["parent_version_id"] = Value::String(parent_id.clone());
        transaction
            .execute(
                "UPDATE operations SET input_refs_json = ?1, result_json = ?2 WHERE operation_id = ?3",
                rusqlite::params![serde_json::to_string(&input_refs).unwrap(), serde_json::to_string(&result).unwrap(), version.creation_operation_id],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    let leaf = fixture
        .repository
        .metadata()
        .version_record(&versions[999].version_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_versions("ws-graph")
            .unwrap()
            .len(),
        1000
    );
    assert_eq!(
        leaf.parent_version_id,
        Some(versions[998].version_id.clone())
    );
}

#[test]
fn failed_parent_validation_does_not_complete_operation() {
    let mut fixture = fixture();
    let root = root(
        &mut fixture,
        "failed-validation-root",
        1,
        "op-failed-validation-root",
    );
    let snapshot_id = snapshot(&mut fixture, "failed-validation-child", 2);
    fixture
        .repository
        .metadata_mut()
        .start_operation(operation(
            &snapshot_id,
            "op-failed-validation-child",
            Some("ver-missing"),
        ))
        .unwrap();
    let error = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-graph".into(),
            project_id: "project-graph".into(),
            snapshot_id,
            creation_operation_id: "op-failed-validation-child".into(),
            environment_id: Some("env-graph".into()),
            created_at: "t2".into(),
            parent_version_id: Some(root.version_id),
        })
        .expect_err("operation parent reference mismatch");
    assert_code(error, &["CONFLICT"]);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .operation_record("op-failed-validation-child")
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "started"
    );
}

#[test]
fn parent_child_lookup_validates_missing_child_parent() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "lookup-root", 1, "op-lookup-root");
    let child = child(&mut fixture, "lookup-child", 2, "op-lookup-child", &root);
    corrupt_version_parent(&fixture, &root.version_id, Some("ver-missing-root-parent"));
    assert!(matches!(
        fixture.repository.metadata().get_children(&root.version_id),
        Err(PongError::Integrity(_))
    ));
    assert!(fixture
        .repository
        .metadata()
        .get_parent(&child.version_id)
        .is_err());
}

#[test]
fn operation_result_retains_parent_binding() {
    let mut fixture = fixture();
    let root = root(&mut fixture, "result-root", 1, "op-result-root");
    let child = child(&mut fixture, "result-child", 2, "op-result-child", &root);
    let operation = fixture
        .repository
        .metadata()
        .operation_record("op-result-child")
        .unwrap()
        .unwrap();
    assert_eq!(
        operation
            .result
            .as_ref()
            .and_then(|value| value.get("parent_version_id"))
            .and_then(Value::as_str),
        Some(root.version_id.as_str())
    );
    assert!(operation
        .output_refs
        .iter()
        .any(|reference| reference.kind == "version" && reference.reference == child.version_id));
}
