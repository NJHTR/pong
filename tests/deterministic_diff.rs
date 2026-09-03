use pong_core::canonical::canonical_bytes;
use pong_core::redaction::Redactor;
use pong_core::workspace::{LocalWorkspace, SnapshotChangeType, SnapshotOptions};
use pong_core::{PongError, Repository};
use rusqlite::Connection;
use std::fs;
use std::time::Instant;
use tempfile::{tempdir, TempDir};

struct Fixture {
    _project: TempDir,
    _workspace_parent: TempDir,
    repository: Repository,
    workspace: LocalWorkspace,
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let repository = Repository::init(project.path()).expect("repository");
    let workspace = LocalWorkspace::create(
        "ws-diff",
        "project-diff",
        workspace_parent.path().join("source"),
        Redactor::default(),
    )
    .expect("workspace");
    Fixture {
        _project: project,
        _workspace_parent: workspace_parent,
        repository,
        workspace,
    }
}

fn diff(
    fixture: &Fixture,
    old: pong_core::Snapshot,
    new: pong_core::Snapshot,
) -> pong_core::workspace::SnapshotDiff {
    fixture
        .workspace
        .diff_snapshots(fixture.repository.cas(), old.digest, new.digest)
        .expect("diff")
}

#[test]
fn d1_identical_snapshots_have_empty_diff() {
    let fixture = fixture();
    let old = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("old");
    let new = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("new");
    let result = diff(&fixture, old, new);
    assert!(result.entries.is_empty());
    assert_eq!(result.old_snapshot_id, result.new_snapshot_id);
}

#[test]
fn d2_added_file_is_reported() {
    let fixture = fixture();
    let old = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("old");
    fs::write(fixture.workspace.root().join("added.txt"), b"added").expect("write");
    let new = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("new");
    let result = diff(&fixture, old, new);
    assert_eq!(result.entries.len(), 1);
    let entry = &result.entries[0];
    assert_eq!(entry.path, "added.txt");
    assert_eq!(entry.change_type, SnapshotChangeType::Added);
    assert!(entry.old_digest.is_none());
    assert_eq!(entry.new_size, Some(5));
}

#[test]
fn d3_removed_file_is_reported() {
    let fixture = fixture();
    fs::write(fixture.workspace.root().join("removed.txt"), b"removed").expect("write");
    let old = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("old");
    fs::remove_file(fixture.workspace.root().join("removed.txt")).expect("remove");
    let new = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("new");
    let result = diff(&fixture, old, new);
    assert_eq!(result.entries.len(), 1);
    let entry = &result.entries[0];
    assert_eq!(entry.path, "removed.txt");
    assert_eq!(entry.change_type, SnapshotChangeType::Removed);
    assert_eq!(entry.old_size, Some(7));
    assert!(entry.new_digest.is_none());
}

#[test]
fn d4_modified_file_includes_old_and_new_metadata() {
    let fixture = fixture();
    fs::write(fixture.workspace.root().join("state.txt"), b"before").expect("write");
    let old = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("old");
    fs::write(fixture.workspace.root().join("state.txt"), b"after-value").expect("rewrite");
    let new = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("new");
    let result = diff(&fixture, old, new);
    assert_eq!(result.entries.len(), 1);
    let entry = &result.entries[0];
    assert_eq!(entry.path, "state.txt");
    assert_eq!(entry.change_type, SnapshotChangeType::Modified);
    assert_eq!(entry.old_size, Some(6));
    assert_eq!(entry.new_size, Some(11));
    assert_ne!(entry.old_digest, entry.new_digest);
}

#[test]
fn d5_file_directory_replacement_is_type_changed() {
    let fixture = fixture();
    fs::write(fixture.workspace.root().join("item"), b"file").expect("write");
    let old = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("old");
    fs::remove_file(fixture.workspace.root().join("item")).expect("remove");
    fs::create_dir(fixture.workspace.root().join("item")).expect("directory");
    let new = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("new");
    let result = diff(&fixture, old, new);
    assert_eq!(result.entries.len(), 1);
    let entry = &result.entries[0];
    assert_eq!(entry.path, "item");
    assert_eq!(entry.change_type, SnapshotChangeType::TypeChanged);
    assert_eq!(entry.old_type.as_deref(), Some("file"));
    assert_eq!(entry.new_type.as_deref(), Some("directory"));
}

#[test]
fn d6_multiple_changes_are_classified_without_unchanged_entries() {
    let fixture = fixture();
    fs::write(fixture.workspace.root().join("modify.txt"), b"old").expect("write");
    fs::write(fixture.workspace.root().join("remove.txt"), b"remove").expect("write");
    let old = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("old");
    fs::write(fixture.workspace.root().join("modify.txt"), b"new").expect("rewrite");
    fs::remove_file(fixture.workspace.root().join("remove.txt")).expect("remove");
    fs::write(fixture.workspace.root().join("add.txt"), b"add").expect("add");
    let new = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("new");
    let result = diff(&fixture, old, new);
    let changes = result
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.change_type))
        .collect::<Vec<_>>();
    assert_eq!(
        changes,
        vec![
            ("add.txt", SnapshotChangeType::Added),
            ("modify.txt", SnapshotChangeType::Modified),
            ("remove.txt", SnapshotChangeType::Removed),
        ]
    );
}

#[test]
fn d7_repeated_diff_is_deterministically_serialized_and_ordered() {
    let fixture = fixture();
    fs::write(fixture.workspace.root().join("z.txt"), b"z").expect("write");
    fs::write(fixture.workspace.root().join("a.txt"), b"a").expect("write");
    let old = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("old");
    fs::write(fixture.workspace.root().join("m.txt"), b"m").expect("write");
    fs::write(fixture.workspace.root().join("a.txt"), b"a2").expect("rewrite");
    let new = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("new");
    let first = diff(&fixture, old.clone(), new.clone());
    let second = diff(&fixture, old, new);
    assert_eq!(first, second);
    let first_json = serde_json::to_vec(&first).expect("json");
    let second_json = serde_json::to_vec(&second).expect("json");
    assert_eq!(first_json, second_json);
    assert_eq!(
        first
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        vec!["a.txt", "m.txt"]
    );
}

#[test]
fn d8_ten_thousand_entries_have_linear_manifest_diff() {
    let fixture = fixture();
    let old = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("old");
    for index in 0..10_000 {
        fs::write(
            fixture
                .workspace
                .root()
                .join(format!("entry-{index:05}.txt")),
            [index as u8],
        )
        .expect("write entry");
    }
    let new = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("new");
    let result = diff(&fixture, old, new);
    assert_eq!(result.entries.len(), 10_000);
    assert_eq!(result.entries.first().unwrap().path, "entry-00000.txt");
    assert_eq!(result.entries.last().unwrap().path, "entry-09999.txt");
}

#[test]
fn d8_performance_sanity_scales_manifest_sizes() {
    let fixture = fixture();
    for size in [100usize, 1_000, 10_000, 100_000] {
        let old_digest = put_synthetic_manifest(&fixture, size, false);
        let new_digest = put_synthetic_manifest(&fixture, size, true);
        let started = Instant::now();
        let result = fixture
            .workspace
            .diff_snapshots(fixture.repository.cas(), old_digest, new_digest)
            .expect("diff");
        let duration_ms = started.elapsed().as_millis();
        assert_eq!(result.entries.len(), size);
        println!("performance_entries={size} diff_duration_ms={duration_ms}");
    }
}

#[test]
fn d9_corrupt_manifest_fails_closed() {
    let fixture = fixture();
    let old = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("old");
    fs::write(fixture.workspace.root().join("new.txt"), b"new").expect("write");
    let new = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("new");
    fs::write(fixture.repository.cas().object_path(new.digest), b"{}").expect("corrupt manifest");
    assert!(matches!(
        fixture
            .workspace
            .diff_snapshots(fixture.repository.cas(), old.digest, new.digest),
        Err(PongError::Integrity(_))
    ));
}

#[test]
fn d10_cold_reopen_preserves_diff_result() {
    let fixture = fixture();
    fs::write(fixture.workspace.root().join("state.txt"), b"before").expect("write");
    let old = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("old");
    fs::write(fixture.workspace.root().join("state.txt"), b"after").expect("rewrite");
    let new = fixture
        .workspace
        .snapshot(fixture.repository.cas(), SnapshotOptions::default())
        .expect("new");
    let before = diff(&fixture, old.clone(), new.clone());
    let reopened = LocalWorkspace::open(
        "ws-diff",
        "project-diff",
        fixture.workspace.root(),
        Redactor::default(),
    )
    .expect("reopen");
    let after = reopened
        .diff_snapshots(fixture.repository.cas(), old.digest, new.digest)
        .expect("reopened diff");
    assert_eq!(before, after);
}

#[test]
fn d11_raw_v01_repository_remains_openable_and_diff_is_additive() {
    let project = tempdir().expect("project");
    let repository = Repository::init(project.path()).expect("repository layout");
    let metadata_path = repository.layout().metadata_path().to_path_buf();
    drop(repository);
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut candidate = metadata_path.as_os_str().to_os_string();
        candidate.push(suffix);
        match fs::remove_file(std::path::PathBuf::from(candidate)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("remove initialized metadata: {error}"),
        }
    }
    Connection::open(&metadata_path)
        .expect("legacy database")
        .execute_batch(include_str!("fixtures/CT-09-v01-metadata.sql"))
        .expect("legacy schema");
    let reopened = Repository::open(project.path()).expect("raw v0.1 reopen");
    let workspace = LocalWorkspace::create(
        "ws-legacy-diff",
        "legacy-project",
        project.path().join("workspace"),
        Redactor::default(),
    )
    .expect("workspace");
    let old = workspace
        .snapshot(reopened.cas(), SnapshotOptions::default())
        .expect("old");
    fs::write(workspace.root().join("new.txt"), b"new").expect("write");
    let new = workspace
        .snapshot(reopened.cas(), SnapshotOptions::default())
        .expect("new");
    let result = workspace
        .diff_snapshots(reopened.cas(), old.digest, new.digest)
        .expect("diff");
    assert_eq!(result.entries[0].change_type, SnapshotChangeType::Added);
}

fn put_synthetic_manifest(
    fixture: &Fixture,
    size: usize,
    modified: bool,
) -> pong_core::cas::Digest {
    let digest = if modified {
        "sha256:1111111111111111111111111111111111111111111111111111111111111111"
    } else {
        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    };
    let entries = (0..size)
        .map(|index| pong_core::TreeEntry {
            path: format!("entry-{index:06}.txt"),
            kind: "file".into(),
            size: 1,
            digest: Some(digest.into()),
        })
        .collect();
    let manifest = pong_core::TreeManifest {
        manifest_version: 1,
        workspace_id: "ws-diff".into(),
        project_id: "project-diff".into(),
        entries,
        redaction_profile_id: Redactor::default().profile_id().into(),
        redaction_profile_version: Redactor::default().version().into(),
    };
    let value = serde_json::to_value(manifest).expect("manifest value");
    let bytes = canonical_bytes(&value).expect("canonical manifest");
    fixture
        .repository
        .cas()
        .put("workspace/tree/v1", &bytes)
        .expect("manifest CAS publication")
}
