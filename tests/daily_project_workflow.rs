use pong_core::metadata::{
    AgentIdentity, CheckpointCreation, ExecutionCreation, OperationEnvelope, OperationRef,
    TaskCreation, VersionPublication, WorkspaceRecord,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::Repository;
use serde_json::json;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::tempdir;

const PROJECT: &str = "project-daily";
const AGENT: &str = "agent-daily";
const TASK: &str = "task-daily";
const EXECUTION: &str = "execution-daily";
const WORKSPACE: &str = "workspace-daily";
const CHECKPOINT: &str = "checkpoint-daily";

fn run(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pong"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run pong")
}

#[test]
fn init_supports_non_git_projects_preserves_user_files_and_is_idempotent() {
    let project = tempdir().expect("project");
    let readme = project.path().join("README.md");
    fs::write(&readme, b"user content").expect("user file");

    let first = run(&["init"], project.path());
    assert!(first.status.success(), "{first:?}");
    assert!(!project.path().join(".git").exists());
    assert_eq!(fs::read(&readme).expect("read user file"), b"user content");
    let bootstrap = project.path().join(".pong/bootstrap.json");
    let before = fs::read(&bootstrap).expect("bootstrap");

    let second = run(&["init", &project.path().to_string_lossy()], project.path());
    assert!(second.status.success(), "{second:?}");
    assert_eq!(fs::read(bootstrap).expect("bootstrap"), before);
    assert_eq!(fs::read(readme).expect("read user file"), b"user content");
}

#[test]
fn status_reports_uninitialized_and_empty_initialized_projects() {
    let project = tempdir().expect("project");
    let uninitialized = run(&["status", "--json"], project.path());
    assert!(uninitialized.status.success(), "{uninitialized:?}");
    let value: Value = serde_json::from_slice(&uninitialized.stdout).expect("status JSON");
    assert_eq!(value["initialized"], false);
    assert_eq!(value["repository"], Value::Null);

    assert!(run(&["init"], project.path()).status.success());
    let initialized = run(&["status", "--json"], project.path());
    assert!(initialized.status.success(), "{initialized:?}");
    let value: Value = serde_json::from_slice(&initialized.stdout).expect("status JSON");
    assert_eq!(value["initialized"], true);
    assert_eq!(value["projects"], serde_json::json!([]));
}

#[test]
fn help_discloses_the_daily_project_commands() {
    let project = tempdir().expect("project");
    let root = run(&["--help"], project.path());
    assert!(root.status.success());
    let root = String::from_utf8(root.stdout).expect("root help");
    assert!(root.contains("init"));
    assert!(root.contains("start"));
    assert!(root.contains("status"));
    assert!(root.contains("recovery"));

    let init = run(&["init", "--help"], project.path());
    assert!(init.status.success());
    assert!(String::from_utf8(init.stdout)
        .unwrap()
        .contains("pong init"));
    let status = run(&["status", "--help"], project.path());
    assert!(status.status.success());
    assert!(String::from_utf8(status.stdout)
        .unwrap()
        .contains("--project-root"));
}

#[test]
fn start_creates_durable_task_workspace_and_running_execution() {
    let project = tempdir().expect("project");
    let readme = project.path().join("README.md");
    fs::write(&readme, b"user content").expect("user file");
    assert!(run(&["init"], project.path()).status.success());

    let started = run(&["start", "daily agent task", "--json"], project.path());
    assert!(started.status.success(), "{started:?}");
    let report: Value = serde_json::from_slice(&started.stdout).expect("start JSON");
    assert_eq!(report["status"], "started");
    for field in [
        "project_id",
        "task_id",
        "workspace_id",
        "execution_id",
        "agent_id",
        "workspace_root",
    ] {
        assert!(
            !report[field].as_str().unwrap_or_default().is_empty(),
            "{field}"
        );
    }
    let workspace = Path::new(report["workspace_root"].as_str().unwrap());
    assert!(workspace.is_dir());
    assert!(!workspace.starts_with(project.path().join(".pong")));
    assert_eq!(fs::read(&readme).expect("read user file"), b"user content");

    let status = run(&["status", "--json"], project.path());
    assert!(status.status.success(), "{status:?}");
    let status: Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    let project_status = &status["projects"][0];
    assert_eq!(project_status["project_id"], report["project_id"]);
    assert_eq!(project_status["tasks"][0]["id"], report["task_id"]);
    assert_eq!(project_status["tasks"][0]["state"], "running");
    assert_eq!(
        project_status["active_executions"][0]["id"],
        report["execution_id"]
    );
    assert_eq!(project_status["active_executions"][0]["state"], "running");
    assert_eq!(
        project_status["active_executions"][0]["agent_id"],
        report["agent_id"]
    );

    let workspace_namespace = workspace.parent().expect("workspace namespace");
    fs::remove_dir_all(workspace_namespace).expect("cleanup workspace namespace");
}

#[test]
fn start_from_project_copies_source_and_publishes_initial_version() {
    let project = tempdir().expect("project");
    let readme = project.path().join("README.md");
    let nested = project.path().join("src/nested");
    fs::create_dir_all(&nested).expect("nested source directory");
    fs::write(&readme, b"source README").expect("source README");
    fs::write(nested.join("lib.txt"), b"nested source").expect("nested source file");
    fs::create_dir_all(project.path().join(".pong")).expect("pong directory");
    fs::write(project.path().join(".pong/control.txt"), b"control").expect("pong control file");
    assert!(run(&["init"], project.path()).status.success());
    let source_readme = fs::read(&readme).expect("source README before start");
    let source_nested = fs::read(nested.join("lib.txt")).expect("nested source before start");

    let started = run(
        &["start", "import project", "--from-project", "--json"],
        project.path(),
    );
    assert!(started.status.success(), "{started:?}");
    let report: Value = serde_json::from_slice(&started.stdout).expect("start JSON");
    assert!(report["initial_version_id"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(report["initial_snapshot_id"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));

    let workspace = Path::new(report["workspace_root"].as_str().unwrap());
    assert_eq!(
        fs::read(workspace.join("README.md")).unwrap(),
        source_readme
    );
    assert_eq!(
        fs::read(workspace.join("src/nested/lib.txt")).unwrap(),
        source_nested
    );
    assert!(!workspace.join(".pong").exists());
    assert_eq!(fs::read(&readme).unwrap(), b"source README");
    assert_eq!(fs::read(nested.join("lib.txt")).unwrap(), b"nested source");

    let status = run(&["status", "--json"], project.path());
    assert!(status.status.success(), "{status:?}");
    let status: Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    let workspace_status = &status["projects"][0]["workspaces"][0];
    assert!(workspace_status["head"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
    assert_eq!(
        workspace_status["version_head_id"],
        report["initial_version_id"]
    );
    assert_eq!(
        workspace_status["snapshot_id"],
        report["initial_snapshot_id"]
    );

    let repository = Repository::open(project.path()).expect("open repository");
    let lease = repository
        .metadata()
        .workspace_lease(report["workspace_id"].as_str().unwrap())
        .expect("read workspace lease")
        .expect("lease tombstone");
    assert!(lease.agent_id.is_none());
    assert_eq!(lease.expires_at_ms, 0);

    let workspace_namespace = workspace.parent().expect("workspace namespace");
    fs::remove_dir_all(workspace_namespace).expect("cleanup workspace namespace");
}

#[test]
fn repeated_start_creates_new_attempts_and_reuses_local_agent_identity() {
    let project = tempdir().expect("project");
    assert!(run(&["init"], project.path()).status.success());

    let first = run(&["start", "first", "--json"], project.path());
    let second = run(&["start", "second", "--json"], project.path());
    assert!(first.status.success(), "{first:?}");
    assert!(second.status.success(), "{second:?}");
    let first: Value = serde_json::from_slice(&first.stdout).expect("first JSON");
    let second: Value = serde_json::from_slice(&second.stdout).expect("second JSON");
    assert_eq!(first["agent_id"], second["agent_id"]);
    for field in ["task_id", "workspace_id", "execution_id"] {
        assert_ne!(first[field], second[field], "{field}");
    }

    let first_workspace = Path::new(first["workspace_root"].as_str().unwrap());
    let second_workspace = Path::new(second["workspace_root"].as_str().unwrap());
    let _ = fs::remove_dir_all(first_workspace);
    let _ = fs::remove_dir_all(second_workspace);
}

#[test]
fn start_requires_an_initialized_project() {
    let project = tempdir().expect("project");
    let output = run(&["start", "missing initialization"], project.path());
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("BOOTSTRAP_PONG_DIRECTORY_MISSING"),
        "{stderr}"
    );
}

#[test]
fn status_discovers_the_nearest_project_from_a_nested_directory() {
    let project = tempdir().expect("project");
    assert!(run(&["init"], project.path()).status.success());
    let nested = project.path().join("src/nested");
    fs::create_dir_all(&nested).expect("nested directory");

    let status = run(&["status", "--json"], &nested);
    assert!(status.status.success(), "{status:?}");
    let value: Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    assert_eq!(value["initialized"], true);
    assert_eq!(
        value["project_root"],
        project.path().canonicalize().unwrap().display().to_string()
    );
}

#[test]
fn status_discovers_active_execution_and_resumable_checkpoint() {
    let project = tempdir().expect("project");
    assert!(run(&["init"], project.path()).status.success());
    create_recovery_state(project.path());

    let status = run(
        &[
            "status",
            "--project-root",
            &project.path().to_string_lossy(),
            "--json",
        ],
        project.path(),
    );
    assert!(status.status.success(), "{status:?}");
    let value: Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    let project_status = &value["projects"][0];
    assert_eq!(project_status["project_id"], PROJECT);
    assert_eq!(project_status["workspaces"][0]["id"], WORKSPACE);
    assert_eq!(project_status["active_executions"][0]["id"], EXECUTION);
    assert_eq!(project_status["latest_checkpoint"]["id"], CHECKPOINT);
    assert_eq!(project_status["resumable_checkpoints"][0]["id"], CHECKPOINT);
    assert_eq!(
        project_status["resumable_checkpoints"][0]["resumable"],
        true
    );

    let human = run(&["status"], project.path());
    assert!(human.status.success(), "{human:?}");
    let human = String::from_utf8(human.stdout).expect("human status");
    assert!(human.contains(WORKSPACE));
    assert!(human.contains(EXECUTION));
    assert!(human.contains(CHECKPOINT));
}

fn create_recovery_state(root: &Path) {
    let workspace_parent = tempdir().expect("workspace parent");
    let workspace_path = workspace_parent.path().join("workspace");
    fs::create_dir(&workspace_path).expect("workspace");
    fs::write(workspace_path.join("state.txt"), b"durable").expect("workspace file");

    let mut repository = Repository::open(root).expect("repository");
    repository
        .metadata_mut()
        .record_environment("environment-daily", PROJECT, &json!({"os": "test"}), "t0")
        .expect("environment");
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: AGENT.into(),
            provider: "test".into(),
            display_name: None,
            created_at: "t0".into(),
        })
        .expect("agent");
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: TASK.into(),
            project_id: PROJECT.into(),
            goal: "daily workflow".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .expect("task");
    repository
        .metadata_mut()
        .create_workspace(&WorkspaceRecord {
            workspace_id: WORKSPACE.into(),
            project_id: PROJECT.into(),
            driver: "local".into(),
            locator: workspace_path.canonicalize().unwrap().display().to_string(),
            branch_ref: None,
            head: None,
            version_head_id: None,
            environment_id: Some("environment-daily".into()),
            status: "created".into(),
            revision: 0,
            created_at: "t0".into(),
            updated_at: "t0".into(),
        })
        .expect("workspace");
    repository
        .metadata_mut()
        .create_execution(&ExecutionCreation {
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

    let now_ms = unix_millis() as i64;
    let lease = WorkspaceManager::new(&mut repository, Redactor::default())
        .acquire_lease(WORKSPACE, AGENT, now_ms, 60_000)
        .expect("lease");
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local(WORKSPACE, &lease, SnapshotOptions::default(), now_ms, "t2")
        .expect("snapshot");
    repository
        .metadata_mut()
        .start_operation(OperationEnvelope {
            operation_id: "operation:daily:version".into(),
            project_id: PROJECT.into(),
            request_id: "request:daily:version".into(),
            agent_id: AGENT.into(),
            session_id: "session:daily".into(),
            workspace_id: Some(WORKSPACE.into()),
            environment_id: Some("environment-daily".into()),
            parent_operation_id: None,
            schema_version: "0.1".into(),
            started_at: "t2".into(),
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
        })
        .expect("operation");
    let version = repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: WORKSPACE.into(),
            project_id: PROJECT.into(),
            snapshot_id: snapshot.snapshot_id,
            creation_operation_id: "operation:daily:version".into(),
            environment_id: Some("environment-daily".into()),
            created_at: "t3".into(),
            parent_version_id: None,
        })
        .expect("version");
    repository
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: CHECKPOINT.into(),
            task_id: TASK.into(),
            execution_id: EXECUTION.into(),
            workspace_id: WORKSPACE.into(),
            version_id: version.version_id,
            operation_id: Some("operation:daily:version".into()),
            reason: "ready to resume".into(),
            actor_agent_id: AGENT.into(),
            request_id: "request:daily:checkpoint".into(),
            created_at: "t4".into(),
        })
        .expect("checkpoint");
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis()
}
