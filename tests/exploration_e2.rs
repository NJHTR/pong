use pong_core::metadata::{
    CheckpointCreation, HandoffCreation, ResumeCreation, RollbackCreation, RollbackTarget,
    RouteExecutionAttachment,
};
use pong_core::{
    AgentControl, CreateExecutionRequest, CreateExplorationRequest, CreateRouteRequest,
    CreateTaskRequest, CreateWorkspaceRequest, PublishVersionRequest, RegisterAgentRequest,
    Repository,
};
use serde_json::json;
use std::fs;
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "project-e2";
const TASK: &str = "task-e2";
const AGENT_A: &str = "agent-e2-a";
const AGENT_B: &str = "agent-e2-b";
const ENVIRONMENT: &str = "environment-e2";
const WORKSPACE_A: &str = "workspace-e2-a";
const WORKSPACE_B: &str = "workspace-e2-b";

struct Fixture {
    repository_dir: TempDir,
    _workspace_parent: TempDir,
    repository: Repository,
    workspace_a_path: std::path::PathBuf,
    workspace_b_path: std::path::PathBuf,
    v1: String,
    c1: String,
    lease_a: pong_core::LeaseToken,
    lease_b: pong_core::LeaseToken,
}

fn fixture() -> Fixture {
    let repository_dir = tempdir().expect("repository directory");
    let workspace_parent = tempdir().expect("workspace parent");
    let workspace_a_path = workspace_parent.path().join("workspace-a");
    let workspace_b_path = workspace_parent.path().join("workspace-b");
    let mut repository = Repository::init(repository_dir.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            ENVIRONMENT,
            PROJECT,
            &json!({"schema_version": 1, "test": "e2"}),
            "t0",
        )
        .expect("environment");
    let mut control = AgentControl::new(&mut repository);
    for (agent_id, display_name) in [(AGENT_A, "E2-A"), (AGENT_B, "E2-B")] {
        control
            .register_agent(RegisterAgentRequest {
                agent_id: agent_id.into(),
                provider: "test".into(),
                display_name: Some(display_name.into()),
                created_at: "t0".into(),
            })
            .expect("agent");
    }
    control
        .create_task(CreateTaskRequest {
            task_id: TASK.into(),
            project_id: PROJECT.into(),
            goal: "E2 route fork lifecycle".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .expect("task");
    for (workspace_id, path) in [
        (WORKSPACE_A, workspace_a_path.clone()),
        (WORKSPACE_B, workspace_b_path.clone()),
    ] {
        control
            .create_workspace(CreateWorkspaceRequest {
                workspace_id: workspace_id.into(),
                project_id: PROJECT.into(),
                path,
                branch_ref: None,
                environment_id: Some(ENVIRONMENT.into()),
                now: "t0".into(),
            })
            .expect("workspace");
    }
    let lease_a = control
        .acquire_workspace(pong_core::AcquireWorkspaceRequest {
            workspace_id: WORKSPACE_A.into(),
            agent_id: AGENT_A.into(),
            now_ms: 1,
            ttl_ms: 100_000,
        })
        .expect("workspace A lease");
    let lease_b = control
        .acquire_workspace(pong_core::AcquireWorkspaceRequest {
            workspace_id: WORKSPACE_B.into(),
            agent_id: AGENT_B.into(),
            now_ms: 1,
            ttl_ms: 100_000,
        })
        .expect("workspace B lease");

    fs::write(workspace_a_path.join("state.txt"), b"one").expect("initial state");
    let v1 = publish(&mut control, WORKSPACE_A, &lease_a, None, "op-e2-v1", "t1");
    control
        .create_execution(CreateExecutionRequest {
            execution_id: "execution:e2:a1".into(),
            task_id: TASK.into(),
            agent_id: AGENT_A.into(),
            parent_execution_id: None,
            workspace_id: Some(WORKSPACE_A.into()),
            base_version_id: Some(v1.version.version_id.clone()),
            current_version_id: Some(v1.version.version_id.clone()),
            created_at: "t1".into(),
        })
        .expect("route A execution");
    let c1 = control
        .create_checkpoint(CheckpointCreation {
            checkpoint_id: "checkpoint:e2:c1".into(),
            task_id: TASK.into(),
            execution_id: "execution:e2:a1".into(),
            workspace_id: WORKSPACE_A.into(),
            version_id: v1.version.version_id.clone(),
            operation_id: None,
            reason: "known-good fork source".into(),
            actor_agent_id: AGENT_A.into(),
            request_id: "request:e2:c1".into(),
            created_at: "t2".into(),
        })
        .expect("checkpoint");
    Fixture {
        repository_dir,
        _workspace_parent: workspace_parent,
        repository,
        workspace_a_path,
        workspace_b_path,
        v1: v1.version.version_id,
        c1: c1.checkpoint_id,
        lease_a,
        lease_b,
    }
}

fn publish(
    control: &mut AgentControl<'_>,
    workspace_id: &str,
    lease: &pong_core::LeaseToken,
    parent_version_id: Option<&str>,
    operation_id: &str,
    created_at: &str,
) -> pong_core::PublishVersionResult {
    let revision = control
        .workspace(workspace_id)
        .expect("workspace query")
        .expect("workspace")
        .revision;
    control
        .publish_version(PublishVersionRequest {
            workspace_id: workspace_id.into(),
            lease: lease.clone(),
            expected_workspace_revision: revision,
            now_ms: revision + 2,
            created_at: created_at.into(),
            operation_id: operation_id.into(),
            request_id: format!("request:{operation_id}"),
            session_id: Some(format!("session:{workspace_id}")),
            tool: Some("e2-test".into()),
            parent_version_id: parent_version_id.map(str::to_owned),
            update_version_head: true,
        })
        .expect("publish version")
}

fn setup_exploration(control: &mut AgentControl<'_>, v1: &str, c1: &str) {
    control
        .create_exploration(CreateExplorationRequest {
            exploration_id: "exploration:e2".into(),
            task_id: TASK.into(),
            created_by: AGENT_A.into(),
            purpose_ref: Some("purpose:e2".into()),
            created_at: "t3".into(),
        })
        .expect("exploration");
    control
        .create_route(CreateRouteRequest {
            route_id: "route:e2:a".into(),
            exploration_id: "exploration:e2".into(),
            source_kind: "version".into(),
            source_id: v1.into(),
            status: "active".into(),
            terminal_version_id: None,
            created_by: AGENT_A.into(),
            purpose_ref: Some("route A".into()),
            created_at: "t4".into(),
        })
        .expect("route A");
    control
        .create_route(CreateRouteRequest {
            route_id: "route:e2:b".into(),
            exploration_id: "exploration:e2".into(),
            source_kind: "checkpoint".into(),
            source_id: c1.into(),
            status: "active".into(),
            terminal_version_id: None,
            created_by: AGENT_A.into(),
            purpose_ref: Some("route B from checkpoint".into()),
            created_at: "t5".into(),
        })
        .expect("route B");
}

#[test]
fn e2_route_fork_lifecycle_preserves_local_lineage_and_history() {
    let mut fixture = fixture();
    {
        let mut control = AgentControl::new(&mut fixture.repository);
        setup_exploration(&mut control, &fixture.v1, &fixture.c1);
        control
            .attach_execution_to_route(RouteExecutionAttachment {
                route_id: "route:e2:a".into(),
                execution_id: "execution:e2:a1".into(),
                attached_at: "t5".into(),
            })
            .expect("route A initial membership");

        fs::write(fixture.workspace_a_path.join("state.txt"), b"two").expect("route A change");
        let v2 = publish(
            &mut control,
            WORKSPACE_A,
            &fixture.lease_a,
            Some(&fixture.v1),
            "op-e2-v2",
            "t6",
        );
        control
            .attach_operation_to_execution("execution:e2:a1", "op-e2-v2", "t6")
            .expect("operation ownership A");
        control
            .start_execution("execution:e2:a1", 0, "t7")
            .expect("start route A execution");
        control
            .finish_execution(
                "execution:e2:a1",
                "interrupted",
                Some("provider_failure"),
                1,
                "t8",
            )
            .expect("interrupt route A execution");
        assert_eq!(
            control.route("route:e2:a").unwrap().unwrap().status,
            "active"
        );

        let route_b = control.route("route:e2:b").unwrap().unwrap();
        assert_eq!(route_b.source_kind, "checkpoint");
        assert_eq!(route_b.source_id, fixture.c1);
        assert_eq!(
            control.checkpoint(&fixture.c1).unwrap().unwrap().version_id,
            fixture.v1
        );

        let b_initial_revision = control.workspace(WORKSPACE_B).unwrap().unwrap().revision;
        control
            .materialize_from_version(
                WORKSPACE_B,
                &fixture.v1,
                &fixture.lease_b,
                b_initial_revision,
                20,
                "t9",
            )
            .expect("materialize V1 into Workspace B");
        assert_eq!(
            fs::read(fixture.workspace_b_path.join("state.txt")).unwrap(),
            b"one"
        );

        control
            .create_execution(CreateExecutionRequest {
                execution_id: "execution:e2:b1".into(),
                task_id: TASK.into(),
                agent_id: AGENT_B.into(),
                parent_execution_id: None,
                workspace_id: Some(WORKSPACE_B.into()),
                base_version_id: None,
                current_version_id: None,
                created_at: "t10".into(),
            })
            .expect("route B execution");
        control
            .attach_execution_to_route(RouteExecutionAttachment {
                route_id: "route:e2:b".into(),
                execution_id: "execution:e2:b1".into(),
                attached_at: "t10".into(),
            })
            .expect("route B membership");
        let v3 = publish(
            &mut control,
            WORKSPACE_B,
            &fixture.lease_b,
            None,
            "op-e2-v3",
            "t11",
        );
        control
            .attach_operation_to_execution("execution:e2:b1", "op-e2-v3", "t11")
            .expect("operation ownership B root");
        assert_eq!(v3.version.parent_version_id, None);
        assert_eq!(v3.version.workspace_id, WORKSPACE_B);

        fs::write(fixture.workspace_b_path.join("state.txt"), b"four").expect("route B change");
        let v4 = publish(
            &mut control,
            WORKSPACE_B,
            &fixture.lease_b,
            Some(&v3.version.version_id),
            "op-e2-v4",
            "t12",
        );
        control
            .attach_operation_to_execution("execution:e2:b1", "op-e2-v4", "t12")
            .expect("operation ownership B child");
        assert_eq!(
            v4.version.parent_version_id.as_deref(),
            Some(v3.version.version_id.as_str())
        );

        let a2 = control
            .create_execution(CreateExecutionRequest {
                execution_id: "execution:e2:a2".into(),
                task_id: TASK.into(),
                agent_id: AGENT_B.into(),
                parent_execution_id: Some("execution:e2:a1".into()),
                workspace_id: Some(WORKSPACE_A.into()),
                base_version_id: Some(fixture.v1.clone()),
                current_version_id: None,
                created_at: "t13".into(),
            })
            .expect("handoff target execution");
        assert_eq!(a2.parent_execution_id.as_deref(), Some("execution:e2:a1"));
        control
            .attach_execution_to_route(RouteExecutionAttachment {
                route_id: "route:e2:a".into(),
                execution_id: a2.execution_id.clone(),
                attached_at: "t13".into(),
            })
            .expect("route A resumed membership");
        control
            .create_handoff(HandoffCreation {
                handoff_id: "handoff:e2:a".into(),
                task_id: TASK.into(),
                from_execution_id: "execution:e2:a1".into(),
                to_execution_id: a2.execution_id,
                source_version_id: Some(fixture.v1.clone()),
                checkpoint_id: Some(fixture.c1.clone()),
                reason: "continue after failure".into(),
                actor_agent_id: AGENT_A.into(),
                requester_execution_id: None,
                request_id: "request:e2:handoff".into(),
                created_at: "t14".into(),
            })
            .expect("handoff");
        let resumed = control
            .resume_from_checkpoint(ResumeCreation {
                execution_id: "execution:e2:a3".into(),
                task_id: TASK.into(),
                agent_id: AGENT_B.into(),
                parent_execution_id: Some("execution:e2:a2".into()),
                workspace_id: Some(WORKSPACE_A.into()),
                source_version_id: None,
                checkpoint_id: Some(fixture.c1.clone()),
                request_id: "request:e2:resume".into(),
                created_at: "t15".into(),
            })
            .expect("resume route A from checkpoint");
        assert_eq!(resumed.source_version_id, fixture.v1);
        control
            .attach_execution_to_route(RouteExecutionAttachment {
                route_id: "route:e2:a".into(),
                execution_id: resumed.execution_id.clone(),
                attached_at: "t15".into(),
            })
            .expect("route A resume membership");
        assert_eq!(
            control
                .update_route_status("route:e2:a", "paused")
                .unwrap()
                .status,
            "paused"
        );
        assert_eq!(
            control
                .update_route_status("route:e2:a", "active")
                .unwrap()
                .status,
            "active"
        );

        let rollback_revision = control.workspace(WORKSPACE_A).unwrap().unwrap().revision;
        let rollback = control
            .rollback(&RollbackCreation {
                rollback_id: "rollback:e2:a".into(),
                task_id: TASK.into(),
                execution_id: "execution:e2:a1".into(),
                workspace_id: WORKSPACE_A.into(),
                target: RollbackTarget::Version(fixture.v1.clone()),
                actor_agent_id: AGENT_A.into(),
                request_id: "request:e2:rollback".into(),
                created_at: "t16".into(),
                lease: Some(fixture.lease_a.clone()),
                expected_workspace_revision: Some(rollback_revision),
                now_ms: Some(30),
            })
            .expect("rollback route A to V1");
        assert_eq!(rollback.status, "completed");
        assert_eq!(
            fs::read(fixture.workspace_a_path.join("state.txt")).unwrap(),
            b"one"
        );

        assert_eq!(control.route_executions("route:e2:a").unwrap().len(), 3);
        assert_eq!(control.route_executions("route:e2:b").unwrap().len(), 1);
        assert_eq!(
            control
                .execution_for_operation("op-e2-v3")
                .unwrap()
                .unwrap()
                .execution_id,
            "execution:e2:b1"
        );
        assert!(control.handoff("handoff:e2:a").unwrap().is_some());
        assert!(control.resume_record("execution:e2:a3").unwrap().is_some());
        assert_eq!(
            v2.version.parent_version_id.as_deref(),
            Some(fixture.v1.as_str())
        );
    }

    let project_path = fixture.repository_dir.path().to_path_buf();
    let workspace_a_path = fixture.workspace_a_path.clone();
    let workspace_b_path = fixture.workspace_b_path.clone();
    drop(fixture.repository);
    let reopened = Repository::open(project_path).expect("cold reopen");
    assert_eq!(
        reopened
            .metadata()
            .list_versions(WORKSPACE_A)
            .unwrap()
            .len(),
        2
    );
    let versions_b = reopened.metadata().list_versions(WORKSPACE_B).unwrap();
    assert_eq!(versions_b.len(), 2);
    let v3 = versions_b.first().expect("B root");
    let v4 = versions_b.last().expect("B child");
    assert_eq!(v3.parent_version_id, None);
    assert_eq!(
        v4.parent_version_id.as_deref(),
        Some(v3.version_id.as_str())
    );
    assert_eq!(
        reopened
            .metadata()
            .route("route:e2:b")
            .unwrap()
            .unwrap()
            .source_id,
        "checkpoint:e2:c1"
    );
    assert_eq!(
        reopened
            .metadata()
            .checkpoint("checkpoint:e2:c1")
            .unwrap()
            .unwrap()
            .version_id,
        fixture.v1
    );
    assert!(reopened
        .metadata()
        .handoff("handoff:e2:a")
        .unwrap()
        .is_some());
    assert!(reopened
        .metadata()
        .resume_record("execution:e2:a3")
        .unwrap()
        .is_some());
    assert!(reopened
        .metadata()
        .rollback_record("rollback:e2:a")
        .unwrap()
        .is_some());
    assert_eq!(
        fs::read(workspace_a_path.join("state.txt")).unwrap(),
        b"one"
    );
    assert_eq!(
        fs::read(workspace_b_path.join("state.txt")).unwrap(),
        b"four"
    );
    let route_b_membership = reopened.metadata().route_executions("route:e2:b").unwrap();
    assert_eq!(route_b_membership.len(), 1);
    assert_eq!(route_b_membership[0].execution_id, "execution:e2:b1");
}

#[test]
fn e2_route_statuses_and_idempotency_are_explicit() {
    let mut fixture = fixture();
    let mut control = AgentControl::new(&mut fixture.repository);
    setup_exploration(&mut control, &fixture.v1, &fixture.c1);

    for (suffix, status) in [
        ("paused", "paused"),
        ("failed", "failed"),
        ("completed", "completed"),
        ("abandoned", "abandoned"),
    ] {
        control
            .create_route(CreateRouteRequest {
                route_id: format!("route:e2:{suffix}"),
                exploration_id: "exploration:e2".into(),
                source_kind: "version".into(),
                source_id: fixture.v1.clone(),
                status: status.into(),
                terminal_version_id: None,
                created_by: AGENT_A.into(),
                purpose_ref: Some(format!("{status} route")),
                created_at: format!("t-{status}"),
            })
            .expect("route status");
    }

    let request = CreateRouteRequest {
        route_id: "route:e2:idempotent".into(),
        exploration_id: "exploration:e2".into(),
        source_kind: "version".into(),
        source_id: fixture.v1.clone(),
        status: "active".into(),
        terminal_version_id: None,
        created_by: AGENT_A.into(),
        purpose_ref: None,
        created_at: "t-idempotent".into(),
    };
    let first = control.create_route(request.clone()).expect("route");
    assert_eq!(control.create_route(request.clone()).unwrap(), first);
    let mut changed = request;
    changed.status = "paused".into();
    assert_eq!(
        control.create_route(changed).unwrap_err().code(),
        "IDEMPOTENCY_KEY_REUSE"
    );
    assert_eq!(
        control.route("route:e2:paused").unwrap().unwrap().status,
        "paused"
    );
    assert_eq!(
        control.route("route:e2:failed").unwrap().unwrap().status,
        "failed"
    );
    assert_eq!(
        control.route("route:e2:completed").unwrap().unwrap().status,
        "completed"
    );
    assert_eq!(
        control.route("route:e2:abandoned").unwrap().unwrap().status,
        "abandoned"
    );
    assert_eq!(
        control
            .update_route_status("route:e2:paused", "completed")
            .unwrap()
            .status,
        "completed"
    );
    assert_eq!(
        control
            .update_route_status("route:e2:paused", "completed")
            .unwrap()
            .status,
        "completed"
    );
}
