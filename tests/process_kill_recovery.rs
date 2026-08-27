use pong_core::metadata::IdempotencyKey;
use pong_core::Repository;
use serde_json::json;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::{tempdir, TempDir};

const MODE_ENV: &str = "PONG_PROCESS_KILL_MODE";
const ROOT_ENV: &str = "PONG_PROCESS_KILL_ROOT";
const READY_ENV: &str = "PONG_PROCESS_KILL_READY";

/// This test doubles as the child entry point. The parent waits until the
/// durable boundary marker exists and then terminates the process externally;
/// no production code contains a process-exit hook.
#[test]
fn child_process_entrypoint() {
    let Some(mode) = env::var_os(MODE_ENV) else {
        return;
    };
    let root = PathBuf::from(env::var_os(ROOT_ENV).expect("child root"));
    let ready = PathBuf::from(env::var_os(READY_ENV).expect("child ready path"));
    let mut repository = Repository::open(root).expect("child open");
    let key = IdempotencyKey {
        project_id: "project-process-kill".into(),
        actor_id: "agent-process-kill".into(),
        request_id: format!("request-{}", mode.to_string_lossy()),
    };
    repository
        .metadata_mut()
        .record_intent(
            &key.project_id,
            &key.actor_id,
            &key.request_id,
            &format!("operation-{}", mode.to_string_lossy()),
            &json!({"action":"write","mode":mode.to_string_lossy()}),
            "2026-08-19T00:00:00Z",
        )
        .expect("intent durable");
    if mode == "after-outcome" {
        repository
            .metadata_mut()
            .record_outcome(&key, "outcome_durable", &json!({"status":"accepted"}))
            .expect("outcome durable");
    }
    let mut marker = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(ready)
        .expect("ready marker");
    marker
        .write_all(b"durable-boundary-reached")
        .expect("ready write");
    marker.sync_all().expect("ready sync");
    drop(marker);

    // The parent owns termination. Keeping the repository open here ensures
    // the kill happens while the process still holds its normal handles.
    loop {
        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn process_kill_after_intent_never_becomes_success() {
    let project = run_killed_child("after-intent");
    let repository = Repository::open(project.path()).expect("cold reopen");
    let key = key("after-intent");
    let operation = repository
        .metadata()
        .operation(&key)
        .expect("operation lookup")
        .expect("durable intent");
    assert_eq!(operation.phase, "unknown");
    assert!(repository
        .metadata()
        .unfinished_operations()
        .expect("unfinished")
        .is_empty());
}

#[test]
fn process_kill_after_outcome_keeps_republishable_outbox() {
    let project = run_killed_child("after-outcome");
    let repository = Repository::open(project.path()).expect("cold reopen");
    let key = key("after-outcome");
    let operation = repository
        .metadata()
        .operation(&key)
        .expect("operation lookup")
        .expect("durable outcome");
    assert_eq!(operation.phase, "outcome_durable");
    assert_eq!(
        repository
            .metadata()
            .unfinished_operations()
            .expect("outbox")
            .len(),
        1
    );
}

fn key(mode: &str) -> IdempotencyKey {
    IdempotencyKey {
        project_id: "project-process-kill".into(),
        actor_id: "agent-process-kill".into(),
        request_id: format!("request-{mode}"),
    }
}

fn run_killed_child(mode: &str) -> TempDir {
    let project = tempdir().expect("project");
    Repository::init(project.path()).expect("init");
    let ready = project.path().join("child.ready");
    let executable = env::current_exe().expect("test executable");
    let mut child = Command::new(executable)
        .arg("--exact")
        .arg("child_process_entrypoint")
        .arg("--nocapture")
        .env(MODE_ENV, mode)
        .env(ROOT_ENV, project.path())
        .env(READY_ENV, &ready)
        .spawn()
        .expect("spawn child");

    wait_for_ready(&mut child, &ready);
    child.kill().expect("terminate child");
    let status = child.wait().expect("wait child");
    assert!(!status.success(), "the child must have been terminated");
    assert!(!ready.exists(), "test marker is removed before inspection");
    project
}

fn wait_for_ready(child: &mut Child, ready: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if ready.is_file() {
            // Remove the marker only after the child has durably reached the
            // boundary; the parent asserts the cleanup below.
            fs::remove_file(ready).expect("remove ready marker");
            return;
        }
        if let Some(status) = child.try_wait().expect("poll child") {
            panic!("child exited before boundary: {status}");
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("child did not reach the durable boundary in time");
}
