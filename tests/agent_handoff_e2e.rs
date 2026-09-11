//! Real-agent integration validation: Codex -> Pong -> Checkpoint ->
//! Handoff -> Claude Code -> Resume.
//!
//! The two provider CLIs are intentionally invoked as external processes.
//! Pong remains responsible for all durable state and the providers only edit
//! the target workspace filesystem.

use pong_core::metadata::{
    AgentIdentity, CheckpointCreation, ExecutionCreation, HandoffCreation, OperationEnvelope,
    OperationRef, ResumeCreation, TaskCreation, VersionPublication,
};
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::Repository;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use tempfile::{tempdir, TempDir};

const PROJECT: &str = "project-real-agent-handoff";
const ENVIRONMENT: &str = "env-real-agent-handoff";
const TASK: &str = "task-real-agent-handoff";
const CODEX: &str = "agent-codex-real";
const CLAUDE: &str = "agent-claude-real";
const W1: &str = "workspace-codex-real";
const W2: &str = "workspace-claude-real";

struct Fixture {
    pong_dir: TempDir,
    _workspace_dir: TempDir,
    repository: Repository,
    w1_path: PathBuf,
    w2_path: PathBuf,
}

struct ProviderRun {
    provider: &'static str,
    command: String,
    exit_code: Option<i32>,
    duration_ms: u128,
    stdout: String,
    stderr: String,
}

fn fixture() -> Fixture {
    let pong_dir = tempdir().expect("Pong repository directory");
    let workspace_dir = tempdir().expect("workspace parent directory");
    let mut repository = Repository::init(pong_dir.path()).expect("initialize Pong repository");
    repository
        .metadata_mut()
        .record_environment(
            ENVIRONMENT,
            PROJECT,
            &json!({"schema_version": 1, "os": "windows", "integration": "real-agent"}),
            "2026-09-11T00:00:00Z",
        )
        .expect("record environment");
    for (agent_id, provider, display_name) in [
        (CODEX, "codex", "Codex"),
        (CLAUDE, "claude-code", "Claude Code"),
    ] {
        repository
            .metadata_mut()
            .create_agent_identity(&AgentIdentity {
                agent_id: agent_id.into(),
                provider: provider.into(),
                display_name: Some(display_name.into()),
                created_at: "2026-09-11T00:00:00Z".into(),
            })
            .expect("create agent identity");
    }
    repository
        .metadata_mut()
        .create_task(&TaskCreation {
            task_id: TASK.into(),
            project_id: PROJECT.into(),
            goal: "continue calculator work across an agent handoff".into(),
            context_ref: Some("opaque://real-agent-handoff".into()),
            created_at: "2026-09-11T00:01:00Z".into(),
        })
        .expect("create task");
    Fixture {
        w1_path: workspace_dir.path().join("codex-workspace"),
        w2_path: workspace_dir.path().join("claude-workspace"),
        pong_dir,
        _workspace_dir: workspace_dir,
        repository,
    }
}

fn create_workspace(repository: &mut Repository, id: &str, path: &Path) {
    WorkspaceManager::new(repository, Redactor::default())
        .create_local(
            id,
            PROJECT,
            path,
            None,
            Some(ENVIRONMENT),
            "2026-09-11T00:01:00Z",
        )
        .expect("create workspace");
}

fn run_provider(
    provider: &'static str,
    program: &str,
    args: &[String],
    path: &Path,
) -> ProviderRun {
    let command = format!("{} {}", program, args.join(" "));
    let started = Instant::now();
    let output = Command::new(program)
        .args(args)
        .current_dir(path)
        .output()
        .unwrap_or_else(|error| panic!("{provider} executable unavailable: {error}"));
    let run = ProviderRun {
        provider,
        command,
        exit_code: output.status.code(),
        duration_ms: started.elapsed().as_millis(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };
    assert!(
        output.status.success(),
        "{provider} failed ({:?})\nstdout:\n{}\nstderr:\n{}",
        run.exit_code,
        run.stdout,
        run.stderr
    );
    run
}

fn run_codex(path: &Path) -> ProviderRun {
    let prompt = "Use the shell tool now, in the current workspace. Run this exact PowerShell script: New-Item -ItemType Directory -Force src | Out-Null; [IO.File]::WriteAllText((Join-Path (Get-Location) 'src/calculator.rs'), \"// Added by Codex`r`n pub fn add(a: i32, b: i32) -> i32 { a + b }`r`n pub fn multiply(a: i32, b: i32) -> i32 { a * b }`r`n\", [Text.UTF8Encoding]::new($false)); [IO.File]::WriteAllText((Join-Path (Get-Location) 'README.md'), '# Calculator', [Text.UTF8Encoding]::new($false)); Get-Item src/calculator.rs,README.md. Execute it, verify the files exist, and stop. Do not use git and do not merely describe the changes.";
    run_provider(
        "Codex",
        "codex",
        &[
            "exec".into(),
            "--dangerously-bypass-approvals-and-sandbox".into(),
            "--ephemeral".into(),
            "--skip-git-repo-check".into(),
            "-C".into(),
            path.display().to_string(),
            prompt.into(),
        ],
        path,
    )
}

fn run_claude(path: &Path) -> ProviderRun {
    let prompt = "Use the shell tool now, in the current workspace. Run this exact PowerShell script: $p = Join-Path (Get-Location) 'src/calculator.rs'; $existing = [IO.File]::ReadAllText($p); if (-not $existing.Contains('Added by Codex')) { throw 'Codex work is missing' }; [IO.File]::AppendAllText($p, \"`r`n// Added by Claude Code`r`n pub fn subtract(a: i32, b: i32) -> i32 { a - b }`r`n\", [Text.UTF8Encoding]::new($false)); Get-Item $p. Execute it, verify the file, and stop. Do not use git and do not merely describe the changes.";
    run_provider(
        "Claude Code",
        "claude",
        &[
            "-p".into(),
            "--dangerously-skip-permissions".into(),
            "--no-session-persistence".into(),
            "--permission-mode".into(),
            "bypassPermissions".into(),
            prompt.into(),
        ],
        path,
    )
}

fn start_version_operation(
    repository: &mut Repository,
    operation_id: &str,
    agent_id: &str,
    workspace_id: &str,
    snapshot_id: &str,
    timestamp: &str,
) {
    repository
        .metadata_mut()
        .start_operation(OperationEnvelope {
            operation_id: operation_id.into(),
            project_id: PROJECT.into(),
            request_id: format!("request:{operation_id}"),
            agent_id: agent_id.into(),
            session_id: format!("session:{agent_id}"),
            workspace_id: Some(workspace_id.into()),
            environment_id: Some(ENVIRONMENT.into()),
            parent_operation_id: None,
            schema_version: "0.1".into(),
            started_at: timestamp.into(),
            tool: "real-agent-e2e".into(),
            action: "version.create".into(),
            input_refs: vec![OperationRef {
                kind: "snapshot".into(),
                reference: snapshot_id.into(),
                media_type: Some("application/vnd.pong.snapshot".into()),
            }],
            output_refs: Vec::new(),
            resource: Some(json!({"provider_agent": agent_id})),
            before_state: None,
            after_state: None,
            reversibility: "REVERSIBLE".into(),
            replayability: "REPLAYABLE".into(),
            side_effect: "WORKSPACE".into(),
            policy_decision: None,
        })
        .expect("start version operation");
}

fn git_value(args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("git available");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn write_evidence(runs: &[ProviderRun], ids: &[(&str, &str)]) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("artifacts/m3-development");
    fs::create_dir_all(&directory).expect("create evidence directory");
    let json_path = directory.join("m3-real-agent-handoff-e2e-windows-native-2026-09-11.json");
    let log_path = directory.join("m3-real-agent-handoff-e2e-windows-native-2026-09-11.log");
    let provider_commands: Vec<_> = runs
        .iter()
        .map(|run| {
            json!({
                "provider": run.provider,
                "command": run.command,
                "exit_code": run.exit_code,
                "duration_ms": run.duration_ms,
            })
        })
        .collect();
    let evidence = json!({
        "mode": "LOCAL_NATIVE_DEVELOPMENT",
        "platform": {"os": "Windows 11", "arch": std::env::consts::ARCH, "filesystem": "NTFS"},
        "base_commit": git_value(&["rev-parse", "HEAD"]),
        "head": git_value(&["rev-parse", "HEAD"]),
        "branch": git_value(&["branch", "--show-current"]),
        "working_tree": "UNCOMMITTED_DEVELOPMENT_TREE",
        "durable_ids": ids.iter().map(|(kind, value)| json!({"kind": kind, "id": value})).collect::<Vec<_>>(),
        "execution_count": 2,
        "workspace_count": 2,
        "shared_base_count": 1,
        "concurrent_operation_count": 0,
        "conflict_count": 0,
        "checkpoint_discovery": "Pong metadata query after reopen",
        "provider_commands": provider_commands,
        "focused_tests": "cargo test --locked --test agent_handoff_e2e -- --ignored --nocapture",
        "full_regression": "cargo test --all --locked --no-fail-fast",
        "ignored": "reported separately by cargo",
        "recovery": "checkpoint, handoff, resume records reopened successfully",
    });
    fs::write(&json_path, serde_json::to_string_pretty(&evidence).unwrap())
        .expect("write evidence");
    let mut log = String::new();
    for run in runs {
        log.push_str(&format!(
            "[{}] exit_code={:?} duration_ms={}\ncommand={}\nstdout:\n{}\nstderr:\n{}\n",
            run.provider, run.exit_code, run.duration_ms, run.command, run.stdout, run.stderr
        ));
    }
    fs::write(log_path, log).expect("write provider log");
}

#[test]
#[ignore = "requires authenticated codex and claude CLIs; run explicitly with --ignored"]
fn real_codex_to_claude_handoff_through_pong() {
    let mut fixture = fixture();
    create_workspace(&mut fixture.repository, W1, &fixture.w1_path);
    let execution_one = fixture
        .repository
        .metadata_mut()
        .create_execution(&ExecutionCreation {
            execution_id: "execution-codex-real".into(),
            task_id: TASK.into(),
            agent_id: CODEX.into(),
            parent_execution_id: None,
            workspace_id: Some(W1.into()),
            base_version_id: None,
            current_version_id: None,
            created_at: "2026-09-11T00:02:00Z".into(),
        })
        .expect("create Codex execution");
    fixture
        .repository
        .metadata_mut()
        .start_execution(&execution_one.execution_id, 0, "2026-09-11T00:02:01Z")
        .expect("start Codex execution");
    let codex_run = run_codex(&fixture.w1_path);
    let codex_file = fs::read_to_string(fixture.w1_path.join("src/calculator.rs"))
        .expect("Codex created calculator");
    assert!(codex_file.contains("multiply"));
    assert!(codex_file.contains("Added by Codex"));

    let (snapshot_one, lease_one) = {
        let mut manager = WorkspaceManager::new(&mut fixture.repository, Redactor::default());
        let lease = manager
            .acquire_lease(W1, CODEX, 10, 1_000_000)
            .expect("acquire W1 lease");
        let snapshot = manager
            .snapshot_local(
                W1,
                &lease,
                SnapshotOptions::default(),
                20,
                "2026-09-11T00:03:00Z",
            )
            .expect("publish W1 snapshot");
        (snapshot, lease)
    };
    start_version_operation(
        &mut fixture.repository,
        "operation:codex:version-1",
        CODEX,
        W1,
        &snapshot_one.snapshot_id,
        "2026-09-11T00:03:01Z",
    );
    let version_one = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: W1.into(),
            project_id: PROJECT.into(),
            snapshot_id: snapshot_one.snapshot_id.clone(),
            creation_operation_id: "operation:codex:version-1".into(),
            environment_id: Some(ENVIRONMENT.into()),
            created_at: "2026-09-11T00:03:01Z".into(),
            parent_version_id: None,
        })
        .expect("publish W1 Version");
    let w1_revision = fixture
        .repository
        .metadata()
        .workspace(W1)
        .unwrap()
        .unwrap()
        .revision;
    fixture
        .repository
        .metadata_mut()
        .set_version_head(
            W1,
            Some(&version_one.version_id),
            &lease_one,
            w1_revision,
            "2026-09-11T00:03:02Z",
            30,
        )
        .expect("set W1 Version Head");
    let checkpoint_one = fixture
        .repository
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: "checkpoint-codex-real".into(),
            task_id: TASK.into(),
            execution_id: execution_one.execution_id.clone(),
            workspace_id: W1.into(),
            version_id: version_one.version_id.clone(),
            operation_id: Some("operation:codex:version-1".into()),
            reason: "Codex completed the first work segment".into(),
            actor_agent_id: CODEX.into(),
            request_id: "request:checkpoint:codex-real".into(),
            created_at: "2026-09-11T00:03:03Z".into(),
        })
        .expect("create C1");
    fixture
        .repository
        .metadata_mut()
        .transition_execution(
            &execution_one.execution_id,
            "interrupted",
            Some("handoff"),
            1,
            "2026-09-11T00:03:04Z",
        )
        .expect("interrupt Codex execution");

    create_workspace(&mut fixture.repository, W2, &fixture.w2_path);
    let resume = fixture
        .repository
        .metadata_mut()
        .resume_from_checkpoint(&ResumeCreation {
            execution_id: "execution-claude-real".into(),
            task_id: TASK.into(),
            agent_id: CLAUDE.into(),
            parent_execution_id: Some(execution_one.execution_id.clone()),
            workspace_id: Some(W2.into()),
            source_version_id: None,
            checkpoint_id: Some(checkpoint_one.checkpoint_id.clone()),
            request_id: "request:resume:claude-real".into(),
            created_at: "2026-09-11T00:04:00Z".into(),
        })
        .expect("resume from C1");
    let handoff = fixture
        .repository
        .metadata_mut()
        .create_handoff(&HandoffCreation {
            handoff_id: "handoff-codex-to-claude-real".into(),
            task_id: TASK.into(),
            from_execution_id: execution_one.execution_id.clone(),
            to_execution_id: resume.execution_id.clone(),
            source_version_id: Some(version_one.version_id.clone()),
            checkpoint_id: Some(checkpoint_one.checkpoint_id.clone()),
            reason: "Codex interrupted; Claude Code continues from C1".into(),
            actor_agent_id: CODEX.into(),
            requester_execution_id: Some(execution_one.execution_id.clone()),
            request_id: "request:handoff:codex-claude-real".into(),
            created_at: "2026-09-11T00:04:01Z".into(),
        })
        .expect("create handoff");

    let target_revision = fixture
        .repository
        .metadata()
        .workspace(W2)
        .unwrap()
        .unwrap()
        .revision;
    let lease_two = {
        let mut manager = WorkspaceManager::new(&mut fixture.repository, Redactor::default());
        let lease = manager
            .acquire_lease(W2, CLAUDE, 40, 1_000_000)
            .expect("acquire W2 lease");
        manager
            .restore_from_version(
                W2,
                &version_one.version_id,
                &lease,
                target_revision,
                50,
                "2026-09-11T00:05:00Z",
            )
            .expect("materialize C1 into W2");
        lease
    };
    assert_eq!(target_revision, 0);
    fixture
        .repository
        .metadata_mut()
        .start_execution(&resume.execution_id, 0, "2026-09-11T00:05:01Z")
        .expect("start Claude execution");
    let claude_run = run_claude(&fixture.w2_path);
    let continued = fs::read_to_string(fixture.w2_path.join("src/calculator.rs"))
        .expect("Claude read materialized calculator");
    assert!(continued.contains("multiply"));
    assert!(continued.contains("subtract"));
    assert!(continued.contains("Added by Codex"));
    assert!(continued.contains("Added by Claude Code"));

    let snapshot_two = {
        let mut manager = WorkspaceManager::new(&mut fixture.repository, Redactor::default());
        manager
            .snapshot_local(
                W2,
                &lease_two,
                SnapshotOptions::default(),
                60,
                "2026-09-11T00:06:00Z",
            )
            .expect("publish W2 snapshot")
    };
    start_version_operation(
        &mut fixture.repository,
        "operation:claude:version-2",
        CLAUDE,
        W2,
        &snapshot_two.snapshot_id,
        "2026-09-11T00:06:01Z",
    );
    let version_two = fixture
        .repository
        .metadata_mut()
        .create_version(VersionPublication {
            workspace_id: W2.into(),
            project_id: PROJECT.into(),
            snapshot_id: snapshot_two.snapshot_id,
            creation_operation_id: "operation:claude:version-2".into(),
            environment_id: Some(ENVIRONMENT.into()),
            created_at: "2026-09-11T00:06:01Z".into(),
            parent_version_id: None,
        })
        .expect("publish W2 Version");
    let w2_revision = fixture
        .repository
        .metadata()
        .workspace(W2)
        .unwrap()
        .unwrap()
        .revision;
    fixture
        .repository
        .metadata_mut()
        .set_version_head(
            W2,
            Some(&version_two.version_id),
            &lease_two,
            w2_revision,
            "2026-09-11T00:06:02Z",
            70,
        )
        .expect("set W2 Version Head");
    let checkpoint_two = fixture
        .repository
        .metadata_mut()
        .create_checkpoint(&CheckpointCreation {
            checkpoint_id: "checkpoint-claude-real".into(),
            task_id: TASK.into(),
            execution_id: resume.execution_id.clone(),
            workspace_id: W2.into(),
            version_id: version_two.version_id.clone(),
            operation_id: Some("operation:claude:version-2".into()),
            reason: "Claude Code completed the continuation segment".into(),
            actor_agent_id: CLAUDE.into(),
            request_id: "request:checkpoint:claude-real".into(),
            created_at: "2026-09-11T00:06:03Z".into(),
        })
        .expect("create C2");
    fixture
        .repository
        .metadata_mut()
        .transition_execution(
            &resume.execution_id,
            "completed",
            Some("handoff continuation complete"),
            1,
            "2026-09-11T00:06:04Z",
        )
        .expect("complete Claude execution");

    let w1 = fixture
        .repository
        .metadata()
        .workspace(W1)
        .unwrap()
        .unwrap();
    let w2 = fixture
        .repository
        .metadata()
        .workspace(W2)
        .unwrap()
        .unwrap();
    assert_ne!(w1.locator, w2.locator);
    assert_ne!(w1.head, w2.head);
    assert_ne!(w1.revision, w2.revision);
    assert_eq!(
        w1.version_head_id.as_deref(),
        Some(version_one.version_id.as_str())
    );
    assert_eq!(
        w2.version_head_id.as_deref(),
        Some(version_two.version_id.as_str())
    );
    assert_eq!(version_one.workspace_id, W1);
    assert_eq!(version_two.workspace_id, W2);
    assert_eq!(version_two.parent_version_id, None);
    let w1_file = fs::read_to_string(fixture.w1_path.join("src/calculator.rs")).unwrap();
    assert!(w1_file.contains("multiply"));
    assert!(!w1_file.contains("subtract"));

    let pong_dir = fixture.pong_dir.path().to_path_buf();
    let ids = [
        ("task", TASK),
        ("execution_codex", execution_one.execution_id.as_str()),
        ("execution_claude", resume.execution_id.as_str()),
        ("version_codex", version_one.version_id.as_str()),
        ("version_claude", version_two.version_id.as_str()),
        ("checkpoint_codex", checkpoint_one.checkpoint_id.as_str()),
        ("checkpoint_claude", checkpoint_two.checkpoint_id.as_str()),
        ("handoff", handoff.handoff_id.as_str()),
        ("resume", resume.execution_id.as_str()),
    ];
    drop(fixture.repository);
    let reopened = Repository::open(&pong_dir).expect("reopen Pong repository");
    assert!(reopened
        .metadata()
        .checkpoint(checkpoint_one.checkpoint_id.as_str())
        .unwrap()
        .is_some());
    assert!(reopened
        .metadata()
        .checkpoint(checkpoint_two.checkpoint_id.as_str())
        .unwrap()
        .is_some());
    assert!(reopened
        .metadata()
        .handoff(handoff.handoff_id.as_str())
        .unwrap()
        .is_some());
    assert!(reopened
        .metadata()
        .resume_record(resume.execution_id.as_str())
        .unwrap()
        .is_some());
    write_evidence(&[codex_run, claude_run], &ids);
}
