//! Human-operated M4-021 acceptance runner.
//!
//! This tool never starts or discovers a provider CLI. A user runs Codex and
//! Claude Code in separate terminals and confirms each workspace step here.

use pong_core::Repository;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Init,
    W1Ready,
    WaitCodexW1,
    CodexW1Submitted,
    C1Created,
    E1Interrupted,
    E2Created,
    HandoffReady,
    WaitClaudeW2,
    ClaudeW2Submitted,
    C2Created,
    E2Completed,
    FreshProcessCheck,
    ColdReopenCheck,
    Pass,
}

impl State {
    fn advance(self, next: State) -> Result<State, String> {
        let valid = matches!(
            (self, next),
            (Self::Init, Self::W1Ready)
                | (Self::W1Ready, Self::WaitCodexW1)
                | (Self::WaitCodexW1, Self::CodexW1Submitted)
                | (Self::CodexW1Submitted, Self::C1Created)
                | (Self::C1Created, Self::E1Interrupted)
                | (Self::E1Interrupted, Self::E2Created)
                | (Self::E2Created, Self::HandoffReady)
                | (Self::HandoffReady, Self::WaitClaudeW2)
                | (Self::WaitClaudeW2, Self::ClaudeW2Submitted)
                | (Self::ClaudeW2Submitted, Self::C2Created)
                | (Self::C2Created, Self::E2Completed)
                | (Self::E2Completed, Self::FreshProcessCheck)
                | (Self::FreshProcessCheck, Self::ColdReopenCheck)
                | (Self::ColdReopenCheck, Self::Pass)
        );
        valid
            .then_some(next)
            .ok_or_else(|| format!("invalid M4-021 operator transition: {self:?} -> {next:?}"))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct HandoffManifest {
    slice: String,
    task_id: String,
    source_workspace_id: String,
    target_workspace_id: String,
    e1: String,
    e2: String,
    c1: String,
    handoff_id: String,
    source_version_id: String,
    source_snapshot_id: String,
    source_root_digest: String,
    target_workspace_path: String,
    instructions: String,
}

struct Operator {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    w1_path: PathBuf,
    w2_path: PathBuf,
    state: State,
}

impl Operator {
    fn start(repository_root: PathBuf, workspace_root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&workspace_root).map_err(|e| e.to_string())?;
        let w1_path = workspace_root.join("codex");
        let w2_path = workspace_root.join("claude");
        let mut child = Command::new(protocol_binary()?)
            .args([
                "--repository",
                repository_root.to_str().unwrap(),
                "--workspace-root",
                workspace_root.to_str().unwrap(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("start pong-agent-protocol: {e}"))?;
        let stdin = child.stdin.take().ok_or("protocol stdin unavailable")?;
        let stdout = child.stdout.take().ok_or("protocol stdout unavailable")?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            w1_path,
            w2_path,
            state: State::Init,
        })
    }

    fn call(
        &mut self,
        caller: Option<&str>,
        request_id: &str,
        operation_id: Option<&str>,
        operation: &str,
        payload: Option<Value>,
    ) -> Result<Value, String> {
        let mut request = json!({
            "protocol_version": "1.0",
            "request_id": request_id,
            "caller_agent_id": caller,
            "issued_at": format!("operator:{request_id}"),
            "operation_id": operation_id,
            "operation": operation
        });
        if let Some(payload) = payload {
            request["payload"] = payload;
        }
        serde_json::to_writer(&mut self.stdin, &request).map_err(|e| e.to_string())?;
        self.stdin.write_all(b"\n").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            return Err(format!("protocol exited before response to {operation}"));
        }
        let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if response["status"] != "ok" {
            return Err(format!("{operation} failed: {response}"));
        }
        Ok(response["result"]["data"].clone())
    }

    fn confirm(&self, message: &str) -> Result<(), String> {
        println!("\n{message}");
        println!("Type DONE and press Enter to continue.");
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| e.to_string())?;
        (input.trim() == "DONE")
            .then_some(())
            .ok_or_else(|| "operator confirmation must be DONE".into())
    }

    fn shutdown(mut self) -> Result<(), String> {
        drop(self.stdin);
        let status = self.child.wait().map_err(|e| e.to_string())?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| format!("protocol exit: {status}"))
    }
}

fn protocol_binary() -> Result<PathBuf, String> {
    let current = env::current_exe().map_err(|e| e.to_string())?;
    let name = if cfg!(windows) {
        "pong-agent-protocol.exe"
    } else {
        "pong-agent-protocol"
    };
    let mut candidates = Vec::new();
    for directory in current.ancestors().skip(1) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
        candidates.push(candidate);
    }
    Err(format!(
        "protocol binary not found; checked: {}",
        candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join(format!("pong-m4-021-operator-{}", timestamp())));
    let repository_root = root.join("repository");
    let workspace_root = root.join("workspaces");
    fs::create_dir_all(&root)?;
    let mut repository = Repository::init(&repository_root)?;
    repository.metadata_mut().record_environment(
        "environment-m4-real-protocol",
        "project-m4-real-protocol",
        &json!({"schema_version": 1, "integration": "m4-021-operator"}),
        "operator",
    )?;
    drop(repository);

    let mut op = Operator::start(repository_root.clone(), workspace_root.clone())?;
    op.call(Some("agent-m4-real-codex"), "hello", None, "hello", None)?;
    op.call(Some("agent-m4-real-codex"), "register-codex", None, "register_agent", Some(json!({"agent_id":"agent-m4-real-codex","provider_metadata":"codex-cli","display_name":null})))?;
    op.call(Some("agent-m4-real-claude"), "register-claude", None, "register_agent", Some(json!({"agent_id":"agent-m4-real-claude","provider_metadata":"claude-code-cli","display_name":null})))?;
    op.call(Some("agent-m4-real-codex"), "create-task", None, "create_task", Some(json!({"task_id":"task-m4-real-protocol-handoff","project_id":"project-m4-real-protocol","goal_ref":"continue calculator work across real Agent runtimes","context_ref":"opaque:m4-real-agent-protocol"})))?;
    for (agent, id, binding) in [
        ("agent-m4-real-codex", "workspace-m4-real-codex", "codex"),
        ("agent-m4-real-claude", "workspace-m4-real-claude", "claude"),
    ] {
        op.call(Some(agent), &format!("create-{id}"), None, "create_workspace", Some(json!({"workspace_id":id,"project_id":"project-m4-real-protocol","binding_ref":binding,"branch_ref":null,"environment_id":"environment-m4-real-protocol"})))?;
    }
    op.call(Some("agent-m4-real-codex"), "create-e1", None, "create_execution", Some(json!({"execution_id":"execution-m4-real-codex","task_id":"task-m4-real-protocol-handoff","parent_execution_id":null,"workspace_id":"workspace-m4-real-codex","base_version_id":null})))?;
    op.call(
        Some("agent-m4-real-codex"),
        "start-e1",
        None,
        "start_execution",
        Some(json!({"execution_id":"execution-m4-real-codex","expected_revision":0})),
    )?;
    let lease_a = op.call(Some("agent-m4-real-codex"), "lease-e1", None, "acquire_workspace_lease", Some(json!({"execution_id":"execution-m4-real-codex","workspace_id":"workspace-m4-real-codex","ttl_ms":1_000_000})))?["authority"].clone();
    op.state = op.state.advance(State::W1Ready)?;
    println!("\nCODEX STEP\nWorkspace: {}\nRequired change: create src/calculator.rs and README.md.\nExpected markers: Added by Codex, add, multiply\n", op.w1_path.display());
    op.state = op.state.advance(State::WaitCodexW1)?;
    op.confirm("Run Codex yourself in Terminal A against the W1 workspace, then return here.")?;
    let codex_file = fs::read_to_string(op.w1_path.join("src/calculator.rs"))?;
    if !codex_file.contains("Added by Codex")
        || !codex_file.contains("multiply")
        || op.w1_path.join(".git").exists()
    {
        return Err("Codex W1 verification failed".into());
    }
    op.state = op.state.advance(State::CodexW1Submitted)?;
    let publication_a = op.call(Some("agent-m4-real-codex"), "publish-e1", Some("operation:m4-real:codex-version"), "publish_version", Some(json!({"execution_id":"execution-m4-real-codex","workspace_id":"workspace-m4-real-codex","lease":lease_a,"expected_workspace_revision":0,"parent_version_id":null,"update_version_head":true})))?;
    let source_version = publication_a["version"]["version_id"]
        .as_str()
        .ok_or("missing source version")?
        .to_owned();
    let source_snapshot = publication_a["snapshot"]["snapshot_id"]
        .as_str()
        .ok_or("missing source snapshot")?
        .to_owned();
    let source_root_digest = publication_a["snapshot"]["root_digest"]
        .as_str()
        .ok_or("missing source root digest")?
        .to_owned();
    op.call(Some("agent-m4-real-codex"), "set-e1-version", None, "set_execution_current_version", Some(json!({"execution_id":"execution-m4-real-codex","version_id":source_version,"lease":lease_a,"expected_execution_revision":1,"expected_workspace_revision":2})))?;
    op.call(Some("agent-m4-real-codex"), "checkpoint-c1", None, "create_checkpoint", Some(json!({"checkpoint_id":"checkpoint-m4-real-codex","task_id":"task-m4-real-protocol-handoff","execution_id":"execution-m4-real-codex","workspace_id":"workspace-m4-real-codex","version_id":source_version,"source_operation_id":"operation:m4-real:codex-version","reason_code":"real_runtime_handoff_ready"})))?;
    op.state = op.state.advance(State::C1Created)?;
    op.call(Some("agent-m4-real-codex"), "interrupt-e1", None, "interrupt_execution", Some(json!({"execution_id":"execution-m4-real-codex","expected_revision":2,"outcome_code":"provider_handoff"})))?;
    op.state = op.state.advance(State::E1Interrupted)?;
    let resume = op.call(Some("agent-m4-real-claude"), "resume-e2", None, "resume_from_checkpoint", Some(json!({"execution_id":"execution-m4-real-claude","task_id":"task-m4-real-protocol-handoff","parent_execution_id":"execution-m4-real-codex","workspace_id":"workspace-m4-real-claude","checkpoint_id":"checkpoint-m4-real-codex"})))?;
    if resume["source_version_id"] != source_version {
        return Err("resume source Version mismatch".into());
    }
    op.state = op.state.advance(State::E2Created)?;
    let handoff_id = "handoff-m4-real-codex-claude";
    op.call(Some("agent-m4-real-codex"), "handoff-a-b", None, "create_handoff", Some(json!({"handoff_id":handoff_id,"task_id":"task-m4-real-protocol-handoff","from_execution_id":"execution-m4-real-codex","to_execution_id":"execution-m4-real-claude","source_version_id":source_version,"checkpoint_id":"checkpoint-m4-real-codex","reason_code":"codex_to_claude"})))?;
    op.call(
        Some("agent-m4-real-claude"),
        "start-e2",
        None,
        "start_execution",
        Some(json!({"execution_id":"execution-m4-real-claude","expected_revision":0})),
    )?;
    let lease_b = op.call(Some("agent-m4-real-claude"), "lease-e2", None, "acquire_workspace_lease", Some(json!({"execution_id":"execution-m4-real-claude","workspace_id":"workspace-m4-real-claude","ttl_ms":1_000_000})))?["authority"].clone();
    let materialized = op.call(Some("agent-m4-real-claude"), "materialize-e2", None, "materialize_version", Some(json!({"execution_id":"execution-m4-real-claude","workspace_id":"workspace-m4-real-claude","source_version_id":source_version,"lease":lease_b,"expected_workspace_revision":0})))?;
    if materialized["workspace_id"] != "workspace-m4-real-claude"
        || fs::read_to_string(op.w2_path.join("src/calculator.rs"))? != codex_file
    {
        return Err("W2 materialization verification failed".into());
    }
    let manifest = HandoffManifest {
        slice: "M4-021".into(),
        task_id: "task-m4-real-protocol-handoff".into(),
        source_workspace_id: "workspace-m4-real-codex".into(),
        target_workspace_id: "workspace-m4-real-claude".into(),
        e1: "execution-m4-real-codex".into(),
        e2: "execution-m4-real-claude".into(),
        c1: "checkpoint-m4-real-codex".into(),
        handoff_id: handoff_id.into(),
        source_version_id: source_version.clone(),
        source_snapshot_id: source_snapshot.clone(),
        source_root_digest: source_root_digest.clone(),
        target_workspace_path: op.w2_path.display().to_string(),
        instructions:
            "Continue Codex work, add Claude marker and subtract function, then confirm DONE."
                .into(),
    };
    let manifest_path = root.join("m4-021-handoff.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    op.state = op.state.advance(State::HandoffReady)?;
    println!("\nCLAUDE STEP\nWorkspace: {}\nResume: execution-m4-real-claude\nSource Version: {}\nCheckpoint: checkpoint-m4-real-codex\nHandoff manifest: {}\nExpected inherited marker: Added by Codex\nExpected Claude marker: Added by Claude Code\n", op.w2_path.display(), source_version, manifest_path.display());
    op.state = op.state.advance(State::WaitClaudeW2)?;
    op.confirm("Run Claude Code yourself in Terminal B against W2, then return here.")?;
    let claude_file = fs::read_to_string(op.w2_path.join("src/calculator.rs"))?;
    if !claude_file.contains("Added by Codex")
        || !claude_file.contains("Added by Claude Code")
        || !claude_file.contains("subtract")
        || op.w2_path.join(".git").exists()
        || fs::read_to_string(op.w1_path.join("src/calculator.rs"))? != codex_file
    {
        return Err("Claude W2 verification failed".into());
    }
    op.state = op.state.advance(State::ClaudeW2Submitted)?;
    let publication_b = op.call(Some("agent-m4-real-claude"), "publish-e2", Some("operation:m4-real:claude-version"), "publish_version", Some(json!({"execution_id":"execution-m4-real-claude","workspace_id":"workspace-m4-real-claude","lease":lease_b,"expected_workspace_revision":1,"parent_version_id":null,"update_version_head":true})))?;
    let target_version = publication_b["version"]["version_id"]
        .as_str()
        .ok_or("missing target version")?
        .to_owned();
    let target_snapshot = publication_b["snapshot"]["snapshot_id"]
        .as_str()
        .ok_or("missing target snapshot")?
        .to_owned();
    let target_root_digest = publication_b["snapshot"]["root_digest"]
        .as_str()
        .ok_or("missing target root digest")?
        .to_owned();
    op.call(Some("agent-m4-real-claude"), "set-e2-version", None, "set_execution_current_version", Some(json!({"execution_id":"execution-m4-real-claude","version_id":target_version,"lease":lease_b,"expected_execution_revision":1,"expected_workspace_revision":3})))?;
    op.call(Some("agent-m4-real-claude"), "checkpoint-c2", None, "create_checkpoint", Some(json!({"checkpoint_id":"checkpoint-m4-real-claude","task_id":"task-m4-real-protocol-handoff","execution_id":"execution-m4-real-claude","workspace_id":"workspace-m4-real-claude","version_id":target_version,"source_operation_id":"operation:m4-real:claude-version","reason_code":"real_runtime_continuation_saved"})))?;
    op.call(Some("agent-m4-real-claude"), "complete-e2", None, "complete_execution", Some(json!({"execution_id":"execution-m4-real-claude","expected_revision":2,"outcome_code":"continuation_complete"})))?;
    op.state = op
        .state
        .advance(State::C2Created)?
        .advance(State::E2Completed)?;
    op.shutdown()?;

    let mut final_state = State::E2Completed.advance(State::FreshProcessCheck)?;
    let mut fresh = Operator::start(repository_root.clone(), workspace_root.clone())?;
    let inspect_a = fresh.call(
        Some("agent-m4-real-codex"),
        "inspect-e1",
        None,
        "inspect_execution",
        Some(json!({"id":"execution-m4-real-codex"})),
    )?;
    let inspect_b = fresh.call(
        Some("agent-m4-real-claude"),
        "inspect-e2",
        None,
        "inspect_execution",
        Some(json!({"id":"execution-m4-real-claude"})),
    )?;
    if inspect_a["execution"]["state"] != "interrupted"
        || inspect_b["execution"]["state"] != "completed"
        || inspect_b["execution"]["parent_execution_id"] != "execution-m4-real-codex"
        || inspect_b["resume"]["checkpoint_id"] != "checkpoint-m4-real-codex"
    {
        return Err("fresh-process inspection failed".into());
    }
    fresh.shutdown()?;
    final_state = final_state.advance(State::ColdReopenCheck)?;
    let repository = Repository::open(&repository_root)?;
    let source_snapshot_record = repository
        .metadata()
        .snapshot_record(&source_snapshot)?
        .ok_or("source Snapshot record missing")?;
    let target_snapshot_record = repository
        .metadata()
        .snapshot_record(&target_snapshot)?
        .ok_or("target Snapshot record missing")?;
    let source = repository
        .metadata()
        .workspace("workspace-m4-real-codex")?
        .ok_or("source workspace missing")?;
    let target = repository
        .metadata()
        .workspace("workspace-m4-real-claude")?
        .ok_or("target workspace missing")?;
    if source_snapshot_record.root_digest != source_root_digest
        || target_snapshot_record.root_digest != target_root_digest
        || source.head.as_deref() != Some(source_root_digest.as_str())
        || target.head.as_deref() != Some(target_root_digest.as_str())
        || source.version_head_id.as_deref() != Some(source_version.as_str())
        || target.version_head_id.as_deref() != Some(target_version.as_str())
        || source.locator == target.locator
        || source.head == target.head
        || repository
            .metadata()
            .list_checkpoints("task-m4-real-protocol-handoff")?
            .len()
            != 2
    {
        return Err("cold-reopen verification failed".into());
    }
    final_state = final_state.advance(State::Pass)?;
    assert_eq!(final_state, State::Pass);
    println!("\nM4-021 FINAL ACCEPTANCE\n=======================\nProvider launch: USER CONTROLLED\nW1: PASS\nC1: PASS\nHandoff: PASS\nW2: PASS\nC2: PASS\nE2 completion: PASS\nFresh-process: PASS\nCold reopen: PASS\nWorkspace.head: PASS (root_digest)\nSnapshot identity: PASS (snapshot_id)\nFinal: PASS\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_requires_pause_points() {
        assert_eq!(State::Init.advance(State::W1Ready).unwrap(), State::W1Ready);
        assert!(State::W1Ready.advance(State::C1Created).is_err());
        assert_eq!(
            State::HandoffReady.advance(State::WaitClaudeW2).unwrap(),
            State::WaitClaudeW2
        );
    }

    #[test]
    fn manifest_round_trip_preserves_identity_separation() {
        let manifest = HandoffManifest {
            slice: "M4-021".into(),
            task_id: "task".into(),
            source_workspace_id: "w1".into(),
            target_workspace_id: "w2".into(),
            e1: "e1".into(),
            e2: "e2".into(),
            c1: "c1".into(),
            handoff_id: "handoff".into(),
            source_version_id: "v1".into(),
            source_snapshot_id: "snp-digest".into(),
            source_root_digest: "sha256:digest".into(),
            target_workspace_path: "w2".into(),
            instructions: "continue".into(),
        };
        let decoded: HandoffManifest =
            serde_json::from_str(&serde_json::to_string(&manifest).unwrap()).unwrap();
        assert_eq!(decoded.source_snapshot_id, "snp-digest");
        assert_eq!(decoded.source_root_digest, "sha256:digest");
        assert_ne!(decoded.source_snapshot_id, decoded.source_root_digest);
    }

    #[test]
    fn pause_resume_transitions_are_explicit() {
        let state = State::W1Ready.advance(State::WaitCodexW1).unwrap();
        let state = state
            .advance(State::CodexW1Submitted)
            .unwrap()
            .advance(State::C1Created)
            .unwrap();
        assert_eq!(
            state.advance(State::E1Interrupted).unwrap(),
            State::E1Interrupted
        );
    }

    #[test]
    fn cold_reopen_state_requires_final_transition() {
        assert!(State::FreshProcessCheck.advance(State::Pass).is_err());
        assert_eq!(
            State::FreshProcessCheck
                .advance(State::ColdReopenCheck)
                .unwrap(),
            State::ColdReopenCheck
        );
    }
}
