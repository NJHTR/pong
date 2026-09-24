//! M4-008 local multi-runtime concurrency and process-boundary diagnostics.

use pong_core::metadata::{
    AgentIdentity, ExecutionCreation, OperationEnvelope, OperationRef, TaskCreation,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::{AgentControl, LeaseToken, PongError, PublishVersionRequest, Repository};
use serde_json::json;
#[cfg(feature = "direct-access-stress")]
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::thread;
use std::time::Duration;
#[cfg(feature = "direct-access-stress")]
use std::time::Instant;
use tempfile::{tempdir, TempDir};

const CHILD_MODE: &str = "PONG_LOCAL_CONCURRENCY_CHILD_MODE";
const CHILD_ROOT: &str = "PONG_LOCAL_CONCURRENCY_ROOT";
const CHILD_READY: &str = "PONG_LOCAL_CONCURRENCY_READY";
const CHILD_STOP: &str = "PONG_LOCAL_CONCURRENCY_STOP";
const CHILD_RESULT: &str = "PONG_LOCAL_CONCURRENCY_RESULT";
const CHILD_INDEX: &str = "PONG_LOCAL_CONCURRENCY_INDEX";
const CHILD_WORKSPACE: &str = "PONG_LOCAL_CONCURRENCY_WORKSPACE";
const CHILD_AGENT: &str = "PONG_LOCAL_CONCURRENCY_AGENT";
const CHILD_EPOCH: &str = "PONG_LOCAL_CONCURRENCY_EPOCH";
const CHILD_EXPIRY: &str = "PONG_LOCAL_CONCURRENCY_EXPIRY";

struct Fixture {
    _repository_dir: TempDir,
    _workspace_parent: TempDir,
    repository: Repository,
}

struct WaitedChild(Child);

impl WaitedChild {
    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.0.wait()
    }
}

impl Drop for WaitedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn fixture() -> Fixture {
    let repository_dir = tempdir().unwrap();
    let workspace_parent = tempdir().unwrap();
    let mut repository = Repository::init(repository_dir.path()).unwrap();
    repository
        .metadata_mut()
        .record_environment("env", "project", &json!({"schema_version": 1}), "t0")
        .unwrap();
    for agent_id in ["agent-a", "agent-b"] {
        repository
            .metadata_mut()
            .create_agent_identity(&AgentIdentity {
                agent_id: agent_id.into(),
                provider: "runtime-neutral".into(),
                display_name: None,
                created_at: "t0".into(),
            })
            .unwrap();
    }
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: "task".into(),
            project_id: "project".into(),
            goal: "local concurrency".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .unwrap();
    let workspace = workspace_parent.path().join("workspace");
    WorkspaceManager::new(&mut repository, Redactor::default())
        .create_local("workspace", "project", workspace, None, Some("env"), "t0")
        .unwrap();
    for (execution_id, agent_id) in [("execution-a", "agent-a"), ("execution-b", "agent-b")] {
        repository
            .metadata_mut()
            .create_execution(&ExecutionCreation {
                execution_id: execution_id.into(),
                task_id: "task".into(),
                agent_id: agent_id.into(),
                parent_execution_id: None,
                workspace_id: Some("workspace".into()),
                base_version_id: None,
                current_version_id: None,
                created_at: "t0".into(),
            })
            .unwrap();
    }
    Fixture {
        _repository_dir: repository_dir,
        _workspace_parent: workspace_parent,
        repository,
    }
}

fn operation(operation_id: &str, request_id: &str, action: &str) -> OperationEnvelope {
    OperationEnvelope {
        operation_id: operation_id.into(),
        project_id: "project".into(),
        request_id: request_id.into(),
        agent_id: "agent-a".into(),
        session_id: "runtime-a".into(),
        workspace_id: Some("workspace".into()),
        environment_id: Some("env".into()),
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "t1".into(),
        tool: "runtime".into(),
        action: action.into(),
        input_refs: vec![OperationRef {
            kind: "execution".into(),
            reference: "execution-a".into(),
            media_type: None,
        }],
        output_refs: Vec::new(),
        resource: None,
        before_state: None,
        after_state: None,
        reversibility: "UNKNOWN".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "EXTERNAL".into(),
        policy_decision: None,
    }
}

#[test]
fn lease_contention_expiry_and_epoch_recovery_are_deterministic() {
    let mut f = fixture();
    let first = WorkspaceManager::new(&mut f.repository, Redactor::default())
        .acquire_lease("workspace", "agent-a", 10, 100)
        .unwrap();
    let conflict = WorkspaceManager::new(&mut f.repository, Redactor::default())
        .acquire_lease("workspace", "agent-b", 20, 100)
        .unwrap_err();
    assert!(matches!(conflict, PongError::Conflict(_)));

    let takeover = WorkspaceManager::new(&mut f.repository, Redactor::default())
        .acquire_lease("workspace", "agent-b", 111, 100)
        .unwrap();
    assert!(takeover.epoch > first.epoch);
    assert!(
        WorkspaceManager::new(&mut f.repository, Redactor::default())
            .renew_lease(&first, 112, 100)
            .is_err()
    );
}

#[test]
fn execution_revision_rejects_a_stale_runtime_without_overwrite() {
    let mut f = fixture();
    f.repository
        .metadata_mut()
        .start_execution("execution-a", 0, "t1")
        .unwrap();
    f.repository
        .metadata_mut()
        .transition_execution("execution-a", "paused", None, 1, "t2")
        .unwrap();
    let stale =
        f.repository
            .metadata_mut()
            .transition_execution("execution-a", "completed", None, 1, "t3");
    assert!(matches!(stale, Err(PongError::Conflict(_))));
    let execution = f
        .repository
        .metadata()
        .execution("execution-a")
        .unwrap()
        .unwrap();
    assert_eq!(execution.state, "paused");
    assert_eq!(execution.revision, 2);
}

#[test]
fn operation_identity_replays_exact_request_and_rejects_changed_reuse() {
    let mut f = fixture();
    let envelope = operation("operation-1", "request-1", "work");
    let first = f
        .repository
        .metadata_mut()
        .start_operation(envelope.clone())
        .unwrap();
    let replay = f
        .repository
        .metadata_mut()
        .start_operation(envelope)
        .unwrap();
    assert_eq!(first, replay);

    let changed = f.repository.metadata_mut().start_operation(operation(
        "operation-1",
        "request-1",
        "different",
    ));
    assert!(matches!(changed, Err(PongError::IdempotencyKeyReuse(_))));

    let second = f
        .repository
        .metadata_mut()
        .start_operation(operation("operation-2", "request-2", "work"))
        .unwrap();
    assert_ne!(first.operation_id, second.operation_id);
}

#[test]
fn started_operation_survives_runtime_crash_boundary_and_cold_reopen() {
    let mut f = fixture();
    f.repository
        .metadata_mut()
        .start_operation(operation("operation-crash", "request-crash", "work"))
        .unwrap();
    let root = f._repository_dir.path().to_path_buf();
    drop(f.repository);
    let reopened = Repository::open(root).unwrap();
    let operation = reopened
        .metadata()
        .operation_record("operation-crash")
        .unwrap()
        .unwrap();
    assert_eq!(operation.lifecycle_status, "started");
    assert_ne!(operation.lifecycle_status, "completed");
}

/// Child entry point for the real process-open diagnostic.
#[test]
fn multi_process_child() {
    let Ok(mode) = env::var(CHILD_MODE) else {
        return;
    };
    let root = PathBuf::from(env::var_os(CHILD_ROOT).unwrap());
    if mode == "hold" {
        let _repository = Repository::open(root).unwrap();
        let ready = PathBuf::from(env::var_os(CHILD_READY).unwrap());
        let stop = PathBuf::from(env::var_os(CHILD_STOP).unwrap());
        fs::write(&ready, b"ready").unwrap();
        while !stop.exists() {
            thread::sleep(Duration::from_millis(10));
        }
    } else if mode == "probe" {
        let result = PathBuf::from(env::var_os(CHILD_RESULT).unwrap());
        let text = match Repository::open(root) {
            Ok(_) => "OPENED".into(),
            Err(error) => format!("FAILED:OPEN:{error:?}"),
        };
        fs::write(result, text).unwrap();
    } else if mode == "write" {
        let result = PathBuf::from(env::var_os(CHILD_RESULT).unwrap());
        let index = env::var(CHILD_INDEX).unwrap();
        let text = match Repository::open(root) {
            Ok(mut repository) => {
                match repository
                    .metadata_mut()
                    .start_operation(OperationEnvelope {
                        operation_id: format!("process-operation-{index}"),
                        project_id: "process-project".into(),
                        request_id: format!("process-request-{index}"),
                        agent_id: "process-agent".into(),
                        session_id: format!("process-session-{index}"),
                        workspace_id: None,
                        environment_id: None,
                        parent_operation_id: None,
                        schema_version: "0.1".into(),
                        started_at: "t1".into(),
                        tool: "runtime".into(),
                        action: "concurrent.write".into(),
                        input_refs: Vec::new(),
                        output_refs: Vec::new(),
                        resource: None,
                        before_state: None,
                        after_state: None,
                        reversibility: "UNKNOWN".into(),
                        replayability: "REPLAYABLE".into(),
                        side_effect: "EXTERNAL".into(),
                        policy_decision: None,
                    }) {
                    Ok(_) => "WRITTEN".into(),
                    Err(error) => format!("FAILED:WRITE:{error:?}"),
                }
            }
            Err(error) => format!("FAILED:OPEN:{error:?}"),
        };
        fs::write(result, text).unwrap();
    } else {
        let result = PathBuf::from(env::var_os(CHILD_RESULT).unwrap());
        let workspace_id = env::var(CHILD_WORKSPACE).unwrap();
        let agent_id = env::var(CHILD_AGENT).unwrap();
        let lease = LeaseToken {
            workspace_id: workspace_id.clone(),
            agent_id,
            epoch: env::var(CHILD_EPOCH).unwrap().parse().unwrap(),
            expires_at_ms: env::var(CHILD_EXPIRY).unwrap().parse().unwrap(),
        };
        let text =
            match Repository::open(root) {
                Ok(mut repository) => {
                    match WorkspaceManager::new(&mut repository, Redactor::default())
                        .snapshot_local(&workspace_id, &lease, SnapshotOptions::default(), 20, "t2")
                    {
                        Ok(snapshot) => format!("WRITTEN:{}", snapshot.snapshot_id),
                        Err(error) => format!("FAILED:SNAPSHOT:{error:?}"),
                    }
                }
                Err(error) => format!("FAILED:OPEN:{error:?}"),
            };
        fs::write(result, text).unwrap();
    }
}

#[cfg(feature = "direct-access-stress")]
#[test]
fn real_multi_process_open_is_supported_or_fails_closed_without_corruption() {
    let root = tempdir().unwrap();
    Repository::init(root.path()).unwrap();
    let ready = root.path().join("holder.ready");
    let stop = root.path().join("holder.stop");
    let result = root.path().join("probe.result");
    let executable = env::current_exe().unwrap();

    let mut holder = WaitedChild(
        child(&executable, root.path(), "hold")
            .env(CHILD_READY, &ready)
            .env(CHILD_STOP, &stop)
            .spawn()
            .unwrap(),
    );
    wait_for(&ready);
    let status = child(&executable, root.path(), "probe")
        .env(CHILD_RESULT, &result)
        .status()
        .unwrap();
    assert!(status.success());
    let observed = fs::read_to_string(&result).unwrap();

    fs::write(&stop, b"stop").unwrap();
    assert!(holder.wait().unwrap().success());
    Repository::open(root.path()).expect("repository remains healthy after process contention");

    if cfg!(windows) && observed != "OPENED" {
        assert!(
            observed.contains("code: 33") || observed.contains("sharing violation"),
            "unexpected Windows process-open failure: {observed}"
        );
    } else {
        assert_eq!(observed, "OPENED");
    }
    eprintln!("multi-process observation: {observed}");
}

#[cfg(feature = "direct-access-stress")]
#[test]
fn real_multi_process_writers_are_serialized_or_fail_closed() {
    let root = tempdir().unwrap();
    let mut repository = Repository::init(root.path()).unwrap();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: "process-agent".into(),
            provider: "runtime-neutral".into(),
            display_name: None,
            created_at: "t0".into(),
        })
        .unwrap();
    drop(repository);

    let executable = env::current_exe().unwrap();
    let mut children = Vec::new();
    for index in 0..4 {
        let result = root.path().join(format!("writer-{index}.result"));
        let child = WaitedChild(
            child(&executable, root.path(), "write")
                .env(CHILD_RESULT, &result)
                .env(CHILD_INDEX, index.to_string())
                .spawn()
                .unwrap(),
        );
        children.push((index, result, child));
    }

    let mut written = HashSet::new();
    let mut failed = Vec::new();
    for (index, result, mut child) in children {
        assert!(child.wait().unwrap().success());
        let observed = fs::read_to_string(result).unwrap();
        if observed == "WRITTEN" {
            written.insert(index);
        } else {
            failed.push(observed.clone());
            assert!(
                known_windows_contention(&observed),
                "unexpected writer {index} failure: {observed}"
            );
        }
    }

    let reopened = Repository::open(root.path()).unwrap();
    for index in 0..4 {
        let exists = reopened
            .metadata()
            .operation_record(&format!("process-operation-{index}"))
            .unwrap()
            .is_some();
        assert_eq!(exists, written.contains(&index));
    }
    assert!(
        !written.is_empty(),
        "at least one writer must make durable progress"
    );
    eprintln!("multi-process writers: written={written:?}, failed={failed:?}");
}

#[cfg(feature = "direct-access-stress")]
#[test]
fn real_multi_process_workspace_publications_are_atomic_or_fail_closed() {
    let root = tempdir().unwrap();
    let workspace_parent = tempdir().unwrap();
    let mut repository = Repository::init(root.path()).unwrap();
    repository
        .metadata_mut()
        .record_environment(
            "process-env",
            "process-project",
            &json!({"schema_version": 1}),
            "t0",
        )
        .unwrap();
    let mut leases = Vec::new();
    for index in 0..3 {
        let agent_id = format!("workspace-agent-{index}");
        let workspace_id = format!("process-workspace-{index}");
        repository
            .metadata_mut()
            .create_agent_identity(&AgentIdentity {
                agent_id: agent_id.clone(),
                provider: "runtime-neutral".into(),
                display_name: None,
                created_at: "t0".into(),
            })
            .unwrap();
        let path = workspace_parent.path().join(&workspace_id);
        WorkspaceManager::new(&mut repository, Redactor::default())
            .create_local(
                &workspace_id,
                "process-project",
                &path,
                None,
                Some("process-env"),
                "t0",
            )
            .unwrap();
        fs::write(path.join("state.txt"), format!("runtime-{index}")).unwrap();
        let lease = WorkspaceManager::new(&mut repository, Redactor::default())
            .acquire_lease(&workspace_id, &agent_id, 10, 100_000)
            .unwrap();
        leases.push((index, workspace_id, agent_id, lease));
    }
    drop(repository);

    let executable = env::current_exe().unwrap();
    let mut children = Vec::new();
    for (index, workspace_id, agent_id, lease) in leases {
        let result = root.path().join(format!("workspace-writer-{index}.result"));
        let child = WaitedChild(
            child(&executable, root.path(), "workspace")
                .env(CHILD_RESULT, &result)
                .env(CHILD_WORKSPACE, &workspace_id)
                .env(CHILD_AGENT, &agent_id)
                .env(CHILD_EPOCH, lease.epoch.to_string())
                .env(CHILD_EXPIRY, lease.expires_at_ms.to_string())
                .spawn()
                .unwrap(),
        );
        children.push((index, workspace_id, result, child));
    }

    let mut written = HashSet::new();
    let mut failed = Vec::new();
    let mut unexpected = Vec::new();
    for (index, workspace_id, result, mut child) in children {
        assert!(child.wait().unwrap().success());
        let observed = fs::read_to_string(result).unwrap();
        if observed.starts_with("WRITTEN:") {
            written.insert(workspace_id);
        } else {
            failed.push(observed.clone());
            if !known_windows_contention(&observed) {
                unexpected.push((index, observed));
            }
        }
    }

    let reopened = Repository::open(root.path()).unwrap();
    for index in 0..3 {
        let workspace_id = format!("process-workspace-{index}");
        let workspace = reopened
            .metadata()
            .workspace(&workspace_id)
            .unwrap()
            .unwrap();
        assert_eq!(workspace.head.is_some(), written.contains(&workspace_id));
    }
    assert!(
        !written.is_empty(),
        "at least one workspace writer must make progress"
    );
    assert!(
        unexpected.is_empty(),
        "unexpected workspace writer failures after successful cold-reopen validation: {unexpected:?}"
    );
    eprintln!("multi-process workspace writers: written={written:?}, failed={failed:?}");
}

#[test]
fn direct_workspace_writers_are_rejected_by_core_owner_without_corruption() {
    let mut fixture = fixture();
    let root = fixture._repository_dir.path().to_path_buf();
    let lease = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .acquire_lease("workspace", "agent-a", 10, 100_000)
        .unwrap();
    fs::write(
        fixture._workspace_parent.path().join("workspace/state.txt"),
        "baseline",
    )
    .unwrap();
    let baseline = AgentControl::new(&mut fixture.repository)
        .publish_version(PublishVersionRequest {
            workspace_id: "workspace".into(),
            lease: lease.clone(),
            expected_workspace_revision: 0,
            now_ms: 20,
            created_at: "t1".into(),
            operation_id: "baseline-operation".into(),
            request_id: "baseline-request".into(),
            session_id: Some("baseline-session".into()),
            tool: Some("test".into()),
            parent_version_id: None,
            update_version_head: true,
        })
        .unwrap();
    drop(fixture.repository);

    let owner = Repository::open_as_core_owner(&root).unwrap();
    let result_dir = tempdir().unwrap();
    let executable = env::current_exe().unwrap();
    let mut children = Vec::new();
    for index in 0..3 {
        let result = result_dir.path().join(format!("denied-{index}.result"));
        let child = WaitedChild(
            child(&executable, &root, "workspace")
                .env(CHILD_RESULT, &result)
                .env(CHILD_WORKSPACE, "workspace")
                .env(CHILD_AGENT, "agent-a")
                .env(CHILD_EPOCH, lease.epoch.to_string())
                .env(CHILD_EXPIRY, lease.expires_at_ms.to_string())
                .spawn()
                .unwrap(),
        );
        children.push((result, child));
    }
    for (result, mut child) in children {
        assert!(child.wait().unwrap().success());
        let observed = fs::read_to_string(result).unwrap();
        assert!(
            observed.starts_with("FAILED:OPEN:Conflict("),
            "direct writer bypassed active Core owner: {observed}"
        );
    }
    drop(owner);

    let reopened = Repository::open(&root).expect("cold reopen after rejected direct writers");
    let workspace = reopened.metadata().workspace("workspace").unwrap().unwrap();
    let version = reopened
        .metadata()
        .version_record(&baseline.version.version_id)
        .unwrap()
        .unwrap();
    let snapshot = reopened
        .metadata()
        .snapshot_record(&baseline.snapshot.snapshot_id)
        .unwrap()
        .unwrap();
    let operation = reopened
        .metadata()
        .operation_record("baseline-operation")
        .unwrap()
        .unwrap();
    assert_eq!(
        workspace.head.as_deref(),
        Some(snapshot.root_digest.as_str())
    );
    assert_eq!(
        workspace.version_head_id.as_deref(),
        Some(version.version_id.as_str())
    );
    assert_eq!(version.snapshot_id, snapshot.snapshot_id);
    assert_eq!(operation.lifecycle_status, "completed");
    assert_eq!(
        reopened
            .metadata()
            .list_versions("workspace")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(workspace.revision, baseline.workspace.revision);
    assert_eq!(
        fs::read_to_string(fixture._workspace_parent.path().join("workspace/state.txt")).unwrap(),
        "baseline"
    );
}

fn child(executable: &Path, root: &Path, mode: &str) -> Command {
    let mut command = Command::new(executable);
    command
        .arg("--exact")
        .arg("multi_process_child")
        .arg("--nocapture")
        .env(CHILD_MODE, mode)
        .env(CHILD_ROOT, root);
    command
}

#[cfg(feature = "direct-access-stress")]
fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "child process did not become ready"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(feature = "direct-access-stress")]
fn known_windows_contention(observed: &str) -> bool {
    cfg!(windows)
        && ((observed.starts_with("FAILED:OPEN:") || observed.starts_with("FAILED:SNAPSHOT:"))
            && (observed.contains("code: 33") || observed.contains("sharing violation"))
            || observed.starts_with("FAILED:WRITE:") && observed.contains("database is locked"))
}
