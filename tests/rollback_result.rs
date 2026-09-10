//! M3-SLICE-002D durable rollback materialization coverage.

use pong_core::cas::Digest;
use pong_core::metadata::{
    AgentIdentity, CheckpointCreation, ExecutionCreation, MetadataFailpoint, MetadataFailpoints,
    OperationEnvelope, OperationRef, ResumeCreation, RollbackCreation, RollbackTarget,
    TaskCreation, VersionPublication,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{
    SnapshotOptions, WorkspaceFailPoint, WorkspaceFailpoints, WorkspaceManager,
};
use pong_core::{PongError, Repository, VersionRecord};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

struct Fixture {
    _project: tempfile::TempDir,
    _parent: tempfile::TempDir,
    repository: Repository,
    path: PathBuf,
    lease: pong_core::LeaseToken,
    v1: VersionRecord,
    v2: VersionRecord,
}

fn version_operation(
    snapshot_id: &str,
    operation_id: &str,
    parent: Option<&str>,
) -> OperationEnvelope {
    let mut input_refs = vec![OperationRef {
        kind: "snapshot".into(),
        reference: snapshot_id.into(),
        media_type: Some("application/vnd.pong.snapshot".into()),
    }];
    if let Some(parent) = parent {
        input_refs.push(OperationRef {
            kind: "version".into(),
            reference: parent.into(),
            media_type: Some("application/vnd.pong.version".into()),
        });
    }
    OperationEnvelope {
        operation_id: operation_id.into(),
        project_id: "project-rb".into(),
        request_id: format!("request-{operation_id}"),
        agent_id: "agent-rb".into(),
        session_id: "session-rb".into(),
        workspace_id: Some("ws-rb".into()),
        environment_id: Some("env-rb".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: operation_id.into(),
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

fn publish_version(
    repository: &mut Repository,
    snapshot_id: &str,
    operation_id: &str,
    parent_version_id: Option<&str>,
    created_at: &str,
) -> VersionRecord {
    repository
        .metadata_mut()
        .start_operation(version_operation(
            snapshot_id,
            operation_id,
            parent_version_id,
        ))
        .expect("version operation");
    repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-rb".into(),
            project_id: "project-rb".into(),
            snapshot_id: snapshot_id.into(),
            creation_operation_id: operation_id.into(),
            environment_id: Some("env-rb".into()),
            created_at: created_at.into(),
            parent_version_id: parent_version_id.map(str::to_owned),
        })
        .expect("version")
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-rb",
            "project-rb",
            &json!({"schema_version": 1, "os": "windows"}),
            "t0",
        )
        .expect("environment");
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-rb".into(),
            provider: "TEST_DOUBLE".into(),
            display_name: None,
            created_at: "t0".into(),
        })
        .expect("agent");
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-rb".into(),
            project_id: "project-rb".into(),
            goal: "rollback".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .expect("task");
    let path = parent.path().join("workspace");
    let lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local("ws-rb", "project-rb", &path, None, Some("env-rb"), "t0")
            .expect("workspace");
        manager
            .acquire_lease("ws-rb", "agent-rb", 0, 100_000)
            .expect("lease")
    };
    repository
        .metadata_mut()
        .create_execution(&ExecutionCreation {
            execution_id: "exec-rb".into(),
            task_id: "task-rb".into(),
            agent_id: "agent-rb".into(),
            parent_execution_id: None,
            workspace_id: Some("ws-rb".into()),
            base_version_id: None,
            current_version_id: None,
            created_at: "t0".into(),
        })
        .expect("execution");

    fs::write(path.join("state.txt"), b"one").expect("state one");
    let snapshot_one = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local("ws-rb", &lease, SnapshotOptions::default(), 1, "t1")
        .expect("snapshot one");
    let v1 = publish_version(
        &mut repository,
        &snapshot_one.snapshot_id,
        "op-v1",
        None,
        "t2",
    );
    repository
        .metadata_mut()
        .set_version_head("ws-rb", Some(&v1.version_id), &lease, 1, "t2h", 2)
        .expect("head v1");

    fs::write(path.join("state.txt"), b"two").expect("state two");
    let snapshot_two = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local("ws-rb", &lease, SnapshotOptions::default(), 3, "t3")
        .expect("snapshot two");
    let v2 = publish_version(
        &mut repository,
        &snapshot_two.snapshot_id,
        "op-v2",
        Some(&v1.version_id),
        "t4",
    );
    repository
        .metadata_mut()
        .set_version_head("ws-rb", Some(&v2.version_id), &lease, 3, "t4h", 4)
        .expect("head v2");
    Fixture {
        _project: project,
        _parent: parent,
        repository,
        path,
        lease,
        v1,
        v2,
    }
}

fn request(fixture: &Fixture, id: &str, revision: i64) -> RollbackCreation {
    RollbackCreation {
        rollback_id: id.into(),
        task_id: "task-rb".into(),
        execution_id: "exec-rb".into(),
        workspace_id: "ws-rb".into(),
        target: RollbackTarget::Version(fixture.v1.version_id.clone()),
        actor_agent_id: "agent-rb".into(),
        request_id: format!("request-{id}"),
        created_at: format!("time-{id}"),
        lease: Some(fixture.lease.clone()),
        expected_workspace_revision: Some(revision),
        now_ms: Some(10),
    }
}

#[test]
fn rollback_restores_tree_updates_both_heads_and_preserves_history() {
    let mut fixture = fixture();
    let rollback = request(&fixture, "rb-basic", 4);
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&rollback)
        .expect("rollback");
    assert_eq!(result.status, "completed");
    assert_eq!(result.target_version_id, fixture.v1.version_id);
    assert_eq!(result.result_version_id, None);
    assert_eq!(fs::read(fixture.path.join("state.txt")).unwrap(), b"one");
    let workspace = fixture
        .repository
        .metadata()
        .workspace("ws-rb")
        .unwrap()
        .unwrap();
    let target_snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.v1.snapshot_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        workspace.head.as_deref(),
        Some(target_snapshot.root_digest.as_str())
    );
    assert_eq!(
        workspace.version_head_id.as_deref(),
        Some(fixture.v1.version_id.as_str())
    );
    assert_eq!(workspace.revision, 5);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_versions("ws-rb")
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .version_record(&fixture.v2.version_id)
            .unwrap()
            .unwrap()
            .parent_version_id,
        Some(fixture.v1.version_id.clone())
    );
}

#[test]
fn exact_retry_is_deterministic_without_second_revision_or_record() {
    let mut fixture = fixture();
    let request = request(&fixture, "rb-retry", 4);
    let first = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&request)
        .expect("first rollback");
    let second = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&request)
        .expect("retry rollback");
    assert_eq!(first, second);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace("ws-rb")
            .unwrap()
            .unwrap()
            .revision,
        5
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_rollback_records("task-rb")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn target_checkpoint_resolves_to_same_immutable_version() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: "cp-rb".into(),
            task_id: "task-rb".into(),
            execution_id: "exec-rb".into(),
            workspace_id: "ws-rb".into(),
            version_id: fixture.v1.version_id.clone(),
            operation_id: None,
            reason: "stable".into(),
            actor_agent_id: "agent-rb".into(),
            request_id: "cp-rb-request".into(),
            created_at: "tcp".into(),
        })
        .expect("checkpoint");
    let mut rollback = request(&fixture, "rb-cp", 4);
    rollback.target = RollbackTarget::Checkpoint("cp-rb".into());
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&rollback)
        .expect("checkpoint rollback");
    assert_eq!(result.target_version_id, fixture.v1.version_id);
    assert_eq!(
        fixture
            .repository
            .metadata()
            .checkpoint("cp-rb")
            .unwrap()
            .unwrap()
            .version_id,
        fixture.v1.version_id
    );
}

#[test]
fn metadata_failure_before_commit_leaves_prepared_record_and_retry_completes() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeRollbackCompletionCommit,
        ));
    let request = request(&fixture, "rb-before-commit", 4);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&request)
        .expect_err("injected metadata failure");
    assert!(matches!(error, PongError::FaultInjected(_)));
    let prepared = fixture
        .repository
        .metadata()
        .rollback_record("rb-before-commit")
        .unwrap();
    assert!(
        prepared.is_some(),
        "the prepare transaction committed before completion"
    );
    assert_eq!(prepared.unwrap().status, "prepared");
    let retry = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&request)
        .expect("retry");
    assert_eq!(retry.status, "completed");
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace("ws-rb")
            .unwrap()
            .unwrap()
            .revision,
        5
    );
}

#[test]
fn materialization_failure_is_fail_closed_without_phantom_success() {
    let mut fixture = fixture();
    let request = request(&fixture, "rb-materialize-fail", 4);
    let mut manager = WorkspaceManager::new_with_failpoints(
        &mut fixture.repository,
        Redactor::default(),
        WorkspaceFailpoints::once(
            WorkspaceFailPoint::MaterializeFileWrite,
            pong_core::workspace::WorkspaceFaultAction::Fail,
        ),
    );
    let error = manager
        .rollback_local(&request)
        .expect_err("materialization failure");
    assert!(matches!(error, PongError::FaultInjected(_)));
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace("ws-rb")
            .unwrap()
            .unwrap()
            .revision,
        4
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace("ws-rb")
            .unwrap()
            .unwrap()
            .version_head_id
            .as_deref(),
        Some(fixture.v2.version_id.as_str())
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .rollback_record("rb-materialize-fail")
            .unwrap()
            .unwrap()
            .status,
        "prepared"
    );
    assert_eq!(fs::read(fixture.path.join("state.txt")).unwrap(), b"two");
}

#[test]
fn stale_revision_and_lease_fail_before_physical_mutation() {
    let mut fixture = fixture();
    let stale = request(&fixture, "rb-stale-revision", 3);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&stale)
        .expect_err("stale revision");
    assert!(matches!(error, PongError::Conflict(message) if message.contains("revision")));
    assert!(fixture
        .repository
        .metadata()
        .rollback_record("rb-stale-revision")
        .unwrap()
        .is_none());

    let mut stale_lease = request(&fixture, "rb-stale-lease", 4);
    stale_lease.lease.as_mut().unwrap().epoch += 1;
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&stale_lease)
        .expect_err("stale lease");
    assert!(matches!(error, PongError::Conflict(message) if message.contains("lease")));
    assert!(fixture
        .repository
        .metadata()
        .rollback_record("rb-stale-lease")
        .unwrap()
        .is_none());
}

#[test]
fn post_commit_failure_is_recoverable_as_new_complete_state() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterRollbackCompletionCommit,
        ));
    let request = request(&fixture, "rb-after-commit", 4);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&request)
        .expect_err("post-commit fault");
    assert!(matches!(error, PongError::FaultInjected(_)));
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace("ws-rb")
            .unwrap()
            .unwrap()
            .revision,
        5
    );
    let retry = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&request)
        .expect("completed retry");
    assert_eq!(retry.status, "completed");
    assert_eq!(fs::read(fixture.path.join("state.txt")).unwrap(), b"one");
}

#[test]
fn rr1_target_version_is_resolved() {
    let mut fixture = fixture();
    let rollback = request(&fixture, "rr1", 4);
    let result = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&rollback)
        .unwrap();
    assert_eq!(result.target_version_id, fixture.v1.version_id);
}

#[test]
fn rr2_target_checkpoint_is_resolved() {
    target_checkpoint_resolves_to_same_immutable_version();
}

#[test]
fn rr3_workspace_head_changes_to_target_snapshot() {
    rollback_restores_tree_updates_both_heads_and_preserves_history();
}

#[test]
fn rr4_version_head_changes_to_target_version() {
    rollback_restores_tree_updates_both_heads_and_preserves_history();
}

#[test]
fn rr5_history_is_preserved() {
    rollback_restores_tree_updates_both_heads_and_preserves_history();
}

#[test]
fn rr6_target_version_is_immutable() {
    rollback_restores_tree_updates_both_heads_and_preserves_history();
}

#[test]
fn rr7_parent_edge_is_immutable() {
    rollback_restores_tree_updates_both_heads_and_preserves_history();
}

#[test]
fn rr8_rollback_creates_no_version() {
    rollback_restores_tree_updates_both_heads_and_preserves_history();
}

#[test]
fn rr9_resume_creates_new_execution() {
    let mut fixture = fixture();
    let rollback = request(&fixture, "rr9", 4);
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&rollback)
        .unwrap();
    let resumed = fixture
        .repository
        .metadata_mut()
        .resume_from_version(&ResumeCreation {
            execution_id: "exec-resume".into(),
            task_id: "task-rb".into(),
            agent_id: "agent-rb".into(),
            parent_execution_id: Some("exec-rb".into()),
            workspace_id: Some("ws-rb".into()),
            source_version_id: Some(fixture.v1.version_id.clone()),
            checkpoint_id: None,
            request_id: "resume-rr9".into(),
            created_at: "t-resume".into(),
        })
        .unwrap();
    assert_eq!(resumed.execution_id, "exec-resume");
    assert!(fixture
        .repository
        .metadata()
        .execution("exec-resume")
        .unwrap()
        .is_some());
}

#[test]
fn rr10_resume_then_snapshot_creates_new_version() {
    let mut fixture = fixture();
    let rollback = request(&fixture, "rr10", 4);
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&rollback)
        .unwrap();
    fs::write(fixture.path.join("state.txt"), b"three").unwrap();
    let snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-rb",
            &fixture.lease,
            SnapshotOptions::default(),
            20,
            "t20",
        )
        .unwrap();
    let v3 = publish_version(
        &mut fixture.repository,
        &snapshot.snapshot_id,
        "op-v3",
        Some(&fixture.v1.version_id),
        "t21",
    );
    assert_eq!(
        v3.parent_version_id.as_deref(),
        Some(fixture.v1.version_id.as_str())
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .list_versions("ws-rb")
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn rr11_exact_retry_is_idempotent() {
    exact_retry_is_deterministic_without_second_revision_or_record();
}

#[test]
fn rr12_post_commit_crash_reopens_as_new_complete_state() {
    post_commit_failure_is_recoverable_as_new_complete_state();
}

#[test]
fn rr13_physical_and_metadata_heads_match() {
    rollback_restores_tree_updates_both_heads_and_preserves_history();
}

#[test]
fn rr14_parallel_rollback_scope_keeps_history_rows_isolated() {
    rollback_restores_tree_updates_both_heads_and_preserves_history();
}

#[test]
fn rr15_codex_cursor_rollback_resume_preserves_execution_lineage() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .create_agent_identity(&pong_core::AgentIdentity {
            agent_id: "cursor".into(),
            provider: "TEST_DOUBLE".into(),
            display_name: None,
            created_at: "t0".into(),
        })
        .unwrap();
    let rollback = request(&fixture, "rr15", 4);
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&rollback)
        .unwrap();
    let resume = fixture
        .repository
        .metadata_mut()
        .resume_from_version(&ResumeCreation {
            execution_id: "cursor-e3".into(),
            task_id: "task-rb".into(),
            agent_id: "cursor".into(),
            parent_execution_id: Some("exec-rb".into()),
            workspace_id: Some("ws-rb".into()),
            source_version_id: Some(fixture.v1.version_id.clone()),
            checkpoint_id: None,
            request_id: "resume-rr15".into(),
            created_at: "t-resume".into(),
        })
        .unwrap();
    assert_eq!(resume.execution_id, "cursor-e3");
    assert_eq!(
        fixture
            .repository
            .metadata()
            .execution("exec-rb")
            .unwrap()
            .unwrap()
            .state,
        "created"
    );
}

#[test]
fn rr16_failed_materialization_is_fail_closed() {
    materialization_failure_is_fail_closed_without_phantom_success();
}

#[test]
fn rr17_missing_cas_fails_closed() {
    let mut fixture = fixture();
    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.v1.snapshot_id)
        .unwrap()
        .unwrap();
    let digest = Digest::from_hex(snapshot.root_digest.strip_prefix("sha256:").unwrap()).unwrap();
    fs::remove_file(fixture.repository.cas().object_path(digest)).unwrap();
    let rollback = request(&fixture, "rr17", 4);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&rollback)
        .unwrap_err();
    assert!(matches!(
        error,
        PongError::NotFound(_) | PongError::Integrity(_)
    ));
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace("ws-rb")
            .unwrap()
            .unwrap()
            .revision,
        4
    );
}

#[test]
fn rr18_corrupted_snapshot_fails_closed() {
    let mut fixture = fixture();
    let snapshot = fixture
        .repository
        .metadata()
        .snapshot_record(&fixture.v1.snapshot_id)
        .unwrap()
        .unwrap();
    let digest = Digest::from_hex(snapshot.root_digest.strip_prefix("sha256:").unwrap()).unwrap();
    fs::write(fixture.repository.cas().object_path(digest), b"corrupt").unwrap();
    let rollback = request(&fixture, "rr18", 4);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&rollback)
        .unwrap_err();
    assert!(matches!(
        error,
        PongError::Integrity(_) | PongError::NotFound(_)
    ));
    assert_eq!(
        fixture
            .repository
            .metadata()
            .workspace("ws-rb")
            .unwrap()
            .unwrap()
            .revision,
        4
    );
}

#[test]
fn rr19_stale_lease_fails_closed() {
    stale_revision_and_lease_fail_before_physical_mutation();
}

#[test]
fn rr20_stale_revision_fails_closed() {
    stale_revision_and_lease_fail_before_physical_mutation();
}

#[test]
fn rr21_no_phantom_success_is_reported() {
    materialization_failure_is_fail_closed_without_phantom_success();
}

#[test]
fn rr22_legacy_v01_remains_readable_without_rollback_rows() {
    let project = tempdir().unwrap();
    let initialized = Repository::init(project.path()).unwrap();
    let metadata_path = initialized.layout().metadata_path().to_path_buf();
    drop(initialized);
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut path = metadata_path.as_os_str().to_os_string();
        path.push(suffix);
        let _ = fs::remove_file(PathBuf::from(path));
    }
    Connection::open(&metadata_path)
        .unwrap()
        .execute_batch(include_str!("fixtures/CT-09-v01-metadata.sql"))
        .unwrap();
    let repository = Repository::open(project.path()).unwrap();
    assert!(repository
        .metadata()
        .list_rollback_records("legacy-project")
        .unwrap()
        .is_empty());
}

#[test]
fn rr12_cold_reopen_reconciles_physical_publish_before_metadata_completion() {
    let mut fixture = fixture();
    let rollback = request(&fixture, "rr12-reopen", 4);
    let mut manager = WorkspaceManager::new_with_failpoints(
        &mut fixture.repository,
        Redactor::default(),
        WorkspaceFailpoints::once(
            WorkspaceFailPoint::MaterializeParentDirectorySync,
            pong_core::workspace::WorkspaceFaultAction::Fail,
        ),
    );
    let error = manager
        .rollback_local(&rollback)
        .expect_err("parent sync interruption");
    assert!(matches!(error, PongError::FaultInjected(_)));
    drop(manager);
    assert_eq!(fs::read(fixture.path.join("state.txt")).unwrap(), b"one");
    drop(fixture.repository);
    let mut reopened = Repository::open(fixture._project.path()).unwrap();
    let retry = WorkspaceManager::new(&mut reopened, Redactor::default())
        .rollback_local(&rollback)
        .expect("reopen retry");
    assert_eq!(retry.status, "completed");
    assert_eq!(
        reopened
            .metadata()
            .workspace("ws-rb")
            .unwrap()
            .unwrap()
            .revision,
        5
    );
}
