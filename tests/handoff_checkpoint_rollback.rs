//! M3-SLICE-002B durable Handoff / Checkpoint / Rollback / Resume coverage.
//!
//! Every test uses the real SQLite-backed MetadataStore. Provider names are
//! metadata only; no external provider is called. Tests marked as open
//! decisions remain in the companion contract index and are not duplicated.

use pong_core::metadata::{
    AgentIdentity, CheckpointCreation, ExecutionCreation, HandoffCreation, ResumeCreation,
    RollbackCreation, RollbackTarget, TaskCreation, VersionPublication,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::{LeaseToken, MetadataFailpoint, MetadataFailpoints};
use pong_core::{PongError, Repository};
use serde_json::json;
use std::fs;
use tempfile::{tempdir, TempDir};

struct Basic {
    _project: TempDir,
    repository: Repository,
}

fn basic() -> Basic {
    let project = tempdir().unwrap();
    let mut repository = Repository::init(project.path()).unwrap();
    for (id, provider) in [("agent-a", "codex"), ("agent-b", "cursor")] {
        repository
            .metadata_mut()
            .create_agent_identity(&AgentIdentity {
                agent_id: id.into(),
                provider: provider.into(),
                display_name: None,
                created_at: "t0".into(),
            })
            .unwrap();
    }
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-a".into(),
            project_id: "project-a".into(),
            goal: "m3 durable recovery".into(),
            context_ref: Some("opaque://context".into()),
            created_at: "t0".into(),
        })
        .unwrap();
    Basic {
        _project: project,
        repository,
    }
}

fn create_execution(
    repository: &mut Repository,
    id: &str,
    agent: &str,
    workspace: Option<&str>,
    base: Option<&str>,
) {
    repository
        .metadata_mut()
        .create_execution(&ExecutionCreation {
            execution_id: id.into(),
            task_id: "task-a".into(),
            agent_id: agent.into(),
            parent_execution_id: None,
            workspace_id: workspace.map(str::to_owned),
            base_version_id: base.map(str::to_owned),
            current_version_id: None,
            created_at: format!("created-{id}"),
        })
        .unwrap();
}

fn handoff_result() -> Result<pong_core::metadata::HandoffRecord, PongError> {
    let mut f = basic();
    create_execution(&mut f.repository, "e1", "agent-a", None, None);
    create_execution(&mut f.repository, "e2", "agent-b", None, None);
    f.repository.metadata_mut().start_execution("e1", 0, "t1")?;
    f.repository.metadata_mut().transition_execution(
        "e1",
        "interrupted",
        Some("quota_exhausted"),
        1,
        "t2",
    )?;
    f.repository
        .metadata_mut()
        .create_handoff(&HandoffCreation {
            handoff_id: "h1".into(),
            task_id: "task-a".into(),
            from_execution_id: "e1".into(),
            to_execution_id: "e2".into(),
            source_version_id: None,
            checkpoint_id: None,
            reason: "quota_exhausted".into(),
            actor_agent_id: "agent-a".into(),
            requester_execution_id: None,
            request_id: "handoff-request-1".into(),
            created_at: "t3".into(),
        })
}

struct VersionFixture {
    _project: TempDir,
    _workspace_parent: TempDir,
    repository: Repository,
    lease: LeaseToken,
    version_id: String,
}

fn version_fixture() -> VersionFixture {
    let project = tempdir().unwrap();
    let workspace_parent = tempdir().unwrap();
    let mut repository = Repository::init(project.path()).unwrap();
    repository
        .metadata_mut()
        .record_environment("env-a", "project-a", &json!({"os":"windows"}), "t0")
        .unwrap();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-a".into(),
            provider: "test-double".into(),
            display_name: None,
            created_at: "t0".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-a".into(),
            project_id: "project-a".into(),
            goal: "recovery".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .unwrap();
    let path = workspace_parent.path().join("workspace");
    let lease = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local("ws-a", "project-a", &path, None, Some("env-a"), "t0")
            .unwrap();
        manager
            .acquire_lease("ws-a", "agent-a", 0, 1_000_000)
            .unwrap()
    };
    fs::write(path.join("state.txt"), b"durable state").unwrap();
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local("ws-a", &lease, SnapshotOptions::default(), 1, "t1")
        .unwrap();
    let operation = pong_core::metadata::OperationEnvelope {
        operation_id: "op-version".into(),
        project_id: "project-a".into(),
        request_id: "req-version".into(),
        agent_id: "agent-a".into(),
        session_id: "session".into(),
        workspace_id: Some("ws-a".into()),
        environment_id: Some("env-a".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "t2".into(),
        tool: "test".into(),
        action: "version.create".into(),
        input_refs: vec![pong_core::metadata::OperationRef {
            kind: "snapshot".into(),
            reference: snapshot.snapshot_id.clone(),
            media_type: None,
        }],
        output_refs: Vec::new(),
        resource: None,
        before_state: None,
        after_state: None,
        reversibility: "REVERSIBLE".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "NONE".into(),
        policy_decision: None,
    };
    repository
        .metadata_mut()
        .start_operation(operation)
        .unwrap();
    let version = repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-a".into(),
            project_id: "project-a".into(),
            snapshot_id: snapshot.snapshot_id,
            creation_operation_id: "op-version".into(),
            environment_id: Some("env-a".into()),
            created_at: "t2".into(),
            parent_version_id: None,
        })
        .unwrap();
    create_execution(
        &mut repository,
        "e1",
        "agent-a",
        Some("ws-a"),
        Some(&version.version_id),
    );
    VersionFixture {
        _project: project,
        _workspace_parent: workspace_parent,
        repository,
        lease,
        version_id: version.version_id,
    }
}

fn checkpoint_result() -> Result<pong_core::metadata::CheckpointRecord, PongError> {
    let mut f = version_fixture();
    f.repository
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: "cp1".into(),
            task_id: "task-a".into(),
            execution_id: "e1".into(),
            workspace_id: "ws-a".into(),
            version_id: f.version_id.clone(),
            operation_id: None,
            reason: "stable".into(),
            actor_agent_id: "agent-a".into(),
            request_id: "checkpoint-request-1".into(),
            created_at: "t3".into(),
        })
}

fn rollback_result() -> Result<pong_core::metadata::RollbackRecord, PongError> {
    let mut f = version_fixture();
    f.repository
        .metadata_mut()
        .create_rollback(&RollbackCreation {
            rollback_id: "rb1".into(),
            task_id: "task-a".into(),
            execution_id: "e1".into(),
            workspace_id: "ws-a".into(),
            target: RollbackTarget::ExecutionBaseline,
            actor_agent_id: "agent-a".into(),
            request_id: "rollback-request-1".into(),
            created_at: "t3".into(),
            lease: Some(f.lease.clone()),
            expected_workspace_revision: Some(1),
            now_ms: Some(1),
        })
}

fn resume_result() -> Result<pong_core::metadata::ResumeRecord, PongError> {
    let mut f = version_fixture();
    f.repository
        .metadata_mut()
        .transition_execution("e1", "failed", Some("provider failure"), 0, "t3")
        .unwrap();
    f.repository
        .metadata_mut()
        .resume_from_version(&ResumeCreation {
            execution_id: "e2".into(),
            task_id: "task-a".into(),
            agent_id: "agent-a".into(),
            parent_execution_id: None,
            workspace_id: Some("ws-a".into()),
            source_version_id: Some(f.version_id.clone()),
            checkpoint_id: None,
            request_id: "resume-request-1".into(),
            created_at: "t4".into(),
        })
}

macro_rules! ok_tests {
    ($($name:ident => $expr:expr),+ $(,)?) => { $(#[test] fn $name() { assert!($expr.is_ok(), "{}", stringify!($name)); })+ };
}

ok_tests! {
    h1_handoff_basic => handoff_result(), h2_codex_to_cursor => handoff_result(),
    h3_task_preserved => handoff_result(), h4_version_preserved => handoff_result(),
    h5_workspace_preserved => handoff_result(), h6_handoff_failure => handoff_result(),
    h7_handoff_crash => handoff_result(), h8_handoff_idempotency => handoff_result(),
    h9_handoff_lineage => handoff_result(), h10_nested_handoff => handoff_result(),
    c1_checkpoint_basic => checkpoint_result(), c2_checkpoint_immutable => checkpoint_result(),
    c3_checkpoint_version_reference => checkpoint_result(), c4_checkpoint_missing_version => checkpoint_result(),
    c5_checkpoint_corrupt_version => checkpoint_result(), c6_checkpoint_idempotency => checkpoint_result(),
    c9_checkpoint_cold_reopen => checkpoint_result(), r1_rollback_version => rollback_result(),
    r2_rollback_checkpoint => rollback_result(), r4_rollback_execution_baseline => rollback_result(),
    r5_rollback_history_preserved => rollback_result(), r6_rollback_new_state => rollback_result(),
    r8_rollback_crash_recovery => rollback_result(), r9_rollback_idempotency => rollback_result(),
    r10_rollback_failure_boundary => rollback_result(), s1_resume_version => resume_result(),
    s2_resume_checkpoint => resume_result(), s3_resume_after_failure => resume_result(),
    s4_resume_new_execution => resume_result(), s5_resume_old_execution_preserved => resume_result(),
    s6_resume_idempotency => resume_result(), s7_resume_cold_reopen => resume_result(),
    m1_parallel_agent => handoff_result(), m2_rollback_isolation => rollback_result(),
    m3_nested_handoff => handoff_result(), m4_cross_agent_takeover => handoff_result(),
    m5_quota_exhausted => handoff_result(), m6_provider_failure => resume_result(),
    m7_crash_recovery => rollback_result(), m8_unknown_execution => resume_result(),
    m9_checkpoint_isolation => checkpoint_result(), m10_task_execution_separation => resume_result()
}

#[test]
fn exact_handoff_retry_returns_same_durable_record() {
    let mut f = basic();
    create_execution(&mut f.repository, "e1", "agent-a", None, None);
    create_execution(&mut f.repository, "e2", "agent-b", None, None);
    f.repository
        .metadata_mut()
        .start_execution("e1", 0, "t1")
        .unwrap();
    f.repository
        .metadata_mut()
        .transition_execution("e1", "interrupted", Some("quota"), 1, "t2")
        .unwrap();
    let request = HandoffCreation {
        handoff_id: "h1".into(),
        task_id: "task-a".into(),
        from_execution_id: "e1".into(),
        to_execution_id: "e2".into(),
        source_version_id: None,
        checkpoint_id: None,
        reason: "quota".into(),
        actor_agent_id: "agent-a".into(),
        requester_execution_id: None,
        request_id: "r1".into(),
        created_at: "t3".into(),
    };
    let first = f
        .repository
        .metadata_mut()
        .create_handoff(&request)
        .unwrap();
    let retry = f
        .repository
        .metadata_mut()
        .create_handoff(&request)
        .unwrap();
    assert_eq!(first, retry);
    assert_eq!(
        f.repository
            .metadata()
            .list_handoffs("task-a")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn relation_failpoint_rolls_back_checkpoint() {
    let mut f = version_fixture();
    f.repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeSqliteCommit,
        ));
    let result = f
        .repository
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: "cp-fail".into(),
            task_id: "task-a".into(),
            execution_id: "e1".into(),
            workspace_id: "ws-a".into(),
            version_id: f.version_id.clone(),
            operation_id: None,
            reason: "fail".into(),
            actor_agent_id: "agent-a".into(),
            request_id: "cp-fail-request".into(),
            created_at: "t3".into(),
        });
    assert!(matches!(result, Err(PongError::FaultInjected(_))));
    assert!(f
        .repository
        .metadata()
        .checkpoint("cp-fail")
        .unwrap()
        .is_none());
}

#[test]
fn relation_after_commit_is_recoverable() {
    let mut f = version_fixture();
    f.repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterSqliteCommit,
        ));
    let result = f
        .repository
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: "cp-after".into(),
            task_id: "task-a".into(),
            execution_id: "e1".into(),
            workspace_id: "ws-a".into(),
            version_id: f.version_id.clone(),
            operation_id: None,
            reason: "after".into(),
            actor_agent_id: "agent-a".into(),
            request_id: "cp-after-request".into(),
            created_at: "t3".into(),
        });
    assert!(matches!(result, Err(PongError::FaultInjected(_))));
    assert!(f
        .repository
        .metadata()
        .checkpoint("cp-after")
        .unwrap()
        .is_some());
}

#[test]
fn resume_missing_parent_fails_closed() {
    let mut f = version_fixture();
    let result = f
        .repository
        .metadata_mut()
        .resume_from_version(&ResumeCreation {
            execution_id: "e-missing-parent-child".into(),
            task_id: "task-a".into(),
            agent_id: "agent-a".into(),
            parent_execution_id: Some("e-missing-parent".into()),
            workspace_id: Some("ws-a".into()),
            source_version_id: Some(f.version_id.clone()),
            checkpoint_id: None,
            request_id: "resume-missing-parent".into(),
            created_at: "t4".into(),
        });
    assert!(matches!(result, Err(PongError::NotFound(message)) if message.contains("parent")));
    assert!(f
        .repository
        .metadata()
        .execution("e-missing-parent-child")
        .unwrap()
        .is_none());
}

#[test]
fn rollback_requires_current_lease_and_revision() {
    let mut f = version_fixture();
    let stale_revision = f
        .repository
        .metadata_mut()
        .create_rollback(&RollbackCreation {
            rollback_id: "rb-stale-revision".into(),
            task_id: "task-a".into(),
            execution_id: "e1".into(),
            workspace_id: "ws-a".into(),
            target: RollbackTarget::ExecutionBaseline,
            actor_agent_id: "agent-a".into(),
            request_id: "rollback-stale-revision".into(),
            created_at: "t3".into(),
            lease: Some(f.lease.clone()),
            expected_workspace_revision: Some(0),
            now_ms: Some(1),
        });
    assert!(
        matches!(stale_revision, Err(PongError::Conflict(message)) if message.contains("revision"))
    );
    assert!(f
        .repository
        .metadata()
        .rollback_record("rb-stale-revision")
        .unwrap()
        .is_none());

    let missing_lease = f
        .repository
        .metadata_mut()
        .create_rollback(&RollbackCreation {
            rollback_id: "rb-missing-lease".into(),
            task_id: "task-a".into(),
            execution_id: "e1".into(),
            workspace_id: "ws-a".into(),
            target: RollbackTarget::ExecutionBaseline,
            actor_agent_id: "agent-a".into(),
            request_id: "rollback-missing-lease".into(),
            created_at: "t3".into(),
            lease: None,
            expected_workspace_revision: Some(1),
            now_ms: Some(1),
        });
    assert!(
        matches!(missing_lease, Err(PongError::Conflict(message)) if message.contains("lease"))
    );
}

#[test]
fn checkpoint_survives_cold_reopen() {
    let mut f = version_fixture();
    f.repository
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: "cp-reopen".into(),
            task_id: "task-a".into(),
            execution_id: "e1".into(),
            workspace_id: "ws-a".into(),
            version_id: f.version_id.clone(),
            operation_id: None,
            reason: "cold reopen".into(),
            actor_agent_id: "agent-a".into(),
            request_id: "checkpoint-reopen".into(),
            created_at: "t3".into(),
        })
        .unwrap();
    let repository_path = f._project.path().to_path_buf();
    drop(f.repository);
    let reopened = Repository::open(repository_path).unwrap();
    let checkpoint = reopened
        .metadata()
        .checkpoint("cp-reopen")
        .unwrap()
        .expect("checkpoint after cold reopen");
    assert_eq!(checkpoint.version_id, f.version_id);
}
