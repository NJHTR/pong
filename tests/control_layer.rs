//! Provider-neutral control facade integration coverage.
//!
//! This test deliberately uses provider names only as durable metadata. The
//! real external-process validation lives in `agent_handoff_e2e.rs`; here the
//! same workflow is exercised through the small public control facade.

use pong_core::metadata::{
    CheckpointCreation, HandoffCreation, MetadataFailpoint, MetadataFailpoints, ResumeCreation,
    WorkspaceUpdate,
};
use pong_core::redaction::Redactor;
use pong_core::{
    AgentControl, CreateExecutionRequest, CreateTaskRequest, CreateWorkspaceRequest,
    PublishVersionRequest, RegisterAgentRequest, Repository, SnapshotOptions, WorkspaceManager,
};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "project-control-layer";
const ENVIRONMENT: &str = "environment-control-layer";
const TASK: &str = "task-control-layer";
const CODEX: &str = "agent-control-codex";
const CLAUDE: &str = "agent-control-claude";
const W1: &str = "workspace-control-codex";
const W2: &str = "workspace-control-claude";

struct Fixture {
    repository_dir: TempDir,
    workspace_dir: TempDir,
    repository: Repository,
    w1_path: PathBuf,
    w2_path: PathBuf,
}

fn fixture() -> Fixture {
    let repository_dir = tempdir().expect("repository directory");
    let workspace_dir = tempdir().expect("workspace parent directory");
    let mut repository = Repository::init(repository_dir.path()).expect("initialize repository");
    repository
        .metadata_mut()
        .record_environment(
            ENVIRONMENT,
            PROJECT,
            &json!({"schema_version": 1, "os": "windows", "test": "control-layer"}),
            "t0",
        )
        .expect("environment");

    let mut control = AgentControl::new(&mut repository);
    for (agent_id, provider, display_name) in [
        (CODEX, "codex", "Codex"),
        (CLAUDE, "claude-code", "Claude Code"),
    ] {
        control
            .register_agent(RegisterAgentRequest {
                agent_id: agent_id.into(),
                provider: provider.into(),
                display_name: Some(display_name.into()),
                created_at: "t0".into(),
            })
            .expect("agent registration");
    }
    control
        .create_task(CreateTaskRequest {
            task_id: TASK.into(),
            project_id: PROJECT.into(),
            goal: "continue work through a provider handoff".into(),
            context_ref: Some("opaque://control-layer".into()),
            created_at: "t0".into(),
        })
        .expect("task");

    let w1_path = workspace_dir.path().join("codex");
    let w2_path = workspace_dir.path().join("claude");
    for (workspace_id, path) in [(W1, &w1_path), (W2, &w2_path)] {
        control
            .create_workspace(CreateWorkspaceRequest {
                workspace_id: workspace_id.into(),
                project_id: PROJECT.into(),
                path: path.clone(),
                branch_ref: None,
                environment_id: Some(ENVIRONMENT.into()),
                now: "t1".into(),
            })
            .expect("workspace");
    }
    drop(control);

    Fixture {
        repository_dir,
        workspace_dir,
        repository,
        w1_path,
        w2_path,
    }
}

fn acquire(
    repository: &mut Repository,
    workspace_id: &str,
    agent_id: &str,
    now_ms: i64,
) -> pong_core::LeaseToken {
    AgentControl::new(repository)
        .acquire_workspace(pong_core::AcquireWorkspaceRequest {
            workspace_id: workspace_id.into(),
            agent_id: agent_id.into(),
            now_ms,
            ttl_ms: 1_000_000,
        })
        .expect("workspace lease")
}

#[allow(clippy::too_many_arguments)]
fn publish(
    repository: &mut Repository,
    workspace_id: &str,
    lease: &pong_core::LeaseToken,
    expected_workspace_revision: i64,
    operation_id: &str,
    request_id: &str,
    now_ms: i64,
    created_at: &str,
) -> pong_core::PublishVersionResult {
    AgentControl::new(repository)
        .publish_version(publish_request(
            workspace_id,
            lease,
            expected_workspace_revision,
            operation_id,
            request_id,
            now_ms,
            created_at,
        ))
        .expect("publish version")
}

#[allow(clippy::too_many_arguments)]
fn publish_request(
    workspace_id: &str,
    lease: &pong_core::LeaseToken,
    expected_workspace_revision: i64,
    operation_id: &str,
    request_id: &str,
    now_ms: i64,
    created_at: &str,
) -> PublishVersionRequest {
    PublishVersionRequest {
        workspace_id: workspace_id.into(),
        lease: lease.clone(),
        expected_workspace_revision,
        now_ms,
        created_at: created_at.into(),
        operation_id: operation_id.into(),
        request_id: request_id.into(),
        session_id: Some(format!("session:{workspace_id}")),
        tool: Some("control-layer-test".into()),
        parent_version_id: None,
        update_version_head: true,
    }
}

fn snapshot_workspace(
    repository: &mut Repository,
    workspace_id: &str,
    lease: &pong_core::LeaseToken,
    expected_revision: i64,
    now_ms: i64,
    now: &str,
) -> pong_core::Snapshot {
    let revision = workspace_revision(repository, workspace_id);
    assert_eq!(revision, expected_revision);
    WorkspaceManager::new(repository, Redactor::default())
        .snapshot_local(workspace_id, lease, SnapshotOptions::default(), now_ms, now)
        .expect("pre-publish Snapshot")
}

fn table_count(repository: &Repository, table: &str) -> i64 {
    let connection = Connection::open(repository.active_metadata_path()).expect("SQLite metadata");
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}

fn workspace_revision(repository: &Repository, workspace_id: &str) -> i64 {
    repository
        .metadata()
        .workspace(workspace_id)
        .expect("workspace query")
        .expect("workspace")
        .revision
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("read file")
}

#[test]
fn revision_authority_rejects_stale_publication_without_durable_mutation() {
    let mut fixture = fixture();
    let lease = acquire(&mut fixture.repository, W1, CODEX, 10);
    fs::write(fixture.w1_path.join("state.txt"), "one").expect("first state");
    let first = publish(
        &mut fixture.repository,
        W1,
        &lease,
        0,
        "operation:control:revision:first",
        "request:control:revision:first",
        20,
        "t2",
    );
    assert_eq!(first.workspace.revision, 2);

    let before = AgentControl::new(&mut fixture.repository)
        .workspace(W1)
        .expect("workspace query")
        .expect("workspace");
    let versions_before = fixture.repository.metadata().list_versions(W1).unwrap();
    let snapshots_before = table_count(&fixture.repository, "snapshots");
    fs::write(fixture.w1_path.join("state.txt"), "two").expect("newer state");
    let stale = publish_request(
        W1,
        &lease,
        0,
        "operation:control:revision:stale",
        "request:control:revision:stale",
        30,
        "t3",
    );
    let error = AgentControl::new(&mut fixture.repository)
        .publish_version(stale)
        .expect_err("stale publication must fail");
    assert_eq!(error.code(), "CONFLICT");
    assert_eq!(
        AgentControl::new(&mut fixture.repository)
            .workspace(W1)
            .unwrap()
            .unwrap(),
        before
    );
    assert_eq!(
        fixture.repository.metadata().list_versions(W1).unwrap(),
        versions_before
    );
    assert_eq!(
        table_count(&fixture.repository, "snapshots"),
        snapshots_before
    );
    assert!(AgentControl::new(&mut fixture.repository)
        .operation("operation:control:revision:stale")
        .unwrap()
        .is_none());

    let refreshed = publish(
        &mut fixture.repository,
        W1,
        &lease,
        before.revision,
        "operation:control:revision:refreshed",
        "request:control:revision:refreshed",
        40,
        "t4",
    );
    assert_eq!(refreshed.workspace.revision, before.revision + 2);
    assert_eq!(refreshed.version.workspace_id, W1);
    let untouched = AgentControl::new(&mut fixture.repository)
        .workspace(W2)
        .unwrap()
        .unwrap();
    assert_eq!(untouched.revision, 0);
    assert_eq!(untouched.head, None);
    assert_eq!(untouched.version_head_id, None);
}

#[test]
fn lease_failures_are_rejected_before_snapshot_or_operation_publication() {
    let mut fixture = fixture();
    let lease = acquire(&mut fixture.repository, W1, CODEX, 10);
    fs::write(fixture.w1_path.join("state.txt"), "lease guarded").expect("state");

    let mut foreign = lease.clone();
    foreign.agent_id = CLAUDE.into();
    let foreign_error = AgentControl::new(&mut fixture.repository)
        .publish_version(publish_request(
            W1,
            &foreign,
            0,
            "operation:control:foreign-lease",
            "request:control:foreign-lease",
            20,
            "t2",
        ))
        .expect_err("foreign lease");
    assert_eq!(foreign_error.code(), "CONFLICT");

    let expired_error = AgentControl::new(&mut fixture.repository)
        .publish_version(publish_request(
            W1,
            &lease,
            0,
            "operation:control:expired-lease",
            "request:control:expired-lease",
            lease.expires_at_ms,
            "t3",
        ))
        .expect_err("expired lease");
    assert_eq!(expired_error.code(), "CONFLICT");
    assert_eq!(table_count(&fixture.repository, "snapshots"), 0);
    assert_eq!(table_count(&fixture.repository, "versions"), 0);
    assert_eq!(table_count(&fixture.repository, "operations"), 0);
    assert_eq!(workspace_revision(&fixture.repository, W1), 0);
}

#[test]
fn invalid_workspace_parent_and_request_fail_before_snapshot_publication() {
    let mut fixture = fixture();
    let lease = acquire(&mut fixture.repository, W1, CODEX, 10);
    fs::write(fixture.w1_path.join("state.txt"), "unchanged").expect("state");

    let mut invalid = publish_request(W1, &lease, 0, "", "request:control:invalid", 20, "t2");
    invalid.parent_version_id = Some(String::new());
    let error = AgentControl::new(&mut fixture.repository)
        .publish_version(invalid)
        .expect_err("invalid request");
    assert_eq!(error.code(), "INVALID_INPUT");

    let missing_workspace = AgentControl::new(&mut fixture.repository)
        .publish_version(publish_request(
            "workspace-missing",
            &lease,
            0,
            "operation:control:missing-workspace",
            "request:control:missing-workspace",
            20,
            "t2",
        ))
        .expect_err("missing workspace");
    assert_eq!(missing_workspace.code(), "NOT_FOUND");

    let mut missing_parent = publish_request(
        W1,
        &lease,
        0,
        "operation:control:missing-parent",
        "request:control:missing-parent",
        20,
        "t2",
    );
    missing_parent.parent_version_id = Some("version-missing".into());
    let error = AgentControl::new(&mut fixture.repository)
        .publish_version(missing_parent)
        .expect_err("missing parent");
    assert_eq!(error.code(), "NOT_FOUND");
    assert_eq!(table_count(&fixture.repository, "snapshots"), 0);
    assert_eq!(table_count(&fixture.repository, "versions"), 0);
    assert_eq!(table_count(&fixture.repository, "operations"), 0);
    assert_eq!(workspace_revision(&fixture.repository, W1), 0);
}

#[test]
fn precommit_version_failure_has_one_retryable_operation_and_one_result() {
    let mut fixture = fixture();
    let lease = acquire(&mut fixture.repository, W1, CODEX, 10);
    fs::write(fixture.w1_path.join("state.txt"), "durable snapshot").expect("state");
    let snapshot = snapshot_workspace(&mut fixture.repository, W1, &lease, 0, 20, "t2");
    let request = publish_request(
        W1,
        &lease,
        1,
        "operation:control:precommit",
        "request:control:precommit",
        30,
        "t3",
    );
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeOperationFinishCommit,
        ));
    let error = AgentControl::new(&mut fixture.repository)
        .publish_version(request.clone())
        .expect_err("precommit fault");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert_eq!(table_count(&fixture.repository, "versions"), 0);
    let started = AgentControl::new(&mut fixture.repository)
        .operation(&request.operation_id)
        .unwrap()
        .expect("durable publication intent");
    assert_eq!(started.lifecycle_status, "started");
    assert_eq!(workspace_revision(&fixture.repository, W1), 1);

    let recovered = AgentControl::new(&mut fixture.repository)
        .publish_version(request.clone())
        .expect("retry publication");
    assert_eq!(recovered.snapshot.snapshot_id, snapshot.snapshot_id);
    assert_eq!(recovered.workspace.revision, 2);
    assert_eq!(table_count(&fixture.repository, "versions"), 1);
    assert_eq!(
        AgentControl::new(&mut fixture.repository)
            .operation(&request.operation_id)
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "completed"
    );

    let replay = AgentControl::new(&mut fixture.repository)
        .publish_version(request)
        .expect("completed replay");
    assert_eq!(replay.version.version_id, recovered.version.version_id);
    assert_eq!(replay.workspace.revision, 2);
    assert_eq!(table_count(&fixture.repository, "versions"), 1);
}

#[test]
fn postcommit_uncertainty_resumes_after_cold_reopen() {
    let mut fixture = fixture();
    let lease = acquire(&mut fixture.repository, W1, CODEX, 10);
    fs::write(fixture.w1_path.join("state.txt"), "postcommit").expect("state");
    snapshot_workspace(&mut fixture.repository, W1, &lease, 0, 20, "t2");
    let request = publish_request(
        W1,
        &lease,
        1,
        "operation:control:postcommit",
        "request:control:postcommit",
        30,
        "t3",
    );
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterOperationFinishCommit,
        ));
    let error = AgentControl::new(&mut fixture.repository)
        .publish_version(request.clone())
        .expect_err("postcommit uncertainty");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert_eq!(table_count(&fixture.repository, "versions"), 1);
    assert_eq!(workspace_revision(&fixture.repository, W1), 1);
    assert_eq!(
        AgentControl::new(&mut fixture.repository)
            .operation(&request.operation_id)
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "completed"
    );

    let repository_path = fixture.repository_dir.path().to_path_buf();
    drop(fixture.repository);
    let mut reopened = Repository::open(repository_path).expect("cold reopen");
    let recovered = AgentControl::new(&mut reopened)
        .publish_version(request.clone())
        .expect("recover committed Version");
    assert_eq!(recovered.workspace.revision, 2);
    assert_eq!(
        recovered.workspace.version_head_id,
        Some(recovered.version.version_id.clone())
    );
    let replay = AgentControl::new(&mut reopened)
        .publish_version(request)
        .expect("completed replay");
    assert_eq!(replay.version.version_id, recovered.version.version_id);
    assert_eq!(replay.workspace.revision, 2);
    assert_eq!(table_count(&reopened, "versions"), 1);
}

fn interrupted_publication_fixture(
    operation_id: &str,
    request_id: &str,
) -> (Fixture, pong_core::LeaseToken, PublishVersionRequest) {
    let mut fixture = fixture();
    let lease = acquire(&mut fixture.repository, W1, CODEX, 10);
    fs::write(fixture.w1_path.join("state.txt"), "replay source").expect("state");
    snapshot_workspace(&mut fixture.repository, W1, &lease, 0, 20, "t2");
    let request = publish_request(W1, &lease, 1, operation_id, request_id, 30, "t3");
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeOperationFinishCommit,
        ));
    let error = AgentControl::new(&mut fixture.repository)
        .publish_version(request.clone())
        .expect_err("interrupted Version publication");
    assert_eq!(error.code(), "FAULT_INJECTED");
    (fixture, lease, request)
}

#[test]
fn replay_with_missing_snapshot_fails_closed_after_cold_reopen() {
    let (fixture, _lease, request) = interrupted_publication_fixture(
        "operation:control:missing-snapshot",
        "request:control:missing-snapshot",
    );
    let metadata_path = fixture.repository.active_metadata_path().to_path_buf();
    let repository_path = fixture.repository_dir.path().to_path_buf();
    drop(fixture.repository);
    Connection::open(metadata_path)
        .unwrap()
        .execute("DELETE FROM snapshots", [])
        .expect("remove Snapshot metadata");
    let mut reopened = Repository::open(repository_path).expect("cold reopen");
    let error = AgentControl::new(&mut reopened)
        .publish_version(request.clone())
        .expect_err("missing replay Snapshot");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
    assert_eq!(table_count(&reopened, "versions"), 0);
    assert_eq!(
        AgentControl::new(&mut reopened)
            .operation(&request.operation_id)
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "started"
    );
}

#[test]
fn replay_with_foreign_snapshot_ownership_fails_closed() {
    let (fixture, _lease, request) = interrupted_publication_fixture(
        "operation:control:foreign-snapshot",
        "request:control:foreign-snapshot",
    );
    let metadata_path = fixture.repository.active_metadata_path().to_path_buf();
    let repository_path = fixture.repository_dir.path().to_path_buf();
    drop(fixture.repository);
    Connection::open(metadata_path)
        .unwrap()
        .execute("UPDATE snapshots SET workspace_id = ?1", [W2])
        .expect("corrupt Snapshot ownership");
    let mut reopened = Repository::open(repository_path).expect("cold reopen");
    let error = AgentControl::new(&mut reopened)
        .publish_version(request)
        .expect_err("foreign replay Snapshot");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
    assert_eq!(table_count(&reopened, "versions"), 0);
    assert_eq!(workspace_revision(&reopened, W1), 1);
}

#[test]
fn mutation_after_publication_intent_is_not_overwritten_by_retry() {
    let (mut fixture, lease, request) = interrupted_publication_fixture(
        "operation:control:concurrent-mutation",
        "request:control:concurrent-mutation",
    );
    let before = fixture
        .repository
        .metadata()
        .workspace(W1)
        .unwrap()
        .unwrap();
    fixture
        .repository
        .metadata_mut()
        .update_workspace(WorkspaceUpdate {
            workspace_id: W1,
            expected_revision: before.revision,
            lease: &lease,
            branch_ref: Some("newer-writer"),
            head: before.head.as_deref(),
            environment_id: before.environment_id.as_deref(),
            status: &before.status,
            updated_at: "t4",
            now_ms: 40,
        })
        .expect("concurrent Workspace mutation");
    let newer = fixture
        .repository
        .metadata()
        .workspace(W1)
        .unwrap()
        .unwrap();
    assert_eq!(newer.revision, 2);
    let error = AgentControl::new(&mut fixture.repository)
        .publish_version(request.clone())
        .expect_err("stale retry");
    assert_eq!(error.code(), "CONFLICT");
    assert_eq!(
        fixture.repository.metadata().workspace(W1).unwrap(),
        Some(newer)
    );
    assert_eq!(table_count(&fixture.repository, "versions"), 0);
    assert_eq!(
        AgentControl::new(&mut fixture.repository)
            .operation(&request.operation_id)
            .unwrap()
            .unwrap()
            .lifecycle_status,
        "started"
    );
}

#[test]
fn three_provider_metadata_values_publish_to_isolated_workspaces() {
    const THIRD_AGENT: &str = "agent-control-third";
    const W3: &str = "workspace-control-third";
    let mut fixture = fixture();
    AgentControl::new(&mut fixture.repository)
        .register_agent(RegisterAgentRequest {
            agent_id: THIRD_AGENT.into(),
            provider: "future-provider".into(),
            display_name: None,
            created_at: "t2".into(),
        })
        .expect("third Agent");
    let w3_path = fixture.workspace_dir.path().join("third");
    AgentControl::new(&mut fixture.repository)
        .create_workspace(CreateWorkspaceRequest {
            workspace_id: W3.into(),
            project_id: PROJECT.into(),
            path: w3_path.clone(),
            branch_ref: None,
            environment_id: Some(ENVIRONMENT.into()),
            now: "t2".into(),
        })
        .expect("third Workspace");
    for path in [&fixture.w1_path, &fixture.w2_path, &w3_path] {
        fs::write(path.join("state.txt"), path.display().to_string()).expect("Workspace state");
    }
    let leases = [
        acquire(&mut fixture.repository, W1, CODEX, 10),
        acquire(&mut fixture.repository, W2, CLAUDE, 10),
        acquire(&mut fixture.repository, W3, THIRD_AGENT, 10),
    ];
    let repository_path = fixture.repository_dir.path().to_path_buf();
    drop(fixture.repository);

    let handles = [W1, W2, W3]
        .into_iter()
        .zip(leases)
        .enumerate()
        .map(|(index, (workspace_id, lease))| {
            let repository_path = repository_path.clone();
            std::thread::spawn(move || {
                let mut repository = Repository::open(repository_path).expect("parallel open");
                AgentControl::new(&mut repository)
                    .publish_version(publish_request(
                        workspace_id,
                        &lease,
                        0,
                        &format!("operation:control:parallel:{index}"),
                        &format!("request:control:parallel:{index}"),
                        20,
                        "t3",
                    ))
                    .expect("parallel publication")
            })
        })
        .collect::<Vec<_>>();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("publication thread"))
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 3);
    for (result, workspace_id) in results.iter().zip([W1, W2, W3]) {
        assert_eq!(result.version.workspace_id, workspace_id);
        assert_eq!(result.snapshot.workspace_id, workspace_id);
        assert_eq!(result.workspace.revision, 2);
        assert_eq!(
            result.workspace.version_head_id.as_deref(),
            Some(result.version.version_id.as_str())
        );
    }
    assert_eq!(
        results
            .iter()
            .map(|result| result.workspace.head.as_deref().unwrap())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
}

#[test]
fn provider_neutral_facade_persists_handoff_and_continuation() {
    let mut fixture = fixture();

    let execution_one = AgentControl::new(&mut fixture.repository)
        .create_execution(CreateExecutionRequest {
            execution_id: "execution-control-codex".into(),
            task_id: TASK.into(),
            agent_id: CODEX.into(),
            parent_execution_id: None,
            workspace_id: Some(W1.into()),
            base_version_id: None,
            current_version_id: None,
            created_at: "t2".into(),
        })
        .expect("Codex execution");
    AgentControl::new(&mut fixture.repository)
        .start_execution(&execution_one.execution_id, 0, "t3")
        .expect("start Codex execution");

    let lease_one = acquire(&mut fixture.repository, W1, CODEX, 10);
    fs::create_dir_all(fixture.w1_path.join("src")).expect("src directory");
    fs::write(
        fixture.w1_path.join("src/calculator.rs"),
        "// Codex\npub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .expect("Codex work");
    let version_one = publish(
        &mut fixture.repository,
        W1,
        &lease_one,
        0,
        "operation:control:codex-version",
        "request:control:codex-version",
        20,
        "t4",
    );
    assert_eq!(version_one.version.workspace_id, W1);
    assert_eq!(
        version_one.workspace.version_head_id,
        Some(version_one.version.version_id.clone())
    );

    let checkpoint_one = AgentControl::new(&mut fixture.repository)
        .create_checkpoint(CheckpointCreation {
            checkpoint_id: "checkpoint-control-codex".into(),
            task_id: TASK.into(),
            execution_id: execution_one.execution_id.clone(),
            workspace_id: W1.into(),
            version_id: version_one.version.version_id.clone(),
            operation_id: Some(version_one.version.creation_operation_id.clone()),
            reason: "Codex handoff checkpoint".into(),
            actor_agent_id: CODEX.into(),
            request_id: "request:control:checkpoint-codex".into(),
            created_at: "t5".into(),
        })
        .expect("checkpoint C1");
    AgentControl::new(&mut fixture.repository)
        .transition_execution(
            &execution_one.execution_id,
            "interrupted",
            Some("handoff"),
            1,
            "t6",
        )
        .expect("interrupt Codex execution");

    let resume = AgentControl::new(&mut fixture.repository)
        .resume_from_checkpoint(ResumeCreation {
            execution_id: "execution-control-claude".into(),
            task_id: TASK.into(),
            agent_id: CLAUDE.into(),
            parent_execution_id: Some(execution_one.execution_id.clone()),
            workspace_id: Some(W2.into()),
            source_version_id: None,
            checkpoint_id: Some(checkpoint_one.checkpoint_id.clone()),
            request_id: "request:control:resume".into(),
            created_at: "t7".into(),
        })
        .expect("resume E2 from C1");
    let handoff_request = HandoffCreation {
        handoff_id: "handoff-control-codex-claude".into(),
        task_id: TASK.into(),
        from_execution_id: execution_one.execution_id.clone(),
        to_execution_id: resume.execution_id.clone(),
        source_version_id: Some(version_one.version.version_id.clone()),
        checkpoint_id: Some(checkpoint_one.checkpoint_id.clone()),
        reason: "Codex stopped; Claude Code continues".into(),
        actor_agent_id: CODEX.into(),
        requester_execution_id: Some(execution_one.execution_id.clone()),
        request_id: "request:control:handoff".into(),
        created_at: "t8".into(),
    };
    let handoff = AgentControl::new(&mut fixture.repository)
        .create_handoff(handoff_request.clone())
        .expect("handoff");
    assert_eq!(
        handoff.source_version_id,
        Some(version_one.version.version_id.clone())
    );
    assert_eq!(
        AgentControl::new(&mut fixture.repository)
            .create_handoff(handoff_request)
            .expect("idempotent handoff retry"),
        handoff
    );

    let w1_before = AgentControl::new(&mut fixture.repository)
        .workspace(W1)
        .expect("W1 query")
        .expect("W1")
        .clone();
    let lease_two = acquire(&mut fixture.repository, W2, CLAUDE, 30);
    let materialized = AgentControl::new(&mut fixture.repository)
        .materialize_from_version(W2, &version_one.version.version_id, &lease_two, 0, 40, "t9")
        .expect("cross-workspace materialization");
    assert_eq!(materialized.workspace_id, W2);
    assert_ne!(materialized.snapshot_id, version_one.snapshot.snapshot_id);
    assert_eq!(
        read(&fixture.w2_path.join("src/calculator.rs")),
        read(&fixture.w1_path.join("src/calculator.rs"))
    );
    let w2_after_materialize = AgentControl::new(&mut fixture.repository)
        .workspace(W2)
        .expect("W2 query")
        .expect("W2")
        .clone();
    assert_eq!(
        w2_after_materialize.head,
        Some(materialized.root_digest.clone())
    );
    assert_eq!(w2_after_materialize.version_head_id, None);

    AgentControl::new(&mut fixture.repository)
        .start_execution(&resume.execution_id, 0, "t10")
        .expect("start Claude execution");
    fs::write(
        fixture.w2_path.join("src/calculator.rs"),
        format!(
            "{}// Claude Code\npub fn subtract(a: i32, b: i32) -> i32 {{ a - b }}\n",
            read(&fixture.w2_path.join("src/calculator.rs"))
        ),
    )
    .expect("Claude continuation");

    let diff = AgentControl::new(&mut fixture.repository)
        .diff_against_version(W2, &version_one.version.version_id)
        .expect("cross-workspace diff");
    assert!(!diff.entries.is_empty());
    assert_eq!(
        AgentControl::new(&mut fixture.repository)
            .workspace(W1)
            .expect("W1 query")
            .expect("W1")
            .head,
        w1_before.head
    );

    let w2_revision_before_publish = workspace_revision(&fixture.repository, W2);
    let version_two = publish(
        &mut fixture.repository,
        W2,
        &lease_two,
        w2_revision_before_publish,
        "operation:control:claude-version",
        "request:control:claude-version",
        50,
        "t11",
    );
    let execution_two_before_current = AgentControl::new(&mut fixture.repository)
        .execution(&resume.execution_id)
        .expect("E2 query")
        .expect("E2");
    let w2_revision_before_current = workspace_revision(&fixture.repository, W2);
    let current = AgentControl::new(&mut fixture.repository)
        .set_execution_current_version(
            &resume.execution_id,
            &version_two.version.version_id,
            &lease_two,
            execution_two_before_current.revision,
            w2_revision_before_current,
            "t12",
            60,
        )
        .expect("set E2 current Version");
    assert_eq!(
        current.current_version_id,
        Some(version_two.version.version_id.clone())
    );

    let checkpoint_two = AgentControl::new(&mut fixture.repository)
        .create_checkpoint(CheckpointCreation {
            checkpoint_id: "checkpoint-control-claude".into(),
            task_id: TASK.into(),
            execution_id: resume.execution_id.clone(),
            workspace_id: W2.into(),
            version_id: version_two.version.version_id.clone(),
            operation_id: Some(version_two.version.creation_operation_id.clone()),
            reason: "Claude continuation checkpoint".into(),
            actor_agent_id: CLAUDE.into(),
            request_id: "request:control:checkpoint-claude".into(),
            created_at: "t13".into(),
        })
        .expect("checkpoint C2");
    assert_eq!(checkpoint_two.version_id, version_two.version.version_id);
    assert_eq!(checkpoint_two.workspace_id, W2);
    AgentControl::new(&mut fixture.repository)
        .finish_execution(
            &resume.execution_id,
            "completed",
            Some("done"),
            current.revision,
            "t14",
        )
        .expect("finish E2");

    let state = AgentControl::new(&mut fixture.repository)
        .state(&resume.execution_id, 70)
        .expect("state view");
    assert_eq!(state.agent.provider, "claude-code");
    assert_eq!(state.task.task_id, TASK);
    assert_eq!(
        state.execution.base_version_id,
        Some(version_one.version.version_id.clone())
    );
    assert_eq!(
        state.execution.current_version_id,
        Some(version_two.version.version_id.clone())
    );
    assert_eq!(state.workspace.as_ref().unwrap().workspace_id, W2);
    assert_eq!(
        state.workspace.as_ref().unwrap().version_head_id,
        Some(version_two.version.version_id.clone())
    );
    assert_eq!(state.checkpoints.len(), 2);
    assert_eq!(state.handoffs.len(), 1);
    assert_eq!(
        state.resume.as_ref().unwrap().source_version_id,
        version_one.version.version_id
    );

    let repository_path = fixture.repository_dir.path().to_path_buf();
    let w1_contents = read(&fixture.w1_path.join("src/calculator.rs"));
    let w1_version_head = AgentControl::new(&mut fixture.repository)
        .workspace(W1)
        .expect("W1 query")
        .expect("W1")
        .version_head_id;
    drop(fixture.repository);
    let mut reopened = Repository::open(repository_path).expect("cold reopen");
    let reopened_state = AgentControl::new(&mut reopened)
        .state("execution-control-claude", 80)
        .expect("reopened state");
    assert_eq!(reopened_state.checkpoints.len(), 2);
    assert_eq!(reopened_state.handoffs.len(), 1);
    assert_eq!(
        reopened_state.resume.unwrap().source_version_id,
        version_one.version.version_id
    );
    assert_eq!(
        read(&fixture.w1_path.join("src/calculator.rs")),
        w1_contents
    );
    assert_eq!(
        AgentControl::new(&mut reopened)
            .workspace(W1)
            .expect("reopened W1")
            .expect("W1")
            .version_head_id,
        w1_version_head
    );
    assert!(read(&fixture.w2_path.join("src/calculator.rs")).contains("subtract"));
    drop(fixture.workspace_dir);
}
