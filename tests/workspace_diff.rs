use pong_core::metadata::LeaseToken;
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotChangeType, SnapshotOptions, WorkspaceManager};
use pong_core::{PongError, Repository, Snapshot};
use rusqlite::Connection;
use std::fs;
use std::time::Instant;
use tempfile::{tempdir, TempDir};

struct Fixture {
    project: TempDir,
    _workspace_parent: TempDir,
    repository: Repository,
    workspace_path: std::path::PathBuf,
    lease: LeaseToken,
    snapshot: Option<Snapshot>,
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-workspace-diff",
            "project-workspace-diff",
            &serde_json::json!({"schema_version": 1, "os": "test"}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_parent.path().join("workspace");
    let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
    manager
        .create_local(
            "ws-workspace-diff",
            "project-workspace-diff",
            &workspace_path,
            Some("refs/heads/main"),
            Some("env-workspace-diff"),
            "t0",
        )
        .expect("workspace");
    let lease = manager
        .acquire_lease("ws-workspace-diff", "agent-workspace-diff", 0, 10_000)
        .expect("lease");
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local(
            "ws-workspace-diff",
            &lease,
            SnapshotOptions::default(),
            10,
            "t1",
        )
        .expect("empty reference snapshot");
    Fixture {
        project,
        _workspace_parent: workspace_parent,
        repository,
        workspace_path,
        lease,
        snapshot: Some(snapshot),
    }
}

fn fixture_with_reference_file(name: &str, bytes: &[u8]) -> Fixture {
    let mut fixture = fixture();
    fs::write(fixture.workspace_path.join(name), bytes).expect("reference file");
    let lease = fixture.lease.clone();
    fixture.snapshot = Some(
        WorkspaceManager::new(&mut fixture.repository, Redactor::default())
            .snapshot_local(
                "ws-workspace-diff",
                &lease,
                SnapshotOptions::default(),
                30,
                "t2",
            )
            .expect("reference snapshot"),
    );
    fixture
}

fn workspace_diff(fixture: &mut Fixture) -> pong_core::workspace::WorkspaceDiffResult {
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace("ws-workspace-diff")
        .expect("workspace diff")
}

#[test]
fn w1_clean_workspace_matches_head_snapshot() {
    let mut fixture = fixture_with_reference_file("state.txt", b"same");
    let result = workspace_diff(&mut fixture);
    assert!(result.diff.entries.is_empty());
    assert_eq!(
        result.reference_snapshot_id,
        "snp-".to_owned() + &fixture.snapshot.as_ref().unwrap().digest.to_string()
    );
    assert!(result.current_tree_id.starts_with("workspace-current-"));
}

#[test]
fn w2_added_file_is_reported() {
    let mut fixture = fixture_with_reference_file("state.txt", b"same");
    fs::write(fixture.workspace_path.join("added.txt"), b"added").expect("added");
    let result = workspace_diff(&mut fixture);
    assert_eq!(result.diff.entries.len(), 1);
    assert_eq!(result.diff.entries[0].path, "added.txt");
    assert_eq!(
        result.diff.entries[0].change_type,
        SnapshotChangeType::Added
    );
}

#[test]
fn w3_deleted_file_is_reported() {
    let mut fixture = fixture_with_reference_file("removed.txt", b"removed");
    fs::remove_file(fixture.workspace_path.join("removed.txt")).expect("remove");
    let result = workspace_diff(&mut fixture);
    assert_eq!(result.diff.entries.len(), 1);
    assert_eq!(result.diff.entries[0].path, "removed.txt");
    assert_eq!(
        result.diff.entries[0].change_type,
        SnapshotChangeType::Removed
    );
}

#[test]
fn w4_modified_file_is_reported() {
    let mut fixture = fixture_with_reference_file("state.txt", b"before");
    fs::write(fixture.workspace_path.join("state.txt"), b"after").expect("modify");
    let result = workspace_diff(&mut fixture);
    assert_eq!(result.diff.entries.len(), 1);
    assert_eq!(result.diff.entries[0].path, "state.txt");
    assert_eq!(
        result.diff.entries[0].change_type,
        SnapshotChangeType::Modified
    );
}

#[test]
fn w5_file_to_directory_and_directory_to_file_are_type_changes() {
    let mut file_fixture = fixture_with_reference_file("item", b"file");
    fs::remove_file(file_fixture.workspace_path.join("item")).expect("remove file");
    fs::create_dir(file_fixture.workspace_path.join("item")).expect("create directory");
    let file_to_directory = workspace_diff(&mut file_fixture);
    assert_eq!(
        file_to_directory.diff.entries[0].change_type,
        SnapshotChangeType::TypeChanged
    );
    assert_eq!(file_to_directory.diff.entries[0].path, "item");

    let mut directory_fixture = fixture();
    fs::create_dir(directory_fixture.workspace_path.join("item")).expect("directory");
    fs::write(
        directory_fixture
            .workspace_path
            .join("item")
            .join("child.txt"),
        b"child",
    )
    .expect("child");
    let lease = directory_fixture.lease.clone();
    directory_fixture.snapshot = Some(
        WorkspaceManager::new(&mut directory_fixture.repository, Redactor::default())
            .snapshot_local(
                "ws-workspace-diff",
                &lease,
                SnapshotOptions::default(),
                30,
                "t2",
            )
            .expect("directory reference"),
    );
    fs::remove_dir_all(directory_fixture.workspace_path.join("item")).expect("remove directory");
    fs::write(directory_fixture.workspace_path.join("item"), b"file").expect("file");
    let directory_to_file = workspace_diff(&mut directory_fixture);
    assert!(directory_to_file
        .diff
        .entries
        .iter()
        .any(|entry| entry.path == "item" && entry.change_type == SnapshotChangeType::TypeChanged));
}

#[test]
fn w6_multiple_changes_are_deterministic_and_omit_unchanged_paths() {
    let mut fixture = fixture();
    fs::write(fixture.workspace_path.join("modify.txt"), b"old").expect("modify");
    fs::write(fixture.workspace_path.join("remove.txt"), b"remove").expect("remove");
    let lease = fixture.lease.clone();
    fixture.snapshot = Some(
        WorkspaceManager::new(&mut fixture.repository, Redactor::default())
            .snapshot_local(
                "ws-workspace-diff",
                &lease,
                SnapshotOptions::default(),
                30,
                "t2",
            )
            .expect("reference"),
    );
    fs::write(fixture.workspace_path.join("modify.txt"), b"new").expect("rewrite");
    fs::remove_file(fixture.workspace_path.join("remove.txt")).expect("remove");
    fs::write(fixture.workspace_path.join("add.txt"), b"add").expect("add");
    fs::write(fixture.workspace_path.join("unchanged.txt"), b"unchanged").expect("new file");
    let first = workspace_diff(&mut fixture);
    let second = workspace_diff(&mut fixture);
    assert_eq!(first, second);
    assert_eq!(
        first
            .diff
            .entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry.change_type))
            .collect::<Vec<_>>(),
        vec![
            ("add.txt", SnapshotChangeType::Added),
            ("modify.txt", SnapshotChangeType::Modified),
            ("remove.txt", SnapshotChangeType::Removed),
            ("unchanged.txt", SnapshotChangeType::Added),
        ]
    );
}

#[test]
fn w7_repeated_scan_of_the_same_tree_has_identical_result() {
    let mut fixture = fixture_with_reference_file("state.txt", b"same");
    let first = workspace_diff(&mut fixture);
    let second = workspace_diff(&mut fixture);
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
}

#[test]
fn w8_ten_thousand_files_are_classified_correctly() {
    let mut fixture = fixture();
    for index in 0..10_000 {
        fs::write(
            fixture.workspace_path.join(format!("entry-{index:05}.txt")),
            [index as u8],
        )
        .expect("entry");
    }
    let result = workspace_diff(&mut fixture);
    assert_eq!(result.diff.entries.len(), 10_000);
    assert!(result
        .diff
        .entries
        .iter()
        .all(|entry| entry.change_type == SnapshotChangeType::Added));
    assert_eq!(result.diff.entries[0].path, "entry-00000.txt");
    assert_eq!(result.diff.entries[9_999].path, "entry-09999.txt");
}

#[test]
fn w9_corrupt_reference_manifest_fails_closed() {
    let mut fixture = fixture_with_reference_file("state.txt", b"same");
    let snapshot = fixture.snapshot.as_ref().expect("snapshot");
    fs::write(
        fixture.repository.cas().object_path(snapshot.digest),
        b"{\"entries\":[]}",
    )
    .expect("corrupt manifest");
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace("ws-workspace-diff")
        .expect_err("corrupt manifest");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}

#[test]
fn w10_missing_head_snapshot_is_integrity_error() {
    let mut fixture = fixture();
    let connection = Connection::open(fixture.repository.active_metadata_path()).expect("sqlite");
    connection
        .execute(
            "UPDATE workspaces SET head = NULL WHERE workspace_id = ?1",
            ["ws-workspace-diff"],
        )
        .expect("clear head");
    drop(connection);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace("ws-workspace-diff")
        .expect_err("missing head");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}

#[test]
fn w11_cold_reopen_preserves_workspace_diff() {
    let mut fixture = fixture_with_reference_file("state.txt", b"before");
    fs::write(fixture.workspace_path.join("state.txt"), b"after").expect("modify");
    let before = workspace_diff(&mut fixture);
    drop(fixture.repository);
    let mut reopened = Repository::open(fixture.project.path()).expect("reopen");
    let after = WorkspaceManager::new(&mut reopened, Redactor::default())
        .diff_workspace("ws-workspace-diff")
        .expect("reopened diff");
    assert_eq!(before, after);
}

#[test]
fn w12_mutation_between_observations_is_explicitly_point_in_time() {
    let mut fixture = fixture_with_reference_file("state.txt", b"same");
    let first = workspace_diff(&mut fixture);
    fs::write(fixture.workspace_path.join("new.txt"), b"new").expect("mutation");
    let second = workspace_diff(&mut fixture);
    assert!(first.diff.entries.is_empty());
    assert_eq!(second.diff.entries.len(), 1);
    assert_eq!(
        second.diff.entries[0].change_type,
        SnapshotChangeType::Added
    );
    // No watcher or lock is promised: each call is a point-in-time
    // observation, and concurrent external mutation may be unstable.
}

#[test]
fn w13_head_identity_corruption_fails_closed_before_current_tree_scan() {
    let mut fixture = fixture_with_reference_file("state.txt", b"same");
    let head = fixture
        .repository
        .metadata()
        .workspace("ws-workspace-diff")
        .unwrap()
        .unwrap()
        .head
        .expect("head");
    let connection = Connection::open(fixture.repository.active_metadata_path()).expect("sqlite");
    connection
        .execute(
            "UPDATE snapshots SET project_id = 'other-project' WHERE root_digest = ?1",
            [&head],
        )
        .expect("corrupt snapshot identity");
    drop(connection);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace("ws-workspace-diff")
        .expect_err("identity corruption");
    assert!(matches!(error, PongError::Integrity(_)));
}

#[test]
fn w14_performance_samples_cover_documented_tree_sizes() {
    for size in [100usize, 1_000, 10_000] {
        let mut fixture = fixture();
        for index in 0..size {
            fs::write(
                fixture.workspace_path.join(format!("entry-{index:05}.txt")),
                [index as u8],
            )
            .expect("entry");
        }
        let started = Instant::now();
        let result = workspace_diff(&mut fixture);
        let duration_ms = started.elapsed().as_millis();
        assert_eq!(result.diff.entries.len(), size);
        println!("performance_entries={size} diff_duration_ms={duration_ms}");
    }
}

#[test]
fn w15_current_tree_secret_bytes_fail_closed() {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-workspace-diff-secret",
            "project-workspace-diff-secret",
            &serde_json::json!({"schema_version": 1, "os": "test"}),
            "t0",
        )
        .expect("environment");
    let mut redactor = Redactor::default();
    redactor
        .register_secret("current-tree-secret")
        .expect("secret");
    let workspace_path = workspace_parent.path().join("workspace");
    let mut manager = WorkspaceManager::new(&mut repository, redactor.clone());
    manager
        .create_local(
            "ws-workspace-diff-secret",
            "project-workspace-diff-secret",
            &workspace_path,
            None,
            Some("env-workspace-diff-secret"),
            "t0",
        )
        .expect("workspace");
    let lease = manager
        .acquire_lease("ws-workspace-diff-secret", "agent-secret", 0, 10_000)
        .expect("lease");
    manager
        .snapshot_local(
            "ws-workspace-diff-secret",
            &lease,
            SnapshotOptions::default(),
            10,
            "t1",
        )
        .expect("empty head");
    fs::write(workspace_path.join("secret.txt"), b"current-tree-secret").expect("secret file");
    let error = WorkspaceManager::new(&mut repository, redactor)
        .diff_workspace("ws-workspace-diff-secret")
        .expect_err("secret bytes must fail closed");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}
