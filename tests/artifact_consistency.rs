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

fn assert_release_bundle_checksums(root: &Path) {
    let bundle = root.join("artifacts/m1-release-evidence");
    let sums_path = bundle.join("SHA256SUMS");
    let sums = fs::read_to_string(&sums_path).expect("release bundle SHA256SUMS");
    let mut listed = 0usize;
    for line in sums.lines().filter(|line| !line.trim().is_empty()) {
        let (expected, relative) = line
            .split_once("  ")
            .unwrap_or_else(|| panic!("malformed SHA256SUMS line: {line}"));
        assert_eq!(expected.len(), 64, "invalid SHA-256 in SHA256SUMS: {line}");
        assert!(
            expected.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid SHA-256 in SHA256SUMS: {line}"
        );
        assert!(relative != "SHA256SUMS", "SHA256SUMS must not hash itself");
        let path =
            relative_artifact_path(root, &format!("artifacts/m1-release-evidence/{relative}"));
        assert!(
            path.is_file(),
            "SHA256SUMS references missing file: {relative}"
        );
        let actual = hex::encode_upper(Sha256::digest(
            fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
        ));
        assert_eq!(
            actual,
            expected.to_ascii_uppercase(),
            "bundle hash drift: {relative}"
        );
        listed += 1;
    }
    let actual_files = fs::read_dir(&bundle)
        .expect("release bundle directory")
        .flat_map(|entry| {
            let entry = entry.expect("release bundle entry");
            let path = entry.path();
            if path.is_dir() {
                let mut files = Vec::new();
                collect_files(&path, &mut files);
                files
            } else {
                vec![path]
            }
        })
        .filter(|path| path.as_path() != sums_path.as_path())
        .count();
    assert_eq!(
        listed, actual_files,
        "SHA256SUMS does not cover the full bundle"
    );
}

fn assert_native_failure_artifacts(root: &Path, run_id: &str, macos_complete: bool) {
    let run = root.join(format!(
        "artifacts/m1-platform-runs/github-actions-run-{run_id}"
    ));
    let metadata: Value = serde_json::from_slice(
        &fs::read(run.join("download-metadata.json")).expect("native CI download metadata"),
    )
    .expect("native CI download metadata JSON");
    assert_eq!(metadata["workflow_run_id"], run_id);
    assert_eq!(metadata["status"], "failure");
    if run_id == "33085292318" || run_id == "33142438624" {
        for platform in ["linux", "macos"] {
            let commands = metadata["command_disposition"][platform]
                .as_array()
                .unwrap_or_else(|| {
                    panic!("run {run_id} is missing {platform} command disposition")
                });
            assert_eq!(
                commands.len(),
                13,
                "run {run_id} must retain all native command dispositions for {platform}"
            );
            assert!(
                commands.iter().all(|command| {
                    command.get("label").and_then(Value::as_str).is_some()
                        && command.get("command").and_then(Value::as_str).is_some()
                        && command.get("exit_code").and_then(Value::as_i64).is_some()
                }),
                "run {run_id} contains an incomplete {platform} command disposition"
            );
        }
    }
    let linux_root = run.join("linux");
    assert!(linux_root.join("platform/platform-metadata.json").is_file());
    assert!(linux_root.join("artifact-manifest.json").is_file());
    assert!(linux_root.join("SHA256SUMS").is_file());
    assert!(linux_root.join("logs/cold-reopen.exit").is_file());

    let macos_root = run.join("macos");
    if macos_complete {
        assert!(macos_root.join("platform/platform-metadata.json").is_file());
        assert!(macos_root.join("artifact-manifest.json").is_file());
        assert!(macos_root.join("SHA256SUMS").is_file());
        assert!(macos_root.join("logs/cold-reopen.exit").is_file());
    } else {
        assert!(macos_root.join("build/msrv-178-test.log").is_file());
        assert!(macos_root.join("build/msrv-178-clippy.log").is_file());
        assert!(macos_root.join("logs/cold-reopen.exit").is_file());
        assert!(!macos_root.join("platform/platform-metadata.json").exists());
        assert!(!macos_root.join("artifact-manifest.json").exists());
        assert!(!macos_root.join("build/stable-test.log").exists());
    }
}

fn assert_native_success_artifacts(root: &Path, run_id: &str) {
    let run = root.join(format!(
        "artifacts/m1-platform-runs/github-actions-run-{run_id}"
    ));
    let metadata: Value = serde_json::from_slice(
        &fs::read(run.join("download-metadata.json")).expect("native CI download metadata"),
    )
    .expect("native CI download metadata JSON");
    assert_eq!(metadata["workflow_run_id"], run_id);
    assert_eq!(metadata["status"], "success");
    assert_eq!(
        metadata["commit"],
        "6cb62fb455e92ab731a4bb5233856d10c1f1ce93"
    );

    for (platform, expected_filesystem) in [("linux", "ext4"), ("macos", "unknown")] {
        let platform_root = run.join(platform);
        let commands = metadata["command_disposition"][platform]
            .as_array()
            .unwrap_or_else(|| panic!("run {run_id} is missing {platform} command disposition"));
        assert_eq!(
            commands.len(),
            13,
            "run {run_id} must retain all native command dispositions for {platform}"
        );
        assert!(
            commands.iter().all(|command| {
                command.get("label").and_then(Value::as_str).is_some()
                    && command.get("command").and_then(Value::as_str).is_some()
                    && command["exit_code"].as_i64() == Some(0)
            }),
            "run {run_id} contains a non-zero or incomplete {platform} command disposition"
        );

        let command_lines = fs::read_to_string(platform_root.join("commands.tsv"))
            .expect("native command manifest")
            .lines()
            .map(|line| {
                line.split_once('\t')
                    .unwrap_or_else(|| panic!("malformed command manifest line: {line}"))
                    .0
                    .to_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(command_lines.len(), commands.len());
        for label in command_lines {
            let exit_path = platform_root.join(format!(
                "{}/{}.exit",
                if label.starts_with("stable-") || label.starts_with("msrv-") {
                    "build"
                } else {
                    "logs"
                },
                label
            ));
            assert_eq!(
                fs::read_to_string(&exit_path)
                    .unwrap_or_else(|error| panic!("{}: {error}", exit_path.display()))
                    .trim(),
                "0",
                "native command did not exit zero: {}",
                exit_path.display()
            );
        }

        let platform_metadata_path = platform_root.join("platform/platform-metadata.json");
        let platform_metadata: Value = serde_json::from_slice(
            &fs::read(&platform_metadata_path)
                .unwrap_or_else(|error| panic!("{}: {error}", platform_metadata_path.display())),
        )
        .unwrap_or_else(|error| {
            panic!(
                "{} is invalid JSON: {error}",
                platform_metadata_path.display()
            )
        });
        assert_eq!(platform_metadata["commit"], metadata["commit"]);
        assert_eq!(platform_metadata["workflow_run_id"], run_id);
        assert_eq!(
            platform_metadata["environment"]["filesystem"],
            expected_filesystem
        );

        let manifest_path = platform_root.join("artifact-manifest.json");
        let manifest: Value = serde_json::from_slice(
            &fs::read(&manifest_path)
                .unwrap_or_else(|error| panic!("{}: {error}", manifest_path.display())),
        )
        .unwrap_or_else(|error| panic!("{} is invalid JSON: {error}", manifest_path.display()));
        assert_eq!(manifest["commit"], metadata["commit"]);
        let files = manifest["files"].as_array().expect("native artifact files");
        assert!(!files.is_empty());
        for file in files {
            let relative = file["path"].as_str().expect("native artifact path");
            let resolved = relative_artifact_path(&platform_root, relative);
            assert!(resolved.is_file(), "native artifact is missing: {relative}");
            assert_eq!(
                fs::metadata(&resolved)
                    .unwrap_or_else(|error| panic!("{}: {error}", resolved.display()))
                    .len(),
                file["bytes"].as_u64().expect("native artifact byte count")
            );
        }

        let sums_path = platform_root.join("SHA256SUMS");
        let sums = fs::read_to_string(&sums_path).expect("native artifact SHA256SUMS");
        let mut listed = 0usize;
        for line in sums.lines().filter(|line| !line.trim().is_empty()) {
            let (expected, relative) = line
                .split_once("  ")
                .unwrap_or_else(|| panic!("malformed native SHA256SUMS line: {line}"));
            assert_eq!(expected.len(), 64);
            assert!(expected.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert!(relative.starts_with("evidence/"));
            let relative = relative.strip_prefix("evidence/").expect("evidence prefix");
            let resolved = relative_artifact_path(&platform_root, relative);
            assert!(
                resolved.is_file(),
                "native checksum references missing file: {relative}"
            );
            assert_eq!(
                hex::encode_upper(Sha256::digest(
                    fs::read(&resolved)
                        .unwrap_or_else(|error| panic!("{}: {error}", resolved.display())),
                )),
                expected.to_ascii_uppercase(),
                "native artifact hash drift: {relative}"
            );
            listed += 1;
        }
        let actual_files = {
            let mut files = Vec::new();
            collect_files(&platform_root, &mut files);
            files.into_iter().filter(|path| path != &sums_path).count()
        };
        assert_eq!(
            listed, actual_files,
            "native SHA256SUMS does not cover all files"
        );
    }
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("artifact subdirectory") {
        let path = entry.expect("artifact subdirectory entry").path();
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}

#[test]
fn release_bundle_checksums_and_status_matrix_are_consistent() {
    let root = repository_root();
    assert_release_bundle_checksums(&root);
    let matrix_path = root.join("artifacts/m1-release-evidence/m1-release-matrix.json");
    let matrix: Value =
        serde_json::from_slice(&fs::read(matrix_path).expect("normalized release matrix"))
            .expect("normalized release matrix JSON");
    assert_eq!(matrix["release_decision"], "NOT_PASSED");
    let entries = matrix["entries"].as_array().expect("matrix entries");
    assert!(!entries.is_empty(), "normalized release matrix is empty");
    for entry in entries {
        let status = entry["status"].as_str().expect("matrix status");
        assert!(
            matches!(status, "PASS" | "FAIL" | "BLOCKED" | "NOT_APPLICABLE"),
            "unexpected normalized status: {status}"
        );
        if status == "PASS" {
            assert!(
                entry.get("environment").and_then(Value::as_str).is_some(),
                "PASS matrix entry must declare environment"
            );
            let commands = entry["commands"].as_array().expect("PASS commands");
            assert!(!commands.is_empty(), "PASS matrix entry has no command");
            let exit_codes = entry["exit_codes"].as_array().expect("PASS exit codes");
            assert!(!exit_codes.is_empty(), "PASS matrix entry has no exit code");
            assert!(
                exit_codes.iter().all(|code| code.as_i64() == Some(0)),
                "PASS matrix entry contains a non-zero exit code"
            );
            let artifacts = entry["artifacts"].as_array().expect("PASS artifacts");
            let hashes = entry["sha256"].as_array().expect("PASS hashes");
            assert!(!artifacts.is_empty(), "PASS matrix entry has no artifact");
            assert_eq!(
                artifacts.len(),
                hashes.len(),
                "PASS artifacts/hashes length mismatch"
            );
            for hash in hashes {
                let hash = hash.as_str().expect("PASS artifact hash");
                assert_eq!(hash.len(), 64, "PASS artifact hash must be SHA-256");
                assert!(
                    hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
                    "PASS artifact hash must be hexadecimal"
                );
            }
        }
    }

    let scenarios_path = root.join("artifacts/m1-release-evidence/fault/m1-fault-scenarios.json");
    let scenarios: Value =
        serde_json::from_slice(&fs::read(scenarios_path).expect("normalized fault scenarios"))
            .expect("normalized fault scenarios JSON");
    let scenarios = scenarios["scenarios"].as_array().expect("fault scenarios");
    assert_eq!(scenarios.len(), 14, "expected FI-01 through FI-14");
    for scenario in scenarios {
        for field in [
            "id",
            "environment",
            "setup",
            "command",
            "expected",
            "actual",
            "status",
        ] {
            assert!(
                scenario.get(field).and_then(Value::as_str).is_some(),
                "fault scenario is missing string field {field}"
            );
        }
        let status = scenario["status"].as_str().expect("fault scenario status");
        assert!(
            matches!(status, "PASS" | "FAIL" | "BLOCKED" | "NOT_APPLICABLE"),
            "unexpected fault scenario status: {status}"
        );
        match scenario.get("artifact") {
            Some(Value::String(path)) => {
                let resolved = relative_artifact_path(&root, path);
                assert!(resolved.is_file(), "fault artifact does not exist: {path}");
                let hash = scenario["artifact_sha256"]
                    .as_str()
                    .expect("fault artifact hash");
                assert_eq!(hash.len(), 64, "fault artifact hash must be SHA-256");
                assert!(
                    hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
                    "fault artifact hash must be hexadecimal"
                );
            }
            Some(Value::Null) | None => {
                assert_eq!(
                    status, "BLOCKED",
                    "only blocked scenarios may omit artifacts"
                );
                assert!(
                    scenario["artifact_sha256"].is_null(),
                    "blocked scenario without artifact must have null hash"
                );
            }
            Some(other) => panic!("fault artifact must be string or null: {other}"),
        }
    }
}

#[test]
fn retained_m1_raw_logs_and_fault_matrix_references_are_consistent() {
    let root = repository_root();
    collect_raw_log_records(&root, &root.join("artifacts"));
    assert_native_failure_artifacts(&root, "33074865773", true);
    assert_native_failure_artifacts(&root, "33080915116", false);
    assert_native_failure_artifacts(&root, "33085292318", true);
    assert_native_failure_artifacts(&root, "33142438624", true);
    assert_native_success_artifacts(&root, "33145714975");

    let matrix_path = root.join("artifacts/m1-fault-matrix.json");
    let matrix: Value =
        serde_json::from_slice(&fs::read(matrix_path).expect("fault matrix artifact"))
            .expect("fault matrix JSON");
    assert_eq!(matrix["status"], "not_passed");
    assert_retained_artifacts_exist(&root, &matrix);
}
