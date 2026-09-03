use pong_core::redaction::Redactor;
use pong_core::workspace::{LocalWorkspace, RestoreOptions, SnapshotOptions, WorkspaceManager};
use pong_core::{PongError, Repository};
use rusqlite::Connection;
use std::fs;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::tempdir;

struct Fixture {
    project: tempfile::TempDir,
    workspace_parent: tempfile::TempDir,
    repository: Repository,
    workspace_path: std::path::PathBuf,
    lease: pong_core::LeaseToken,
    snapshot_id: String,
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-reconciliation",
            "project-reconciliation",
            &serde_json::json!({"schema_version": 1, "os": "test"}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_parent.path().join("workspace");
    let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
    manager
        .create_local(
            "ws-reconciliation",
            "project-reconciliation",
            &workspace_path,
            Some("refs/heads/main"),
            Some("env-reconciliation"),
            "t0",
        )
        .expect("workspace");
    let lease = manager
        .acquire_lease("ws-reconciliation", "agent-reconciliation", 0, 10_000)
        .expect("lease");
    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local(
            "ws-reconciliation",
            &lease,
            SnapshotOptions::default(),
            10,
            "t1",
        )
        .expect("empty snapshot");
    Fixture {
        project,
        workspace_parent,
        repository,
        workspace_path,
        lease,
        snapshot_id: snapshot.snapshot_id,
    }
}

fn diff(fixture: &mut Fixture) -> pong_core::WorkspaceDiffResult {
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace("ws-reconciliation")
        .expect("diff")
}

#[test]
fn observation_records_revision_environment_and_stability() {
    let mut fixture = fixture();
    let result = diff(&mut fixture);
    assert_eq!(result.observation_revision, 1);
    assert_eq!(result.environment_id.as_deref(), Some("env-reconciliation"));
    assert_eq!(result.observation_stability, "stable");
    assert!(result.diff.entries.is_empty());
}

#[test]
fn snapshot_publication_reconciles_head_and_next_diff_is_empty() {
    let mut fixture = fixture();
    fs::write(fixture.workspace_path.join("state.txt"), b"before").expect("state");
    let before = diff(&mut fixture);
    assert_eq!(before.diff.entries.len(), 1);
    let next_snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-reconciliation",
            &fixture.lease,
            SnapshotOptions::default(),
            20,
            "t2",
        )
        .expect("publish snapshot");
    assert_ne!(next_snapshot.snapshot_id, fixture.snapshot_id);
    let after = diff(&mut fixture);
    assert!(after.diff.entries.is_empty());
    assert_eq!(after.reference_snapshot_id, next_snapshot.snapshot_id);
    assert_eq!(after.observation_revision, 2);
}

#[test]
fn restore_materialization_can_be_diffed_and_modification_is_visible() {
    let mut fixture = fixture();
    fs::write(fixture.workspace_path.join("state.txt"), b"durable").expect("state");
    let snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-reconciliation",
            &fixture.lease,
            SnapshotOptions::default(),
            20,
            "t2",
        )
        .expect("snapshot");
    let destination = fixture.workspace_parent.path().join("restored");
    WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .restore_local(
            "ws-reconciliation",
            &snapshot.snapshot_id,
            &destination,
            RestoreOptions {
                operation_id: "op-reconciliation-restore".into(),
                request_id: "req-reconciliation-restore".into(),
                agent_id: "agent-reconciliation".into(),
                now: "t3".into(),
            },
        )
        .expect("restore");
    let root_digest = fixture
        .repository
        .metadata()
        .snapshot_record(&snapshot.snapshot_id)
        .expect("snapshot metadata")
        .expect("snapshot row")
        .root_digest;
    let digest = pong_core::cas::Digest::from_hex(
        root_digest
            .strip_prefix("sha256:")
            .expect("prefixed root digest"),
    )
    .expect("snapshot digest");
    let restored = LocalWorkspace::open(
        "ws-reconciliation",
        "project-reconciliation",
        &destination,
        Redactor::default(),
    )
    .expect("open restored workspace");
    let restored_diff = restored
        .diff_against_snapshot(fixture.repository.cas(), digest)
        .expect("restored diff");
    assert!(restored_diff.entries.is_empty());
    fs::write(destination.join("state.txt"), b"changed").expect("modify restored");
    let changed = restored
        .diff_against_snapshot(fixture.repository.cas(), digest)
        .expect("changed restored diff");
    assert_eq!(changed.entries.len(), 1);
}

#[test]
fn repeated_observations_are_deterministic_and_read_only() {
    let mut fixture = fixture();
    fs::write(fixture.workspace_path.join("state.txt"), b"same").expect("state");
    let before = fixture
        .repository
        .metadata()
        .workspace("ws-reconciliation")
        .unwrap()
        .unwrap();
    let event_count = fixture
        .repository
        .metadata()
        .list_event_envelopes("project-reconciliation", 0)
        .unwrap()
        .len();
    let first = diff(&mut fixture);
    let second = diff(&mut fixture);
    let after = fixture
        .repository
        .metadata()
        .workspace("ws-reconciliation")
        .unwrap()
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(before.revision, after.revision);
    assert_eq!(before.head, after.head);
    assert_eq!(
        event_count,
        fixture
            .repository
            .metadata()
            .list_event_envelopes("project-reconciliation", 0)
            .unwrap()
            .len()
    );
}

#[test]
fn environment_binding_mismatch_fails_closed() {
    let mut fixture = fixture();
    let connection = Connection::open(fixture.repository.active_metadata_path()).expect("sqlite");
    connection
        .execute(
            "UPDATE workspaces SET environment_id = 'missing-environment' WHERE workspace_id = ?1",
            ["ws-reconciliation"],
        )
        .expect("corrupt environment binding");
    drop(connection);
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace("ws-reconciliation")
        .expect_err("environment mismatch");
    assert!(matches!(error, PongError::Integrity(_)));
}

#[test]
fn revision_change_during_scan_returns_unstable_observation() {
    let mut fixture = fixture();
    for index in 0..10_000 {
        fs::write(
            fixture.workspace_path.join(format!("entry-{index:05}.txt")),
            [index as u8],
        )
        .expect("entry");
    }
    let metadata_path = fixture.repository.active_metadata_path();
    let updater = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        let connection = Connection::open(metadata_path).expect("sqlite updater");
        connection
            .execute(
                "UPDATE workspaces SET revision = revision + 1 WHERE workspace_id = ?1",
                ["ws-reconciliation"],
            )
            .expect("revision update");
    });
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace("ws-reconciliation")
        .expect_err("unstable observation");
    updater.join().expect("updater");
    assert_eq!(error.code(), "CONFLICT");
    assert!(error.to_string().contains("UNSTABLE_OBSERVATION"));
}

#[test]
fn head_change_during_scan_returns_unstable_observation() {
    let mut fixture = fixture();
    let head_a = fixture
        .repository
        .metadata()
        .workspace("ws-reconciliation")
        .unwrap()
        .unwrap()
        .head
        .expect("initial head");
    fs::write(fixture.workspace_path.join("head-change.txt"), b"head b").expect("head file");
    let snapshot_b = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-reconciliation",
            &fixture.lease,
            SnapshotOptions::default(),
            20,
            "t2",
        )
        .expect("second head");
    let head_b = format!("sha256:{}", snapshot_b.digest);
    let connection = Connection::open(fixture.repository.active_metadata_path()).expect("sqlite");
    connection
        .execute(
            "UPDATE workspaces SET head = ?2, revision = 1 WHERE workspace_id = ?1",
            rusqlite::params!["ws-reconciliation", head_a],
        )
        .expect("restore initial head");
    drop(connection);
    for index in 0..10_000 {
        fs::write(
            fixture.workspace_path.join(format!("entry-{index:05}.txt")),
            [index as u8],
        )
        .expect("entry");
    }
    let metadata_path = fixture.repository.active_metadata_path();
    let updater = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        let connection = Connection::open(metadata_path).expect("sqlite updater");
        connection
            .execute(
                "UPDATE workspaces SET head = ?2, revision = 2 WHERE workspace_id = ?1",
                rusqlite::params!["ws-reconciliation", head_b],
            )
            .expect("head update");
    });
    let error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace("ws-reconciliation")
        .expect_err("unstable head observation");
    updater.join().expect("updater");
    assert_eq!(error.code(), "CONFLICT");
    assert!(error.to_string().contains("UNSTABLE_OBSERVATION"));
}

#[test]
fn cold_reopen_preserves_observation_semantics() {
    let mut fixture = fixture();
    fs::write(fixture.workspace_path.join("state.txt"), b"same").expect("state");
    let before = diff(&mut fixture);
    drop(fixture.repository);
    let mut reopened = Repository::open(fixture.project.path()).expect("reopen");
    let after = WorkspaceManager::new(&mut reopened, Redactor::default())
        .diff_workspace("ws-reconciliation")
        .expect("reopened diff");
    assert_eq!(before, after);
}

#[test]
fn head_change_is_observed_from_the_new_durable_reference() {
    let mut fixture = fixture();
    fs::write(fixture.workspace_path.join("state.txt"), b"new head").expect("state");
    let snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-reconciliation",
            &fixture.lease,
            SnapshotOptions::default(),
            20,
            "t2",
        )
        .expect("new head");
    let result = diff(&mut fixture);
    assert_eq!(result.reference_snapshot_id, snapshot.snapshot_id);
    assert!(result.diff.entries.is_empty());
}

#[test]
fn reconciliation_performance_samples_cover_1k_and_10k_entries() {
    for size in [1_000usize, 10_000] {
        let mut fixture = fixture();
        for index in 0..size {
            fs::write(
                fixture.workspace_path.join(format!("entry-{index:05}.txt")),
                [index as u8],
            )
            .expect("entry");
        }
        let started = Instant::now();
        let result = diff(&mut fixture);
        let duration_ms = started.elapsed().as_millis();
        assert_eq!(result.diff.entries.len(), size);
        println!("reconciliation_entries={size} reconciliation_diff_duration_ms={duration_ms}");
    }
}

#[test]
fn legacy_repository_remains_readable_without_diff_mutation() {
    let fixture = fixture();
    let root = fixture.project.path().to_path_buf();
    drop(fixture.repository);
    let reopened = Repository::open(root).expect("legacy-compatible reopen");
    assert!(reopened
        .metadata()
        .workspace("ws-reconciliation")
        .unwrap()
        .is_some());
}
