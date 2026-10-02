use pong_core::bootstrap::{discover, BootstrapMetadata, BOOTSTRAP_PROTOCOL_VERSION};
use pong_core::Repository;
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::tempdir;

fn run_initialize(path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pong-bootstrap"))
        .arg("initialize")
        .arg(path)
        .output()
        .expect("run pong-bootstrap initialize")
}

#[test]
fn initializes_fresh_directory_and_existing_non_git_directory_without_touching_user_files() {
    let fresh = tempdir().expect("fresh directory");
    let fresh_output = run_initialize(fresh.path());
    assert!(fresh_output.status.success(), "{fresh_output:?}");
    assert_bootstrap_consumable(fresh.path());

    let existing = tempdir().expect("existing directory");
    let user_file = existing.path().join("README.md");
    fs::write(&user_file, b"user content").expect("user file");
    let before = fs::read(&user_file).expect("read user file");
    let output = run_initialize(existing.path());
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fs::read(&user_file).expect("read user file"), before);
    assert!(!existing.path().join(".git").exists());
    assert_bootstrap_consumable(existing.path());
}

#[test]
fn initialization_is_idempotent_and_does_not_overwrite_existing_bootstrap() {
    let project = tempdir().expect("project");
    let first = run_initialize(project.path());
    assert!(first.status.success(), "{first:?}");
    let bootstrap_path = BootstrapMetadata::path(project.path());
    let first_bytes = fs::read(&bootstrap_path).expect("first bootstrap");

    let second = run_initialize(project.path());
    assert!(second.status.success(), "{second:?}");
    assert_eq!(
        fs::read(&bootstrap_path).expect("second bootstrap"),
        first_bytes
    );

    let custom = br#"{
  "schema_version": 1,
  "protocol_version": "1.0",
  "repository_root": ".",
  "workspace_root": ".pong/workspaces",
  "workspace_id": null,
  "core_endpoint": null
}
"#;
    fs::write(&bootstrap_path, custom).expect("custom compatible bootstrap");
    let third = run_initialize(project.path());
    assert!(third.status.success(), "{third:?}");
    assert_eq!(
        fs::read(&bootstrap_path).expect("preserved bootstrap"),
        custom
    );
}

#[test]
fn rejects_invalid_path_and_existing_incompatible_pong_state_without_overwrite() {
    let parent = tempdir().expect("parent");
    let file_path = parent.path().join("not-a-directory");
    fs::write(&file_path, b"user file").expect("file path");
    let invalid = run_initialize(&file_path);
    assert!(!invalid.status.success());
    assert_eq!(fs::read(&file_path).expect("file remains"), b"user file");

    let incompatible = tempdir().expect("incompatible");
    fs::create_dir(incompatible.path().join(".pong")).expect("pong directory");
    let marker = incompatible.path().join(".pong/repository.json");
    fs::write(&marker, b"not-json").expect("incompatible marker");
    let output = run_initialize(incompatible.path());
    assert!(!output.status.success());
    assert_eq!(fs::read(&marker).expect("marker remains"), b"not-json");
}

#[test]
fn bootstrap_metadata_is_v1_and_protocol_process_can_consume_it() {
    let project = tempdir().expect("project");
    let output = run_initialize(project.path());
    assert!(output.status.success(), "{output:?}");
    let resolution = discover(project.path()).expect("discover bootstrap");
    assert_eq!(resolution.protocol_version, BOOTSTRAP_PROTOCOL_VERSION);
    assert_eq!(
        resolution.repository_root,
        project.path().canonicalize().unwrap()
    );
    assert_eq!(
        resolution.workspace_root,
        project
            .path()
            .join(".pong/workspaces")
            .canonicalize()
            .unwrap()
    );
    assert_eq!(resolution.core_endpoint, None);

    let mut child = Command::new(env!("CARGO_BIN_EXE_pong-agent-protocol"))
        .arg("--project-root")
        .arg(project.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start protocol consumer");
    let mut stdin = child.stdin.take().expect("protocol stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("protocol stdout"));
    serde_json::to_writer(
        &mut stdin,
        &json!({
            "protocol_version": "1.0",
            "request_id": "hello-bootstrap-test",
            "caller_agent_id": null,
            "issued_at": "test-time",
            "operation_id": null,
            "operation": "hello"
        }),
    )
    .expect("write hello");
    stdin.write_all(b"\n").expect("hello delimiter");
    stdin.flush().expect("flush hello");
    let mut line = String::new();
    stdout.read_line(&mut line).expect("read hello");
    let response: Value = serde_json::from_str(&line).expect("hello response");
    assert_eq!(response["status"], "ok");
    drop(stdin);
    assert!(child.wait().expect("wait protocol").success());
}

fn assert_bootstrap_consumable(project: &Path) {
    let path = BootstrapMetadata::path(project);
    assert!(path.is_file());
    let metadata: BootstrapMetadata =
        serde_json::from_slice(&fs::read(&path).expect("bootstrap bytes")).expect("bootstrap JSON");
    assert_eq!(metadata.schema_version, 1);
    assert_eq!(metadata.protocol_version, BOOTSTRAP_PROTOCOL_VERSION);
    assert_eq!(metadata.repository_root, ".");
    assert_eq!(metadata.workspace_root, ".pong/workspaces");
    assert!(metadata.workspace_id.is_none());
    assert!(metadata.core_endpoint.is_none());
    assert!(project.join(".pong/workspaces").is_dir());
    let repository = Repository::open(project).expect("open initialized repository");
    assert_eq!(repository.marker().repository_format, "0.1");
    assert_eq!(repository.marker().storage_driver, "cas+sqlite");
}
