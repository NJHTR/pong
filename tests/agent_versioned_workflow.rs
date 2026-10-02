//! Focused Agent-native Version workflow coverage.
//!
//! This test uses the public control facade for Version publication and the
//! existing execution/checkpoint APIs. It deliberately models a historical
//! branch without introducing Git Branch, Commit, or Change entities.

use pong_core::metadata::{CheckpointCreation, ResumeCreation, RollbackCreation, RollbackTarget};
use pong_core::redaction::Redactor;
use pong_core::{
    AcquireWorkspaceRequest, AgentControl, CreateExecutionRequest, CreateTaskRequest,
    CreateWorkspaceRequest, PublishVersionRequest, RegisterAgentRequest, Repository,
    WorkspaceManager,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

const PROJECT: &str = "project-agent-workflow";
const ENVIRONMENT: &str = "environment-agent-workflow";
const TASK: &str = "task-agent-workflow";
const WORKSPACE: &str = "workspace-agent-workflow";
const AGENT: &str = "agent-agent-workflow";
const EXECUTION: &str = "execution-agent-workflow";
const RESUMED_EXECUTION: &str = "execution-agent-workflow-resumed";

struct Fixture {
    project_dir: TempDir,
    _workspace_parent: TempDir,
    repository: Repository,
    workspace_path: PathBuf,
    lease: pong_core::LeaseToken,
}

fn fixture() -> Fixture {
    let project_dir = tempfile::tempdir().expect("project directory");
    let workspace_parent = tempfile::tempdir().expect("workspace parent");
    let mut repository = Repository::init(project_dir.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            ENVIRONMENT,
            PROJECT,
            &json!({"schema_version": 1, "os": "windows"}),
            "t0",
        )
        .expect("environment");

    let mut control = AgentControl::new(&mut repository);
    control
        .register_agent(RegisterAgentRequest {
            agent_id: AGENT.into(),
            provider: "test-double".into(),
            display_name: Some("Workflow Agent".into()),
            created_at: "t0".into(),
        })
        .expect("agent");
    control
        .create_task(CreateTaskRequest {
            task_id: TASK.into(),
            project_id: PROJECT.into(),
            goal: "versioned agent workflow".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .expect("task");

    let workspace_path = workspace_parent.path().join("workspace");
    control
        .create_workspace(CreateWorkspaceRequest {
            workspace_id: WORKSPACE.into(),
            project_id: PROJECT.into(),
            path: workspace_path.clone(),
            branch_ref: None,
            environment_id: Some(ENVIRONMENT.into()),
            now: "t1".into(),
        })
        .expect("workspace");
    fs::create_dir_all(&workspace_path).expect("materialize workspace directory");
    control
        .create_execution(CreateExecutionRequest {
            execution_id: EXECUTION.into(),
            task_id: TASK.into(),
            agent_id: AGENT.into(),
            parent_execution_id: None,
            workspace_id: Some(WORKSPACE.into()),
            base_version_id: None,
            current_version_id: None,
            created_at: "t1".into(),
        })
        .expect("execution");
    let lease = control
        .acquire_workspace(AcquireWorkspaceRequest {
            workspace_id: WORKSPACE.into(),
            agent_id: AGENT.into(),
            now_ms: 0,
            ttl_ms: 1_000_000,
        })
        .expect("lease");

    Fixture {
        project_dir,
        _workspace_parent: workspace_parent,
        repository,
        workspace_path,
        lease,
    }
}

fn publish(
    fixture: &mut Fixture,
    expected_workspace_revision: i64,
    operation_id: &str,
    request_id: &str,
    parent_version_id: Option<&str>,
    now_ms: i64,
    created_at: &str,
) -> pong_core::PublishVersionResult {
    AgentControl::new(&mut fixture.repository)
        .publish_version(PublishVersionRequest {
            workspace_id: WORKSPACE.into(),
            lease: fixture.lease.clone(),
            expected_workspace_revision,
            now_ms,
            created_at: created_at.into(),
            operation_id: operation_id.into(),
            request_id: request_id.into(),
            session_id: Some("session-agent-workflow".into()),
            tool: Some("agent-workflow-test".into()),
            parent_version_id: parent_version_id.map(str::to_owned),
            update_version_head: true,
        })
        .expect("publish Version")
}

fn attach(fixture: &mut Fixture, execution_id: &str, operation_id: &str, created_at: &str) {
    AgentControl::new(&mut fixture.repository)
        .attach_operation_to_execution(execution_id, operation_id, created_at)
        .expect("attach publication operation to execution");
}

fn workspace_revision(fixture: &Fixture) -> i64 {
    fixture
        .repository
        .metadata()
        .workspace(WORKSPACE)
        .expect("workspace")
        .expect("workspace record")
        .revision
}

#[test]
fn agent_versioned_workflow_preserves_linear_history_and_historical_branch() {
    let mut fixture = fixture();
    let started = AgentControl::new(&mut fixture.repository)
        .start_execution(EXECUTION, 0, "t2")
        .expect("start execution");
    assert_eq!(started.state, "running");

    fs::write(fixture.workspace_path.join("state.txt"), b"zero").expect("V0 content");
    let v0 = publish(&mut fixture, 0, "op-v0", "request-v0", None, 1, "t3");
    attach(&mut fixture, EXECUTION, "op-v0", "t3-owner");
    let revision_after_v0 = workspace_revision(&fixture);
    let current = AgentControl::new(&mut fixture.repository)
        .set_execution_current_version(
            EXECUTION,
            &v0.version.version_id,
            &fixture.lease,
            1,
            revision_after_v0,
            "t3-current",
            2,
        )
        .expect("select V0 for execution");
    assert_eq!(
        current.current_version_id.as_deref(),
        Some(v0.version.version_id.as_str())
    );

    fs::write(fixture.workspace_path.join("state.txt"), b"one").expect("V1 content");
    let revision_before_diff = workspace_revision(&fixture);
    let diff_v1 = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace_against_version(WORKSPACE, &v0.version.version_id)
        .expect("observe V1 diff");
    assert_eq!(diff_v1.entries.len(), 1);
    assert_eq!(diff_v1.entries[0].path, "state.txt");
    assert_eq!(
        workspace_revision(&fixture),
        revision_before_diff,
        "diff is read-only"
    );
    let revision_before_v1 = workspace_revision(&fixture);
    let v1 = publish(
        &mut fixture,
        revision_before_v1,
        "op-v1",
        "request-v1",
        Some(&v0.version.version_id),
        3,
        "t4",
    );
    attach(&mut fixture, EXECUTION, "op-v1", "t4-owner");
    let revision_after_v1 = workspace_revision(&fixture);
    let current = AgentControl::new(&mut fixture.repository)
        .set_execution_current_version(
            EXECUTION,
            &v1.version.version_id,
            &fixture.lease,
            2,
            revision_after_v1,
            "t4-current",
            4,
        )
        .expect("select V1 for execution");
    assert_eq!(
        current.current_version_id.as_deref(),
        Some(v1.version.version_id.as_str())
    );

    fixture
        .repository
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: "checkpoint-v1".into(),
            task_id: TASK.into(),
            execution_id: EXECUTION.into(),
            workspace_id: WORKSPACE.into(),
            version_id: v1.version.version_id.clone(),
            operation_id: Some("op-v1".into()),
            reason: "stable agent milestone".into(),
            actor_agent_id: AGENT.into(),
            request_id: "request-checkpoint-v1".into(),
            created_at: "t5".into(),
        })
        .expect("checkpoint V1");

    fs::write(fixture.workspace_path.join("state.txt"), b"two").expect("V2 content");
    let revision_before_v2 = workspace_revision(&fixture);
    let v2 = publish(
        &mut fixture,
        revision_before_v2,
        "op-v2",
        "request-v2",
        Some(&v1.version.version_id),
        5,
        "t6",
    );
    attach(&mut fixture, EXECUTION, "op-v2", "t6-owner");
    let revision_after_v2 = workspace_revision(&fixture);
    let current = AgentControl::new(&mut fixture.repository)
        .set_execution_current_version(
            EXECUTION,
            &v2.version.version_id,
            &fixture.lease,
            3,
            revision_after_v2,
            "t6-current",
            6,
        )
        .expect("select V2 for execution");
    assert_eq!(
        current.current_version_id.as_deref(),
        Some(v2.version.version_id.as_str())
    );
    let revision_before_rollback = workspace_revision(&fixture);
    let failed = AgentControl::new(&mut fixture.repository)
        .finish_execution(EXECUTION, "failed", Some("agent interrupted"), 4, "t7")
        .expect("fail execution");
    assert_eq!(failed.state, "failed");

    let rollback = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .rollback_local(&RollbackCreation {
            rollback_id: "rollback-to-v1".into(),
            task_id: TASK.into(),
            execution_id: EXECUTION.into(),
            workspace_id: WORKSPACE.into(),
            target: RollbackTarget::Version(v1.version.version_id.clone()),
            actor_agent_id: AGENT.into(),
            request_id: "request-rollback-to-v1".into(),
            created_at: "t8".into(),
            lease: Some(fixture.lease.clone()),
            expected_workspace_revision: Some(revision_before_rollback),
            now_ms: Some(7),
        })
        .expect("rollback workspace to V1");
    assert_eq!(
        rollback.result_version_head.as_deref(),
        Some(v1.version.version_id.as_str())
    );
    assert_eq!(
        fs::read(fixture.workspace_path.join("state.txt")).unwrap(),
        b"one"
    );

    fixture
        .repository
        .metadata_mut()
        .resume_from_checkpoint(&ResumeCreation {
            execution_id: RESUMED_EXECUTION.into(),
            task_id: TASK.into(),
            agent_id: AGENT.into(),
            parent_execution_id: Some(EXECUTION.into()),
            workspace_id: Some(WORKSPACE.into()),
            source_version_id: None,
            checkpoint_id: Some("checkpoint-v1".into()),
            request_id: "request-resume-v1".into(),
            created_at: "t9".into(),
        })
        .expect("resume from V1 checkpoint");
    let resumed = fixture
        .repository
        .metadata()
        .execution(RESUMED_EXECUTION)
        .expect("resumed execution lookup")
        .expect("resumed execution");
    assert_eq!(resumed.state, "created");

    fs::write(fixture.workspace_path.join("state.txt"), b"three").expect("V3 branch content");
    let revision_before_v3 = workspace_revision(&fixture);
    let v3 = publish(
        &mut fixture,
        revision_before_v3,
        "op-v3",
        "request-v3",
        Some(&v1.version.version_id),
        11,
        "t11",
    );
    attach(&mut fixture, RESUMED_EXECUTION, "op-v3", "t11-owner");
    let revision_after_v3 = workspace_revision(&fixture);
    let resumed = AgentControl::new(&mut fixture.repository)
        .set_execution_current_version(
            RESUMED_EXECUTION,
            &v3.version.version_id,
            &fixture.lease,
            0,
            revision_after_v3,
            "t11-current",
            12,
        )
        .expect("select V3 for resumed execution");
    assert_eq!(
        resumed.current_version_id.as_deref(),
        Some(v3.version.version_id.as_str())
    );

    let children = fixture
        .repository
        .metadata()
        .get_children(&v1.version.version_id)
        .expect("V1 children");
    assert_eq!(children.len(), 2);
    assert!(children
        .iter()
        .any(|child| child.version_id == v2.version.version_id));
    assert!(children
        .iter()
        .any(|child| child.version_id == v3.version.version_id));
    assert_eq!(
        fixture
            .repository
            .metadata()
            .execution_for_operation("op-v0")
            .unwrap()
            .unwrap()
            .execution_id,
        EXECUTION
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .execution_for_operation("op-v1")
            .unwrap()
            .unwrap()
            .execution_id,
        EXECUTION
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .execution_for_operation("op-v2")
            .unwrap()
            .unwrap()
            .execution_id,
        EXECUTION
    );
    assert_eq!(
        fixture
            .repository
            .metadata()
            .execution_for_operation("op-v3")
            .unwrap()
            .unwrap()
            .execution_id,
        RESUMED_EXECUTION
    );

    let project_path = fixture.project_dir.path().to_path_buf();
    let workspace_path = fixture.workspace_path.clone();
    drop(fixture.repository);
    let reopened = Repository::open(project_path).expect("cold reopen");
    let versions = reopened.metadata().list_versions(WORKSPACE).unwrap();
    assert_eq!(versions.len(), 4);
    let reopened_v0 = versions
        .iter()
        .find(|version| version.version_id == v0.version.version_id)
        .expect("V0 after reopen");
    let reopened_v1 = versions
        .iter()
        .find(|version| version.version_id == v1.version.version_id)
        .expect("V1 after reopen");
    let reopened_v2 = versions
        .iter()
        .find(|version| version.version_id == v2.version.version_id)
        .expect("V2 after reopen");
    let reopened_v3 = versions
        .iter()
        .find(|version| version.version_id == v3.version.version_id)
        .expect("V3 after reopen");
    assert_eq!(reopened_v0.parent_version_id, None);
    assert_eq!(
        reopened_v1.parent_version_id.as_deref(),
        Some(v0.version.version_id.as_str())
    );
    assert_eq!(
        reopened_v2.parent_version_id.as_deref(),
        Some(v1.version.version_id.as_str())
    );
    assert_eq!(
        reopened_v3.parent_version_id.as_deref(),
        Some(v1.version.version_id.as_str())
    );
    let workspace = reopened.metadata().workspace(WORKSPACE).unwrap().unwrap();
    assert_eq!(
        workspace.version_head_id.as_deref(),
        Some(v3.version.version_id.as_str())
    );
    assert_eq!(
        fs::read(workspace_path.join("state.txt")).unwrap(),
        b"three"
    );
    let old_execution = reopened.metadata().execution(EXECUTION).unwrap().unwrap();
    assert_eq!(old_execution.state, "failed");
    assert_eq!(
        old_execution.current_version_id.as_deref(),
        Some(v2.version.version_id.as_str())
    );
    let resumed_execution = reopened
        .metadata()
        .execution(RESUMED_EXECUTION)
        .unwrap()
        .unwrap();
    assert_eq!(
        resumed_execution.parent_execution_id.as_deref(),
        Some(EXECUTION)
    );
    assert_eq!(
        resumed_execution.base_version_id.as_deref(),
        Some(v1.version.version_id.as_str())
    );
    assert_eq!(
        resumed_execution.current_version_id.as_deref(),
        Some(v3.version.version_id.as_str())
    );
    assert_eq!(
        reopened
            .metadata()
            .resume_record(RESUMED_EXECUTION)
            .unwrap()
            .unwrap()
            .source_version_id,
        v1.version.version_id
    );
    assert_eq!(
        reopened
            .metadata()
            .execution_for_operation("op-v3")
            .unwrap()
            .unwrap()
            .execution_id,
        RESUMED_EXECUTION
    );
}
