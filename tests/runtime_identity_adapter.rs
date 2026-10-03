use pong_core::runtime_identity::{
    RuntimeIdentityAdapter, RuntimeIdentityMetadata, RUNTIME_IDENTITY_DIRECTORY,
    RUNTIME_IDENTITY_SCHEMA_VERSION,
};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use tempfile::tempdir;

fn identity_path(root: &Path, profile: &str) -> PathBuf {
    root.join(".pong")
        .join(RUNTIME_IDENTITY_DIRECTORY)
        .join(format!("{profile}.json"))
}

#[test]
fn first_runtime_creates_identity_metadata() {
    let root = tempdir().expect("runtime root");
    let path = identity_path(root.path(), "codex");
    let adapter = RuntimeIdentityAdapter::open(&path, "codex").expect("identity");
    assert!(path.is_file());
    assert_eq!(
        adapter.metadata().schema_version,
        RUNTIME_IDENTITY_SCHEMA_VERSION
    );
    assert_eq!(adapter.metadata().provider, "codex");
    assert!(adapter.agent_id().starts_with("agent:local:"));
}

#[test]
fn restart_preserves_agent_and_changes_session() {
    let root = tempdir().expect("runtime root");
    let path = identity_path(root.path(), "codex");
    let first = RuntimeIdentityAdapter::open(&path, "codex").expect("first runtime");
    let first_id = first.agent_id().to_owned();
    let first_session = first.session_id().to_owned();
    drop(first);
    let second = RuntimeIdentityAdapter::open(&path, "codex").expect("restarted runtime");
    assert_eq!(second.agent_id(), first_id);
    assert_ne!(second.session_id(), first_session);
}

#[test]
fn different_profiles_get_different_logical_agents() {
    let root = tempdir().expect("runtime root");
    let first = RuntimeIdentityAdapter::open(identity_path(root.path(), "codex-a"), "codex")
        .expect("first profile");
    let second = RuntimeIdentityAdapter::open(identity_path(root.path(), "codex-b"), "codex")
        .expect("second profile");
    assert_ne!(first.agent_id(), second.agent_id());
}

#[test]
fn provider_thread_is_metadata_and_never_agent_identity() {
    let root = tempdir().expect("runtime root");
    let adapter = RuntimeIdentityAdapter::open(identity_path(root.path(), "codex"), "codex")
        .expect("identity");
    let metadata: Value = serde_json::from_str(
        &adapter
            .provider_thread_metadata("01thread-real")
            .expect("provider thread metadata"),
    )
    .expect("thread metadata JSON");
    assert_eq!(metadata["provider"], "codex");
    assert_eq!(metadata["thread_id"], "01thread-real");
    assert_ne!(metadata["thread_id"], adapter.agent_id());
}

#[test]
fn existing_identity_is_not_overwritten_or_rebound_to_another_provider() {
    let root = tempdir().expect("runtime root");
    let path = identity_path(root.path(), "codex");
    let adapter = RuntimeIdentityAdapter::open(&path, "codex").expect("identity");
    let bytes = fs::read(&path).expect("identity bytes");
    let error = RuntimeIdentityAdapter::open(&path, "claude").expect_err("provider conflict");
    assert!(error.to_string().contains("belongs to provider codex"));
    assert_eq!(fs::read(&path).expect("identity remains"), bytes);
    drop(adapter);
    let _: RuntimeIdentityMetadata =
        serde_json::from_slice(&bytes).expect("metadata remains parseable");
}

struct ProtocolProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl ProtocolProcess {
    fn start(project_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-protocol"))
            .arg("--project-root")
            .arg(project_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start Pong protocol");
        Self {
            stdin: Some(child.stdin.take().expect("protocol stdin")),
            stdout: BufReader::new(child.stdout.take().expect("protocol stdout")),
            child,
        }
    }

    fn call(&mut self, request: Value) -> Value {
        let stdin = self.stdin.as_mut().expect("protocol remains open");
        serde_json::to_writer(&mut *stdin, &request).expect("write request");
        stdin.write_all(b"\n").expect("request delimiter");
        stdin.flush().expect("flush request");
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read response");
        assert!(!line.is_empty(), "protocol exited before response");
        let response: Value = serde_json::from_str(&line).expect("response JSON");
        assert_eq!(response["status"], "ok", "{response:#}");
        response
    }

    fn shutdown(mut self) {
        drop(self.stdin.take());
        assert!(self.child.wait().expect("protocol exit").success());
    }
}

fn register_and_inspect(
    process: &mut ProtocolProcess,
    adapter: &RuntimeIdentityAdapter,
    provider_metadata: &str,
    request_suffix: &str,
) -> Value {
    let agent_id = adapter.agent_id();
    let registration = process.call(json!({
        "protocol_version": "1.0",
        "request_id": format!("register:{request_suffix}"),
        "caller_agent_id": agent_id,
        "issued_at": adapter.metadata().created_at,
        "operation_id": null,
        "operation": "register_agent",
        "payload": {
            "agent_id": agent_id,
            "provider_metadata": provider_metadata,
            "display_name": null
        }
    }));
    assert_eq!(registration["result"]["kind"], "agent");
    let inspected = process.call(json!({
        "protocol_version": "1.0",
        "request_id": format!("inspect:{request_suffix}"),
        "caller_agent_id": agent_id,
        "issued_at": format!("runtime:{request_suffix}"),
        "operation_id": null,
        "operation": "get_agent",
        "payload": { "id": agent_id }
    }));
    assert_eq!(inspected["result"]["kind"], "agent");
    inspected["result"]["data"].clone()
}

fn real_codex_thread_id(workdir: &Path) -> String {
    let output = Command::new("codex")
        .args([
            "exec",
            "--dangerously-bypass-approvals-and-sandbox",
            "--ephemeral",
            "--skip-git-repo-check",
            "--json",
            "-C",
        ])
        .arg(workdir)
        .arg("Do not use any tools or modify files. Return exactly the word READY.")
        .output()
        .expect("start real Codex");
    assert!(output.status.success(), "Codex failed: {:?}", output);
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|event| {
            (event["type"] == "thread.started")
                .then(|| event["thread_id"].as_str().map(ToOwned::to_owned))
                .flatten()
        })
        .expect("Codex thread.started identity")
}

#[test]
#[ignore = "requires the authenticated real Codex CLI and the easyCode target"]
fn real_codex_registers_and_reconnects_with_stable_adapter_identity() {
    let project = PathBuf::from(r"C:\Users\NJHTR\IdeaProjects\easyCode");
    assert!(project.is_dir(), "target repository is unavailable");
    let probe = tempdir().expect("Codex probe directory");
    let thread_id = real_codex_thread_id(probe.path());
    let identity = identity_path(&project, "codex-real");
    let first = RuntimeIdentityAdapter::open(&identity, "codex").expect("first adapter");
    let first_agent_id = first.agent_id().to_owned();
    let first_session = first.session_id().to_owned();
    let provider_metadata = first.provider_metadata();
    let mut first_process = ProtocolProcess::start(&project);
    let first_agent = register_and_inspect(&mut first_process, &first, provider_metadata, "first");
    first_process.shutdown();
    assert_eq!(first_agent["agent_id"], first_agent_id);

    let second = RuntimeIdentityAdapter::open(&identity, "codex").expect("restarted adapter");
    let second_session = second.session_id().to_owned();
    let second_metadata = second.provider_metadata();
    let mut second_process = ProtocolProcess::start(&project);
    let second_agent =
        register_and_inspect(&mut second_process, &second, second_metadata, "second");
    second_process.shutdown();
    println!(
        "real_register agent_id={} first_session={} second_session={} thread_id={}",
        first_agent_id, first_session, second_session, thread_id
    );
    assert_eq!(second.agent_id(), first_agent_id);
    assert_ne!(second_session, first_session);
    assert_eq!(second_agent["agent_id"], first_agent_id);
    assert_eq!(second_agent["provider_metadata"], second_metadata);
}
