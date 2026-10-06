use pong_core::metadata::{
    AgentIdentity, CheckpointCreation, ExecutionCreation, OperationEnvelope, OperationRef,
    TaskCreation, VersionPublication,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::Repository;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "project-recovery-cli";
const AGENT: &str = "agent-recovery-cli";
const TASK: &str = "task-recovery-cli";
const EXECUTION: &str = "execution-recovery-cli";
const WORKSPACE: &str = "workspace-recovery-cli";
const ENVIRONMENT: &str = "environment-recovery-cli";
const CHECKPOINT: &str = "checkpoint-recovery-cli";

struct Fixture {
    root: TempDir,
    _workspace_parent: TempDir,
}

fn fixture() -> Fixture {
    let root = tempdir().unwrap();
    let workspace_parent = tempdir().unwrap();
    let workspace_path = workspace_parent.path().join("workspace");
    let mut repository = Repository::init(root.path()).unwrap();
    repository
        .metadata_mut()
        .record_environment(ENVIRONMENT, PROJECT, &json!({"os":"test"}), "t0")
        .unwrap();
    repository
        .metadata_mut()
        .create_agent_identity(&AgentIdentity {
            agent_id: AGENT.into(),
            provider: "test".into(),
            display_name: Some("Recovery Test Agent".into()),
            created_at: "t0".into(),
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: TASK.into(),
            project_id: PROJECT.into(),
            goal: "recovery CLI test".into(),
            context_ref: None,
            created_at: "t0".into(),
        })
        .unwrap();
    WorkspaceManager::new(&mut repository, Redactor::default())
        .create_local(
            WORKSPACE,
            PROJECT,
            &workspace_path,
            None,
            Some(ENVIRONMENT),
            "t1",
        )
        .unwrap();
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
        .unwrap();
    fs::write(workspace_path.join("state.txt"), "recovery").unwrap();
    let now_ms = unix_millis() as i64;
    let lease = WorkspaceManager::new(&mut repository, Redactor::default())
        .acquire_lease(WORKSPACE, AGENT, now_ms, 60_000)
        .unwrap();
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local(WORKSPACE, &lease, SnapshotOptions::default(), now_ms, "t2")
        .unwrap();
    repository
        .metadata_mut()
        .start_operation(OperationEnvelope {
            operation_id: "operation:recovery-cli:version".into(),
            project_id: PROJECT.into(),
            request_id: "request:recovery-cli:version".into(),
            agent_id: AGENT.into(),
            session_id: "session:recovery-cli".into(),
            workspace_id: Some(WORKSPACE.into()),
            environment_id: Some(ENVIRONMENT.into()),
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
        .unwrap();
    let version = repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: WORKSPACE.into(),
            project_id: PROJECT.into(),
            snapshot_id: snapshot.snapshot_id,
            creation_operation_id: "operation:recovery-cli:version".into(),
            environment_id: Some(ENVIRONMENT.into()),
            created_at: "t3".into(),
            parent_version_id: None,
        })
        .unwrap();
    repository
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: CHECKPOINT.into(),
            task_id: TASK.into(),
            execution_id: EXECUTION.into(),
            workspace_id: WORKSPACE.into(),
            version_id: version.version_id,
            operation_id: Some("operation:recovery-cli:version".into()),
            reason: "test recovery point".into(),
            actor_agent_id: AGENT.into(),
            request_id: "request:recovery-cli:checkpoint".into(),
            created_at: "t4".into(),
        })
        .unwrap();
    drop(repository);
    Fixture {
        root,
        _workspace_parent: workspace_parent,
    }
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pong"))
        .args(args)
        .output()
        .unwrap()
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

#[test]
fn help_discloses_recovery_commands() {
    let root = run(&["--help"]);
    assert!(root.status.success());
    assert!(String::from_utf8_lossy(&root.stdout).contains("recovery"));
    let recovery = run(&["recovery", "--help"]);
    assert!(recovery.status.success());
    let help = String::from_utf8_lossy(&recovery.stdout);
    assert!(help.contains("inspect"));
    assert!(help.contains("resume"));
}

#[test]
fn inspect_and_resume_use_durable_records() {
    let fixture = fixture();
    let repository = fixture.root.path().to_string_lossy().to_string();
    let inspect = run(&[
        "recovery",
        "inspect",
        "--repository",
        &repository,
        "--checkpoint",
        CHECKPOINT,
        "--json",
    ]);
    assert!(
        inspect.status.success(),
        "{}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(value["checkpoint"]["id"], CHECKPOINT);
    assert_eq!(value["execution"]["id"], EXECUTION);
    assert_eq!(value["workspace"]["id"], WORKSPACE);
    assert!(value["version"]["id"].as_str().unwrap().starts_with("ver-"));
    assert!(value["snapshot"]["id"]
        .as_str()
        .unwrap()
        .starts_with("snp-"));

    let resume = run(&[
        "recovery",
        "resume",
        "--repository",
        &repository,
        "--checkpoint",
        CHECKPOINT,
        "--json",
    ]);
    assert!(
        resume.status.success(),
        "{}",
        String::from_utf8_lossy(&resume.stderr)
    );
    let resumed: serde_json::Value = serde_json::from_slice(&resume.stdout).unwrap();
    assert_eq!(resumed["checkpoint_id"], CHECKPOINT);
    assert_eq!(resumed["original_execution_id"], EXECUTION);
    assert_eq!(resumed["resume_status"], "created");
}

#[test]
fn missing_checkpoint_and_repository_are_nonzero() {
    let fixture = fixture();
    let repository = fixture.root.path().to_string_lossy().to_string();
    let missing = run(&[
        "recovery",
        "inspect",
        "--repository",
        &repository,
        "--checkpoint",
        "missing",
    ]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("NOT_FOUND"));
    let invalid = run(&[
        "recovery",
        "inspect",
        "--repository",
        "C:\\does-not-exist\\pong",
        "--checkpoint",
        CHECKPOINT,
    ]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("NOT_FOUND"));
}

#[allow(dead_code)]
fn _path_exists(path: &Path) -> bool {
    path.exists()
}
