//! Offline-first proof: the complete durable workflow runs without transports
//! or external services.

use pong_core::metadata::{
    AgentIdentity, CheckpointCreation, ExecutionCreation, HandoffCreation, OperationEnvelope,
    OperationRef, ResumeCreation, RollbackCreation, RollbackTarget, TaskCreation,
    VersionPublication,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::Repository;
use serde_json::json;
use std::fs;
use tempfile::{tempdir, TempDir};

fn version_operation(id: &str, snapshot_id: &str, request_id: &str) -> OperationEnvelope {
    OperationEnvelope {
        operation_id: id.into(),
        project_id: "offline-project".into(),
        request_id: request_id.into(),
        agent_id: "agent-a".into(),
        session_id: "offline-session".into(),
        workspace_id: Some("offline-workspace".into()),
        environment_id: Some("offline-environment".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: id.into(),
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

struct Fixture {
    _repo_dir: TempDir,
    _workspace_parent: TempDir,
    repo: Repository,
    workspace_path: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let repo_dir = tempdir().unwrap();
    let workspace_parent = tempdir().unwrap();
    let mut repo = Repository::init(repo_dir.path()).unwrap();
    repo.metadata_mut()
        .record_environment(
            "offline-environment",
            "offline-project",
            &json!({"schema_version": 1, "offline": true}),
            "t0",
        )
        .unwrap();
    for agent_id in ["agent-a", "agent-b"] {
        repo.metadata_mut()
            .create_agent_identity(&AgentIdentity {
                agent_id: agent_id.into(),
                provider: "runtime-neutral".into(),
                display_name: None,
                created_at: "t0".into(),
            })
            .unwrap();
    }
    repo.metadata_mut()
        .create_task(&TaskCreation {
            task_id: "offline-task".into(),
            project_id: "offline-project".into(),
            goal: "offline durable workflow".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .unwrap();
    let workspace_path = workspace_parent.path().join("workspace");
    WorkspaceManager::new(&mut repo, Redactor::default())
        .create_local(
            "offline-workspace",
            "offline-project",
            &workspace_path,
            None,
            Some("offline-environment"),
            "t0",
        )
        .unwrap();
    repo.metadata_mut()
        .create_execution(&ExecutionCreation {
            execution_id: "offline-e1".into(),
            task_id: "offline-task".into(),
            agent_id: "agent-a".into(),
            parent_execution_id: None,
            workspace_id: Some("offline-workspace".into()),
            base_version_id: None,
            current_version_id: None,
            created_at: "t0".into(),
        })
        .unwrap();
    Fixture {
        _repo_dir: repo_dir,
        _workspace_parent: workspace_parent,
        repo,
        workspace_path,
    }
}

#[test]
fn offline_core_workflow_survives_reopen_without_network() {
    let mut f = fixture();
    f.repo
        .metadata_mut()
        .start_execution("offline-e1", 0, "t1")
        .unwrap();
    let lease_a = WorkspaceManager::new(&mut f.repo, Redactor::default())
        .acquire_lease("offline-workspace", "agent-a", 1, 100_000)
        .unwrap();

    fs::write(f.workspace_path.join("state.txt"), b"first").unwrap();
    let snapshot = WorkspaceManager::new(&mut f.repo, Redactor::default())
        .snapshot_local(
            "offline-workspace",
            &lease_a,
            SnapshotOptions::default(),
            0,
            "t2",
        )
        .unwrap();
    f.repo
        .metadata_mut()
        .start_operation(version_operation(
            "offline-op-v1",
            &snapshot.snapshot_id,
            "offline-v1",
        ))
        .unwrap();
    let version = f
        .repo
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "offline-workspace".into(),
            project_id: "offline-project".into(),
            snapshot_id: snapshot.snapshot_id.clone(),
            creation_operation_id: "offline-op-v1".into(),
            environment_id: Some("offline-environment".into()),
            created_at: "t3".into(),
            parent_version_id: None,
        })
        .unwrap();
    let version_id = version.version_id.clone();

    f.repo
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: "offline-c1".into(),
            task_id: "offline-task".into(),
            execution_id: "offline-e1".into(),
            workspace_id: "offline-workspace".into(),
            version_id: version_id.clone(),
            operation_id: Some("offline-op-v1".into()),
            reason: "offline checkpoint".into(),
            actor_agent_id: "agent-a".into(),
            request_id: "offline-c1-request".into(),
            created_at: "t4".into(),
        })
        .unwrap();
    f.repo
        .metadata_mut()
        .transition_execution("offline-e1", "interrupted", Some("offline"), 1, "t5")
        .unwrap();

    f.repo
        .metadata_mut()
        .create_execution(&ExecutionCreation {
            execution_id: "offline-e2".into(),
            task_id: "offline-task".into(),
            agent_id: "agent-b".into(),
            parent_execution_id: Some("offline-e1".into()),
            workspace_id: Some("offline-workspace".into()),
            base_version_id: Some(version_id.clone()),
            current_version_id: None,
            created_at: "t6".into(),
        })
        .unwrap();
    f.repo
        .metadata_mut()
        .create_handoff(&HandoffCreation {
            handoff_id: "offline-h1".into(),
            task_id: "offline-task".into(),
            from_execution_id: "offline-e1".into(),
            to_execution_id: "offline-e2".into(),
            source_version_id: Some(version_id.clone()),
            checkpoint_id: Some("offline-c1".into()),
            reason: "continue offline".into(),
            actor_agent_id: "agent-a".into(),
            requester_execution_id: None,
            request_id: "offline-h1-request".into(),
            created_at: "t6".into(),
        })
        .unwrap();
    f.repo
        .metadata_mut()
        .resume_from_checkpoint(&ResumeCreation {
            execution_id: "offline-e3".into(),
            task_id: "offline-task".into(),
            agent_id: "agent-b".into(),
            parent_execution_id: Some("offline-e2".into()),
            workspace_id: Some("offline-workspace".into()),
            source_version_id: None,
            checkpoint_id: Some("offline-c1".into()),
            request_id: "offline-resume".into(),
            created_at: "t7".into(),
        })
        .unwrap();

    let diff = WorkspaceManager::new(&mut f.repo, Redactor::default())
        .diff_workspace_against_version("offline-workspace", &version_id)
        .unwrap();
    assert!(diff.entries.is_empty());
    WorkspaceManager::new(&mut f.repo, Redactor::default())
        .restore_from_version("offline-workspace", &version_id, &lease_a, 1, 1, "t8")
        .unwrap();

    WorkspaceManager::new(&mut f.repo, Redactor::default())
        .release_lease(&lease_a, 2)
        .unwrap();

    let lease_b = WorkspaceManager::new(&mut f.repo, Redactor::default())
        .acquire_lease("offline-workspace", "agent-b", 100, 100_000)
        .unwrap();
    fs::write(f.workspace_path.join("state.txt"), b"changed").unwrap();
    let rollback_revision = f
        .repo
        .metadata()
        .workspace("offline-workspace")
        .unwrap()
        .unwrap()
        .revision;
    let rolled = WorkspaceManager::new(&mut f.repo, Redactor::default())
        .rollback_local(&RollbackCreation {
            rollback_id: "offline-r1".into(),
            task_id: "offline-task".into(),
            execution_id: "offline-e3".into(),
            workspace_id: "offline-workspace".into(),
            target: RollbackTarget::Version(version_id.clone()),
            actor_agent_id: "agent-b".into(),
            request_id: "offline-r1-request".into(),
            created_at: "t9".into(),
            lease: Some(lease_b),
            expected_workspace_revision: Some(rollback_revision),
            now_ms: Some(100),
        })
        .unwrap();
    assert_eq!(rolled.status, "completed");
    assert_eq!(rolled.result_version_id, None);
    assert_eq!(
        fs::read(f.workspace_path.join("state.txt")).unwrap(),
        b"first"
    );

    let repo_dir = f._repo_dir.path().to_path_buf();
    drop(f.repo);
    let reopened = Repository::open(repo_dir).unwrap();
    assert!(reopened
        .metadata()
        .version_record(&version_id)
        .unwrap()
        .is_some());
    assert!(reopened
        .metadata()
        .checkpoint("offline-c1")
        .unwrap()
        .is_some());
    assert!(reopened.metadata().handoff("offline-h1").unwrap().is_some());
    assert!(reopened
        .metadata()
        .resume_record("offline-e3")
        .unwrap()
        .is_some());
    assert!(reopened
        .metadata()
        .rollback_record("offline-r1")
        .unwrap()
        .is_some());
}
