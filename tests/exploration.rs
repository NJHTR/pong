use pong_core::metadata::{
    AgentIdentity, CandidateCreation, ExecutionCreation, ExplorationCreation, OperationEnvelope,
    OperationRef, TaskCreation, VersionPublication,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::{AgentControl, PongError, Repository};
use std::fs;
use tempfile::tempdir;

struct Fixture {
    project: tempfile::TempDir,
    workspace_root: tempfile::TempDir,
    repository: Repository,
    workspace_path: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let project = tempdir().unwrap();
    let workspace_root = tempdir().unwrap();
    let mut repository = Repository::init(project.path()).unwrap();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "agent-a".into(),
            provider: "test".into(),
            display_name: None,
            created_at: "t0".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task-a".into(),
            project_id: "project-a".into(),
            goal: "explore".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .record_environment(
            "env-a",
            "project-a",
            &serde_json::json!({"os":"windows"}),
            "t0",
        )
        .unwrap();
    let workspace_path = workspace_root.path().join("source");
    {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws-a",
                "project-a",
                &workspace_path,
                None,
                Some("env-a"),
                "t0",
            )
            .unwrap();
        manager
            .acquire_lease("ws-a", "agent-a", 0, 1_000_000)
            .unwrap();
    }
    Fixture {
        project,
        workspace_root,
        repository,
        workspace_path,
    }
}

fn root_version(f: &mut Fixture) -> String {
    fs::write(f.workspace_path.join("state.txt"), b"root").unwrap();
    let lease = f
        .repository
        .metadata()
        .workspace_lease("ws-a")
        .unwrap()
        .unwrap();
    let lease = pong_core::LeaseToken {
        workspace_id: lease.workspace_id,
        agent_id: lease.agent_id.unwrap(),
        epoch: lease.epoch,
        expires_at_ms: lease.expires_at_ms,
    };
    let snapshot = WorkspaceManager::new(&mut f.repository, Redactor::default())
        .snapshot_local("ws-a", &lease, SnapshotOptions::default(), 1, "t1")
        .unwrap();
    let operation = OperationEnvelope {
        operation_id: "op-root".into(),
        project_id: "project-a".into(),
        request_id: "req-root".into(),
        agent_id: "agent-a".into(),
        session_id: "s".into(),
        workspace_id: Some("ws-a".into()),
        environment_id: Some("env-a".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "t1".into(),
        tool: "test".into(),
        action: "version.create".into(),
        input_refs: vec![OperationRef {
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
    f.repository
        .metadata_mut()
        .start_operation(operation)
        .unwrap();
    f.repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: "ws-a".into(),
            project_id: "project-a".into(),
            snapshot_id: snapshot.snapshot_id,
            creation_operation_id: "op-root".into(),
            environment_id: Some("env-a".into()),
            created_at: "t1".into(),
            parent_version_id: None,
        })
        .unwrap()
        .version_id
}

#[test]
fn exploration_routes_select_durably_and_preserve_history() {
    let mut f = fixture();
    let root = root_version(&mut f);
    f.repository
        .metadata_mut()
        .create_exploration(&ExplorationCreation {
            exploration_id: "x1".into(),
            task_id: "task-a".into(),
            root_version_id: root.clone(),
            created_at: "t2".into(),
        })
        .unwrap();
    for id in ["e-a", "e-b"] {
        f.repository
            .metadata_mut()
            .create_execution(&ExecutionCreation {
                execution_id: id.into(),
                task_id: "task-a".into(),
                agent_id: "agent-a".into(),
                parent_execution_id: None,
                workspace_id: Some("ws-a".into()),
                base_version_id: Some(root.clone()),
                current_version_id: None,
                created_at: format!("created-{id}"),
            })
            .unwrap();
    }
    for id in ["c-a", "c-b"] {
        f.repository
            .metadata_mut()
            .create_candidate(&CandidateCreation {
                candidate_id: id.into(),
                exploration_id: "x1".into(),
                task_id: "task-a".into(),
                root_version_id: root.clone(),
                execution_id: if id == "c-a" { "e-a" } else { "e-b" }.into(),
                checkpoint_id: None,
                operation_id: None,
                created_at: format!("created-{id}"),
            })
            .unwrap();
    }
    f.repository
        .metadata_mut()
        .transition_candidate(
            "c-a",
            "completed",
            Some("SUCCESS"),
            Some(&root),
            Some(0.9),
            Some("evidence://a"),
            "t3",
        )
        .unwrap();
    f.repository
        .metadata_mut()
        .transition_candidate(
            "c-b",
            "failed",
            Some("FAILURE"),
            None,
            None,
            Some("evidence://b"),
            "t3",
        )
        .unwrap();
    let selected = f
        .repository
        .metadata_mut()
        .select_candidate("x1", "c-a", "t4")
        .unwrap();
    assert_eq!(selected.selected_candidate_id.as_deref(), Some("c-a"));
    assert_eq!(
        f.repository
            .metadata()
            .candidate("c-a")
            .unwrap()
            .unwrap()
            .status,
        "selected"
    );
    assert_eq!(
        f.repository
            .metadata()
            .candidate("c-b")
            .unwrap()
            .unwrap()
            .status,
        "failed"
    );
    assert_eq!(
        f.repository
            .metadata()
            .exploration("x1")
            .unwrap()
            .unwrap()
            .root_version_id,
        root
    );
    let lease = f
        .repository
        .metadata()
        .workspace_lease("ws-a")
        .unwrap()
        .unwrap();
    let lease = pong_core::LeaseToken {
        workspace_id: lease.workspace_id,
        agent_id: lease.agent_id.unwrap(),
        epoch: lease.epoch,
        expires_at_ms: lease.expires_at_ms,
    };
    let materialized = AgentControl::new(&mut f.repository)
        .materialize_selected_candidate("x1", "ws-a", &lease, 1, 10, "t5")
        .unwrap();
    assert_eq!(materialized.workspace_id, "ws-a");
    let mut reopened = Repository::open(f.project.path()).unwrap();
    assert_eq!(
        reopened
            .metadata()
            .exploration("x1")
            .unwrap()
            .unwrap()
            .selected_candidate_id
            .as_deref(),
        Some("c-a")
    );
    assert!(matches!(
        reopened
            .metadata_mut()
            .select_candidate("x1", "missing", "t5"),
        Err(PongError::NotFound(_))
    ));
}

#[test]
fn candidate_scope_rejects_cross_exploration_and_invalid_completion() {
    let mut f = fixture();
    let root = root_version(&mut f);
    for id in ["x1", "x2"] {
        f.repository
            .metadata_mut()
            .create_exploration(&ExplorationCreation {
                exploration_id: id.into(),
                task_id: "task-a".into(),
                root_version_id: root.clone(),
                created_at: id.into(),
            })
            .unwrap();
    }
    f.repository
        .metadata_mut()
        .create_execution(&ExecutionCreation {
            execution_id: "e1".into(),
            task_id: "task-a".into(),
            agent_id: "agent-a".into(),
            parent_execution_id: None,
            workspace_id: Some("ws-a".into()),
            base_version_id: Some(root.clone()),
            current_version_id: None,
            created_at: "t2".into(),
        })
        .unwrap();
    f.repository
        .metadata_mut()
        .create_candidate(&CandidateCreation {
            candidate_id: "c1".into(),
            exploration_id: "x1".into(),
            task_id: "task-a".into(),
            root_version_id: root.clone(),
            execution_id: "e1".into(),
            checkpoint_id: None,
            operation_id: None,
            created_at: "t3".into(),
        })
        .unwrap();
    assert!(matches!(
        f.repository
            .metadata_mut()
            .select_candidate("x2", "c1", "t4"),
        Err(PongError::Conflict(_))
    ));
    assert!(matches!(
        f.repository.metadata_mut().transition_candidate(
            "c1",
            "completed",
            Some("CANDIDATE"),
            None,
            None,
            None,
            "t4"
        ),
        Err(PongError::InvalidInput(_))
    ));
    let _ = &f.workspace_root;
}
