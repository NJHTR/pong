//! Machine-check the retained M1 evidence package without promoting it to a
//! release claim.  These checks catch stale paths and edited raw logs while
//! leaving platform/owner acceptance decisions in the evidence documents.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn relative_artifact_path(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    assert!(
        !path.is_absolute(),
        "artifact path must be repository-relative: {value}"
    );
    assert!(
        !path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir)),
        "artifact path must not escape the repository: {value}"
    );
    let resolved = root.join(path);
    assert!(
        resolved.starts_with(root),
        "artifact path escaped repository: {value}"
    );
    resolved
}

fn assert_raw_log_digest(root: &Path, record_path: &Path, record: &Value) {
    let Some(raw_log) = record.get("raw_log").and_then(Value::as_str) else {
        return;
    };
    let expected = record
        .get("raw_log_sha256")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            panic!(
                "{} has raw_log but no raw_log_sha256",
                record_path.display()
            )
        });
    assert!(
        expected.len() == 64 && expected.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{} has an invalid raw_log_sha256",
        record_path.display()
    );
    let raw_path = relative_artifact_path(root, raw_log);
    let bytes = fs::read(&raw_path).unwrap_or_else(|error| {
        panic!(
            "{} points to unreadable raw log {}: {error}",
            record_path.display(),
            raw_path.display()
        )
    });
    let actual = hex::encode_upper(Sha256::digest(bytes));
    assert_eq!(
        actual,
        expected.to_ascii_uppercase(),
        "raw log hash drift for {}",
        record_path.display()
    );
}

fn assert_nested_raw_log_digests(root: &Path, record_path: &Path, record: &Value) {
    match record {
        Value::Object(object) => {
            assert_raw_log_digest(root, record_path, record);
            for child in object.values() {
                assert_nested_raw_log_digests(root, record_path, child);
            }
        }
        Value::Array(values) => {
            for child in values {
                assert_nested_raw_log_digests(root, record_path, child);
            }
        }
        _ => {}
    }
}

fn collect_raw_log_records(root: &Path, directory: &Path) {
    for entry in fs::read_dir(directory).expect("artifact directory") {
        let entry = entry.expect("artifact directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_raw_log_records(root, &path);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let record: Value = serde_json::from_slice(
            &fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
        )
        .unwrap_or_else(|error| panic!("{} is invalid JSON: {error}", path.display()));
        assert_nested_raw_log_digests(root, &path, &record);
    }
}

fn assert_retained_artifacts_exist(root: &Path, value: &Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Array(paths)) = object.get("retained_artifacts") {
                for path in paths {
                    let path = path
                        .as_str()
                        .expect("retained_artifacts entries must be strings");
                    let resolved = relative_artifact_path(root, path);
                    assert!(
                        resolved.is_file(),
                        "retained artifact does not exist: {path}"
                    );
                }
            }
            for child in object.values() {
                assert_retained_artifacts_exist(root, child);
            }
        }
        Value::Array(values) => {
            for child in values {
                assert_retained_artifacts_exist(root, child);
            }
        }
        _ => {}
    }
}

#[test]
fn retained_m1_raw_logs_and_fault_matrix_references_are_consistent() {
    let root = repository_root();
    collect_raw_log_records(&root, &root.join("artifacts"));

    let matrix_path = root.join("artifacts/m1-fault-matrix.json");
    let matrix: Value =
        serde_json::from_slice(&fs::read(matrix_path).expect("fault matrix artifact"))
            .expect("fault matrix JSON");
    assert_eq!(matrix["status"], "not_passed");
    assert_retained_artifacts_exist(&root, &matrix);
}
