use pong_core::metadata::{CheckpointCreation, RouteExecutionAttachment};
use pong_core::{
    AgentControl, CreateExecutionRequest, CreateExplorationRequest, CreateRouteRequest,
    CreateTaskRequest, CreateWorkspaceRequest, PublishVersionRequest, RegisterAgentRequest,
    Repository,
};
use serde_json::json;
use std::fs;
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "project-exploration";
const TASK: &str = "task-exploration";
const AGENT: &str = "agent-exploration";
const WORKSPACE: &str = "workspace-exploration";
const ENVIRONMENT: &str = "environment-exploration";

struct Fixture {
    repository_dir: TempDir,
    _workspace_dir: TempDir,
    repository: Repository,
    version_id: String,
    checkpoint_id: String,
}

fn fixture() -> Fixture {
    let repository_dir = tempdir().expect("repository directory");
    let workspace_dir = tempdir().expect("workspace directory");
    let mut repository = Repository::init(repository_dir.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            ENVIRONMENT,
            PROJECT,
            &json!({"schema_version": 1, "test": "exploration"}),
            "t0",
        )
        .expect("environment");
    let mut control = AgentControl::new(&mut repository);
    control
        .register_agent(RegisterAgentRequest {
            agent_id: AGENT.into(),
            provider: "test".into(),
            display_name: Some("E1".into()),
            created_at: "t0".into(),
        })
        .expect("agent");
    control
        .create_task(CreateTaskRequest {
            task_id: TASK.into(),
            project_id: PROJECT.into(),
            goal: "route relation test".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .expect("task");
    control
        .create_workspace(CreateWorkspaceRequest {
            workspace_id: WORKSPACE.into(),
            project_id: PROJECT.into(),
            path: workspace_dir.path().join("tree"),
            branch_ref: None,
            environment_id: Some(ENVIRONMENT.into()),
            now: "t0".into(),
        })
        .expect("workspace");
    let lease = control
        .acquire_workspace(pong_core::AcquireWorkspaceRequest {
            workspace_id: WORKSPACE.into(),
            agent_id: AGENT.into(),
            now_ms: 1,
            ttl_ms: 10_000,
        })
        .expect("lease");
    fs::write(workspace_dir.path().join("tree/state.txt"), "initial").expect("state");
    let publication = control
        .publish_version(PublishVersionRequest {
            workspace_id: WORKSPACE.into(),
            lease: lease.clone(),
            expected_workspace_revision: 0,
            now_ms: 2,
            created_at: "t1".into(),
            operation_id: "operation:e1:version".into(),
            request_id: "request:e1:version".into(),
            session_id: Some("session:e1".into()),
            tool: Some("test".into()),
            parent_version_id: None,
            update_version_head: true,
        })
        .expect("version");
    control
        .create_execution(CreateExecutionRequest {
            execution_id: "execution:e1".into(),
            task_id: TASK.into(),
            agent_id: AGENT.into(),
            parent_execution_id: None,
            workspace_id: Some(WORKSPACE.into()),
            base_version_id: Some(publication.version.version_id.clone()),
            current_version_id: Some(publication.version.version_id.clone()),
            created_at: "t1".into(),
        })
        .expect("execution");
    let checkpoint = control
        .create_checkpoint(CheckpointCreation {
            checkpoint_id: "checkpoint:e1".into(),
            task_id: TASK.into(),
            execution_id: "execution:e1".into(),
            workspace_id: WORKSPACE.into(),
            version_id: publication.version.version_id.clone(),
            operation_id: None,
            reason: "route source".into(),
            actor_agent_id: AGENT.into(),
            request_id: "request:e1:checkpoint".into(),
            created_at: "t2".into(),
        })
        .expect("checkpoint");
    Fixture {
        repository_dir,
        _workspace_dir: workspace_dir,
        repository,
        version_id: publication.version.version_id,
        checkpoint_id: checkpoint.checkpoint_id,
    }
}

fn exploration(fixture: &mut Fixture) {
    AgentControl::new(&mut fixture.repository)
        .create_exploration(CreateExplorationRequest {
            exploration_id: "exploration:e1".into(),
            task_id: TASK.into(),
            created_by: AGENT.into(),
            purpose_ref: Some("purpose:e1".into()),
            created_at: "t3".into(),
        })
        .expect("exploration");
}

#[test]
fn exploration_routes_are_explicit_durable_and_reopenable() {
    let mut fixture = fixture();
    exploration(&mut fixture);
    let mut control = AgentControl::new(&mut fixture.repository);
    let route_a = control
        .create_route(CreateRouteRequest {
            route_id: "route:a".into(),
            exploration_id: "exploration:e1".into(),
            source_kind: "version".into(),
            source_id: fixture.version_id.clone(),
            status: "active".into(),
            terminal_version_id: None,
            created_by: AGENT.into(),
            purpose_ref: Some("route-a".into()),
            created_at: "t4".into(),
        })
        .expect("route A");
    let route_b = control
        .create_route(CreateRouteRequest {
            route_id: "route:b".into(),
            exploration_id: "exploration:e1".into(),
            source_kind: "checkpoint".into(),
            source_id: fixture.checkpoint_id.clone(),
            status: "active".into(),
            terminal_version_id: None,
            created_by: AGENT.into(),
            purpose_ref: Some("route-b".into()),
            created_at: "t5".into(),
        })
        .expect("route B");
    assert_eq!(route_a.source_id, fixture.version_id);
    assert_eq!(route_b.source_id, fixture.checkpoint_id);
    assert_eq!(
        control
            .routes_for_exploration("exploration:e1")
            .unwrap()
            .len(),
        2
    );

    let execution = control
        .attach_execution_to_route(RouteExecutionAttachment {
            route_id: route_a.route_id.clone(),
            execution_id: "execution:e1".into(),
            attached_at: "t6".into(),
        })
        .expect("route membership");
    assert_eq!(execution.execution_id, "execution:e1");
    assert_eq!(
        control.route_executions("route:a").unwrap(),
        vec![execution.clone()]
    );
    assert_eq!(
        control
            .attach_execution_to_route(execution.clone())
            .expect("idempotent membership"),
        execution
    );
    assert_eq!(control.route("route:a").unwrap().unwrap().status, "active");

    drop(control);
    drop(fixture.repository);
    let mut reopened = Repository::open(fixture.repository_dir.path()).expect("cold reopen");
    let control = AgentControl::new(&mut reopened);
    let exploration = control.exploration("exploration:e1").unwrap().unwrap();
    assert_eq!(exploration.task_id, TASK);
    assert_eq!(
        control
            .routes_for_exploration(&exploration.exploration_id)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn route_scope_and_idempotency_are_rejected_without_mutation() {
    let mut fixture = fixture();
    exploration(&mut fixture);
    let mut control = AgentControl::new(&mut fixture.repository);
    let request = CreateRouteRequest {
        route_id: "route:idempotent".into(),
        exploration_id: "exploration:e1".into(),
        source_kind: "version".into(),
        source_id: fixture.version_id.clone(),
        status: "active".into(),
        terminal_version_id: None,
        created_by: AGENT.into(),
        purpose_ref: None,
        created_at: "t4".into(),
    };
    let first = control.create_route(request.clone()).expect("route");
    assert_eq!(control.create_route(request.clone()).unwrap(), first);
    let mut changed = request.clone();
    changed.status = "paused".into();
    assert_eq!(
        control.create_route(changed).unwrap_err().code(),
        "IDEMPOTENCY_KEY_REUSE"
    );
    assert_eq!(
        control
            .routes_for_exploration("exploration:e1")
            .unwrap()
            .len(),
        1
    );

    let error = control
        .create_route(CreateRouteRequest {
            route_id: "route:missing".into(),
            exploration_id: "exploration:e1".into(),
            source_kind: "version".into(),
            source_id: "version:other-project".into(),
            status: "active".into(),
            terminal_version_id: None,
            created_by: AGENT.into(),
            purpose_ref: None,
            created_at: "t5".into(),
        })
        .unwrap_err();
    assert_eq!(error.code(), "NOT_FOUND");
}
