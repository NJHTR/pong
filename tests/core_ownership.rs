//! M4-009 single long-lived Core repository ownership contract.

use pong_core::{MigrationSpec, PongError, Repository};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use tempfile::tempdir;

struct CoreProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl CoreProcess {
    fn start(repository: &Path, workspace_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-protocol"))
            .arg("--repository")
            .arg(repository)
            .arg("--workspace-root")
            .arg(workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            stdin: child.stdin.take().unwrap(),
            stdout: BufReader::new(child.stdout.take().unwrap()),
            child,
        }
    }

    fn hello(&mut self, request_id: &str) {
        let request = json!({
            "protocol_version": "1.0",
            "request_id": request_id,
            "caller_agent_id": null,
            "issued_at": "t0",
            "operation_id": null,
            "operation": "hello"
        });
        serde_json::to_writer(&mut self.stdin, &request).unwrap();
        self.stdin.write_all(b"\n").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["status"], "ok", "{response:#}");
    }

    fn terminate(mut self) {
        self.child.kill().unwrap();
        let status = self.child.wait().unwrap();
        assert!(!status.success());
    }

    fn close(mut self) {
        drop(self.stdin);
        assert!(self.child.wait().unwrap().success());
    }
}

#[test]
fn core_owner_is_exclusive_and_blocks_direct_repository_access() {
    let root = tempdir().unwrap();
    let repository = Repository::init(root.path()).unwrap();
    drop(repository);

    let owner = Repository::open_as_core_owner(root.path()).unwrap();
    assert!(matches!(
        Repository::open_as_core_owner(root.path()),
        Err(PongError::Conflict(_))
    ));
    assert!(matches!(
        Repository::open(root.path()),
        Err(PongError::Conflict(_))
    ));
    let migration = Repository::migrate(
        root.path(),
        MigrationSpec::new("migration-while-owned", "generation-while-owned"),
    );
    assert!(
        matches!(migration, Err(PongError::Conflict(_))),
        "unexpected migration result while Core owns repository: {migration:?}"
    );

    drop(owner);
    Repository::open(root.path()).expect("owner drop releases repository access");
}

#[test]
fn process_owner_rejects_a_second_core_and_recovers_after_forced_exit() {
    let root = tempdir().unwrap();
    let workspace_root = tempdir().unwrap();
    let repository = Repository::init(root.path()).unwrap();
    drop(repository);

    let mut owner = CoreProcess::start(root.path(), workspace_root.path());
    owner.hello("owner-ready");

    assert!(matches!(
        Repository::open(root.path()),
        Err(PongError::Conflict(_))
    ));
    let second = Command::new(env!("CARGO_BIN_EXE_pong-agent-protocol"))
        .arg("--repository")
        .arg(root.path())
        .arg("--workspace-root")
        .arg(workspace_root.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&second.stderr).contains("CONFLICT"));

    owner.terminate();
    let reopened = Repository::open(root.path()).expect("OS releases ownership after process exit");
    drop(reopened);

    let mut replacement = CoreProcess::start(root.path(), workspace_root.path());
    replacement.hello("replacement-ready");
    replacement.close();
}
