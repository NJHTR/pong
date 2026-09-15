//! M4-011 Repository access policy and unsupported-path safety contract.

use pong_core::{PongError, Repository};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::tempdir;

const CHILD_MODE: &str = "PONG_REPOSITORY_ACCESS_POLICY_CHILD_MODE";
const CHILD_ROOT: &str = "PONG_REPOSITORY_ACCESS_POLICY_ROOT";
const CHILD_RESULT: &str = "PONG_REPOSITORY_ACCESS_POLICY_RESULT";

#[test]
fn direct_access_probe_child() {
    if env::var(CHILD_MODE).ok().as_deref() != Some("direct_probe") {
        return;
    }
    let root = PathBuf::from(env::var_os(CHILD_ROOT).unwrap());
    let result = PathBuf::from(env::var_os(CHILD_RESULT).unwrap());
    let observation = match Repository::open(root) {
        Ok(_) => "OPENED".to_string(),
        Err(error) => format!("{}:{:?}", error.code(), error.raw_os_error()),
    };
    fs::write(result, observation).unwrap();
}

fn direct_probe(executable: &Path, root: &Path, result: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .arg("--exact")
        .arg("direct_access_probe_child")
        .arg("--nocapture")
        .env(CHILD_MODE, "direct_probe")
        .env(CHILD_ROOT, root)
        .env(CHILD_RESULT, result)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    command
}

#[test]
fn unsupported_direct_multi_process_access_fails_closed() {
    if env::var(CHILD_MODE).is_ok() {
        return;
    }
    let repository_root = tempdir().unwrap();
    let result_root = tempdir().unwrap();
    let repository = Repository::init(repository_root.path()).unwrap();
    let marker_before = repository.marker().clone();
    drop(repository);

    let owner = Repository::open_as_core_owner(repository_root.path()).unwrap();
    let executable = env::current_exe().unwrap();
    let mut probes = Vec::new();
    for index in 0..2 {
        let result = result_root.path().join(format!("probe-{index}.result"));
        let child = direct_probe(&executable, repository_root.path(), &result)
            .spawn()
            .unwrap();
        probes.push((result, child));
    }

    for (result, mut child) in probes {
        assert!(child.wait().unwrap().success());
        assert_eq!(fs::read_to_string(result).unwrap(), "CONFLICT:None");
    }
    assert!(matches!(
        Repository::open(repository_root.path()),
        Err(PongError::Conflict(_))
    ));

    drop(owner);
    let reopened = Repository::open(repository_root.path())
        .expect("unsupported direct access must not damage the Repository");
    assert_eq!(reopened.marker(), &marker_before);
    assert!(reopened
        .metadata()
        .operation_record("unsupported-direct-operation")
        .unwrap()
        .is_none());
}
