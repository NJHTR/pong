use pong_core::{MigrationFailpoint, MigrationFailpoints, MigrationSpec, Repository};
use serde_json::json;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::{tempdir, TempDir};

const MODE_ENV: &str = "PONG_MIGRATION_KILL_MODE";
const ROOT_ENV: &str = "PONG_MIGRATION_KILL_ROOT";
const READY_ENV: &str = "PONG_MIGRATION_KILL_READY";

#[test]
fn child_migration_entrypoint() {
    let Some(mode) = env::var_os(MODE_ENV) else {
        return;
    };
    let root = PathBuf::from(env::var_os(ROOT_ENV).expect("child root"));
    let ready = PathBuf::from(env::var_os(READY_ENV).expect("child ready"));
    let point = match mode.to_string_lossy().as_ref() {
        "old-only" => MigrationFailpoint::BeforeSelectorReplace,
        "new-only" => MigrationFailpoint::AfterSelectorReplace,
        other => panic!("unknown migration kill mode: {other}"),
    };
    let error = Repository::migrate_with_failpoints(
        root,
        MigrationSpec::new("migration-kill", "generation-kill"),
        MigrationFailpoints::once(point),
    )
    .expect_err("child migration must stop at the selected boundary");
    assert_eq!(error.code(), "FAULT_INJECTED");

    let mut marker = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(ready)
        .expect("ready marker");
    marker
        .write_all(b"migration-boundary-reached")
        .expect("ready write");
    marker.sync_all().expect("ready sync");
    drop(marker);

    loop {
        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn process_kill_before_selector_leaves_legacy_only() {
    let project = run_killed_migration_child("old-only");
    let repository = Repository::open(project.path()).expect("legacy cold reopen");
    assert_eq!(repository.active_generation_id(), None);
    assert_eq!(repository.marker().repository_format, "0.1");
}

#[test]
fn process_kill_after_selector_uses_new_generation_only() {
    let project = run_killed_migration_child("new-only");
    let repository = Repository::open(project.path()).expect("new generation cold reopen");
    assert_eq!(repository.active_generation_id(), Some("generation-kill"));
    assert_eq!(repository.marker().repository_format, "0.2");
    let journal: serde_json::Value = serde_json::from_slice(
        &fs::read(project.path().join(".pong/migrations/migration-kill.json"))
            .expect("migration journal"),
    )
    .expect("journal JSON");
    assert_eq!(journal["status"], "finalized");
}

fn run_killed_migration_child(mode: &str) -> TempDir {
    let project = tempdir().expect("project");
    let mut repository = Repository::init(project.path()).expect("init");
    repository
        .metadata_mut()
        .record_intent(
            "project-kill",
            "agent-kill",
            "request-kill",
            "operation-kill",
            &json!({"action": "migration-test"}),
            "t1",
        )
        .expect("legacy journal");
    drop(repository);

    let ready = project.path().join("migration-child.ready");
    let executable = env::current_exe().expect("test executable");
    let mut child = Command::new(executable)
        .arg("--exact")
        .arg("child_migration_entrypoint")
        .arg("--nocapture")
        .env(MODE_ENV, mode)
        .env(ROOT_ENV, project.path())
        .env(READY_ENV, &ready)
        .spawn()
        .expect("spawn child");
    wait_for_ready(&mut child, &ready);
    child.kill().expect("terminate child");
    let status = child.wait().expect("wait child");
    assert!(!status.success(), "the child must be terminated");
    assert!(!ready.exists(), "test marker is removed before inspection");
    project
}

fn wait_for_ready(child: &mut Child, ready: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if ready.is_file() {
            fs::remove_file(ready).expect("remove ready marker");
            return;
        }
        if let Some(status) = child.try_wait().expect("poll child") {
            panic!("child exited before ready marker: {status}");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    panic!("child did not reach the migration boundary");
}
