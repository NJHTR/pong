use pong_core::metadata::{MetadataFailpoint, MetadataFailpoints, WorkspaceRecord};
use pong_core::redaction::Redactor;
use pong_core::workspace::{SnapshotOptions, WorkspaceLifecycleAction, WorkspaceManager};
use pong_core::{PongError, Repository};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

struct Fixture {
    project: tempfile::TempDir,
    _workspace_parent: tempfile::TempDir,
    repository: Repository,
    lease: pong_core::LeaseToken,
}

// TEST_DOUBLE: the integration suite models provider failure before invoking
// the durable core transition; a failed provider call must leave the row
// untouched and must not fabricate a lifecycle success.
struct TestProvider {
    succeeds: bool,
}

impl TestProvider {
    fn execute(&self) -> Result<(), PongError> {
        if self.succeeds {
            Ok(())
        } else {
            Err(PongError::Io(std::io::Error::other(
                "TEST_DOUBLE provider failure",
            )))
        }
    }
}

fn fixture() -> Fixture {
    let project = tempdir().expect("project");
    let workspace_parent = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .record_environment(
            "env-integration",
            "project-integration",
            &json!({"schema_version": 1, "os": "windows"}),
            "t0",
        )
        .expect("environment");
    let workspace_path = workspace_parent.path().join("source");
    let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
    manager
        .create_local(
            "ws-integration",
            "project-integration",
            &workspace_path,
            Some("refs/heads/main"),
            Some("env-integration"),
            "t0",
        )
        .expect("workspace");
    let lease = manager
        .acquire_lease("ws-integration", "agent-integration", 0, 60_000)
        .expect("lease");
    fs::write(workspace_path.join("state.txt"), b"lifecycle").expect("state");
    Fixture {
        project,
        _workspace_parent: workspace_parent,
        repository,
        lease,
    }
}

fn current(repository: &Repository) -> WorkspaceRecord {
    repository
        .metadata()
        .workspace("ws-integration")
        .expect("workspace read")
        .expect("workspace row")
}

fn transition(
    repository: &mut Repository,
    lease: &pong_core::LeaseToken,
    revision: i64,
    action: WorkspaceLifecycleAction,
    now_ms: i64,
    updated_at: &str,
) -> Result<WorkspaceRecord, PongError> {
    WorkspaceManager::new(repository, Redactor::default()).transition_lifecycle(
        "ws-integration",
        lease,
        revision,
        action,
        now_ms,
        updated_at,
    )
}

#[test]
fn create_open_and_legal_transitions_are_durable_and_monotonic() {
    let mut fixture = fixture();
    let created = current(&fixture.repository);
    assert_eq!(created.status, "created");
    assert_eq!(created.revision, 0);

    let opened = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::Open,
        1,
        "t1",
    )
    .expect("open");
    assert_eq!(opened.status, "created");
    assert_eq!(opened.revision, 0);

    let preparing = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        2,
        "t2",
    )
    .expect("begin capture");
    assert_eq!(preparing.status, "preparing");
    assert_eq!(preparing.revision, 1);

    let snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-integration",
            &fixture.lease,
            SnapshotOptions::default(),
            3,
            "t3",
        )
        .expect("snapshot");
    assert_eq!(snapshot.snapshot_id, format!("snp-{}", snapshot.digest));
    let ready = current(&fixture.repository);
    assert_eq!(ready.status, "ready");
    assert_eq!(ready.revision, 2);

    let active = transition(
        &mut fixture.repository,
        &fixture.lease,
        2,
        WorkspaceLifecycleAction::Activate,
        4,
        "t4",
    )
    .expect("activate");
    assert_eq!(active.status, "active");
    assert_eq!(active.revision, 3);
    let paused = transition(
        &mut fixture.repository,
        &fixture.lease,
        3,
        WorkspaceLifecycleAction::Pause,
        5,
        "t5",
    )
    .expect("pause");
    assert_eq!(paused.status, "paused");
    assert_eq!(paused.revision, 4);
    let resumed = transition(
        &mut fixture.repository,
        &fixture.lease,
        4,
        WorkspaceLifecycleAction::Resume,
        6,
        "t6",
    )
    .expect("resume");
    assert_eq!(resumed.status, "active");
    assert_eq!(resumed.revision, 5);
    let archived = transition(
        &mut fixture.repository,
        &fixture.lease,
        5,
        WorkspaceLifecycleAction::Close,
        7,
        "t7",
    )
    .expect("close");
    assert_eq!(archived.status, "archived");
    assert_eq!(archived.revision, 6);
}

#[test]
fn invalid_transition_stale_revision_and_stale_lease_fail_closed() {
    let mut fixture = fixture();
    let invalid = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::Resume,
        1,
        "t1",
    )
    .expect_err("invalid transition");
    assert_eq!(invalid.code(), "CONFLICT");
    assert_eq!(current(&fixture.repository).revision, 0);

    let preparing = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        2,
        "t2",
    )
    .expect("begin capture");
    assert_eq!(preparing.revision, 1);
    let stale_revision = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        3,
        "t3",
    )
    .expect_err("stale revision");
    assert_eq!(stale_revision.code(), "CONFLICT");
    assert_eq!(current(&fixture.repository).revision, 1);

    let expired = pong_core::LeaseToken {
        expires_at_ms: 1,
        ..fixture.lease.clone()
    };
    let stale_lease = transition(
        &mut fixture.repository,
        &expired,
        1,
        WorkspaceLifecycleAction::BeginCapture,
        100_000,
        "t10",
    )
    .expect_err("stale lease");
    assert_eq!(stale_lease.code(), "CONFLICT");
    assert_eq!(current(&fixture.repository).revision, 1);
}

#[test]
fn exact_retry_does_not_increment_revision_twice() {
    let mut fixture = fixture();
    let first = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        1,
        "t1",
    )
    .expect("first");
    let snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-integration",
            &fixture.lease,
            SnapshotOptions::default(),
            2,
            "t2",
        )
        .expect("snapshot");
    assert!(snapshot.snapshot_id.starts_with("snp-"));
    let completed = transition(
        &mut fixture.repository,
        &fixture.lease,
        2,
        WorkspaceLifecycleAction::Activate,
        3,
        "t3",
    )
    .expect("complete");
    let retry = transition(
        &mut fixture.repository,
        &fixture.lease,
        2,
        WorkspaceLifecycleAction::Activate,
        4,
        "t4",
    )
    .expect("exact retry");
    assert_eq!(retry, completed);
    assert_ne!(retry, first);
    assert_eq!(current(&fixture.repository).revision, 3);
}

#[test]
fn persistence_failures_leave_pre_or_post_state_and_retries_are_deterministic() {
    let mut fixture = fixture();
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeSqliteCommit,
        ));
    let before_error = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        1,
        "t1",
    )
    .expect_err("pre-commit failure");
    assert_eq!(before_error.code(), "FAULT_INJECTED");
    assert_eq!(current(&fixture.repository).status, "created");
    assert_eq!(current(&fixture.repository).revision, 0);
    let after_before_retry = transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        2,
        "t2",
    )
    .expect("retry after pre-commit");
    assert_eq!(after_before_retry.status, "preparing");
    assert_eq!(after_before_retry.revision, 1);

    let snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-integration",
            &fixture.lease,
            SnapshotOptions::default(),
            3,
            "t3",
        )
        .expect("snapshot before post-commit transition");
    assert!(snapshot.snapshot_id.starts_with("snp-"));
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterSqliteCommit,
        ));
    let post_error = transition(
        &mut fixture.repository,
        &fixture.lease,
        2,
        WorkspaceLifecycleAction::Activate,
        4,
        "t4",
    )
    .expect_err("post-commit failure");
    assert_eq!(post_error.code(), "FAULT_INJECTED");
    assert_eq!(current(&fixture.repository).status, "active");
    assert_eq!(current(&fixture.repository).revision, 3);
    let after_post_retry = transition(
        &mut fixture.repository,
        &fixture.lease,
        2,
        WorkspaceLifecycleAction::Activate,
        5,
        "t5",
    )
    .expect("retry after post-commit");
    assert_eq!(after_post_retry.status, "active");
    assert_eq!(after_post_retry.revision, 3);
}

#[test]
fn interrupted_transition_reopens_and_recovers_without_phantom_success() {
    let mut fixture = fixture();
    let snapshot = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .snapshot_local(
            "ws-integration",
            &fixture.lease,
            SnapshotOptions::default(),
            1,
            "t1",
        )
        .expect("snapshot");
    assert!(snapshot.snapshot_id.starts_with("snp-"));
    fixture
        .repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterSqliteCommit,
        ));
    let error = transition(
        &mut fixture.repository,
        &fixture.lease,
        1,
        WorkspaceLifecycleAction::BeginRecovery,
        2,
        "t2",
    )
    .expect_err("interrupted transition");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert_eq!(current(&fixture.repository).status, "reconciling");
    assert_eq!(current(&fixture.repository).revision, 2);

    let reopened = Repository::open(fixture.project.path()).expect("cold reopen");
    let mut reopened = reopened;
    let reopened_record = reopened
        .metadata()
        .workspace("ws-integration")
        .expect("workspace read")
        .expect("workspace row");
    assert_eq!(reopened_record.status, "reconciling");
    assert_eq!(reopened_record.revision, 2);
    let reopened_lease = reopened
        .metadata()
        .workspace_lease("ws-integration")
        .expect("lease read")
        .expect("lease row");
    assert_eq!(
        reopened_lease.agent_id.as_deref(),
        Some("agent-integration")
    );
    assert_eq!(reopened_lease.epoch, fixture.lease.epoch);
    let recovered = transition(
        &mut reopened,
        &fixture.lease,
        2,
        WorkspaceLifecycleAction::CompleteRecovery,
        3,
        "t3",
    );
    assert_eq!(recovered.expect("recovery").status, "ready");
    assert_eq!(
        reopened
            .metadata()
            .workspace("ws-integration")
            .expect("workspace read")
            .expect("workspace row")
            .revision,
        3
    );
}

#[test]
fn provider_failure_leaves_durable_state_unchanged_and_status_diff_are_read_only() {
    let mut fixture = fixture();
    let before = current(&fixture.repository);
    let provider = TestProvider { succeeds: false };
    let error = provider
        .execute()
        .expect_err("TEST_DOUBLE provider failure");
    assert_eq!(error.code(), "IO_ERROR");
    assert_eq!(current(&fixture.repository), before);

    let status = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .status("ws-integration", 1)
        .expect("status");
    assert_eq!(status.revision, before.revision);
    assert_eq!(current(&fixture.repository), before);
    let diff_error = WorkspaceManager::new(&mut fixture.repository, Redactor::default())
        .diff_workspace("ws-integration")
        .expect_err("no snapshot diff");
    assert_eq!(diff_error.code(), "INTEGRITY_ERROR");
    assert_eq!(current(&fixture.repository), before);
}

#[test]
fn no_lifecycle_transition_event_is_created_for_workspace_update() {
    let mut fixture = fixture();
    let before = fixture
        .repository
        .metadata()
        .list_event_envelopes("project-integration", 0)
        .expect("events");
    transition(
        &mut fixture.repository,
        &fixture.lease,
        0,
        WorkspaceLifecycleAction::BeginCapture,
        1,
        "t1",
    )
    .expect("transition");
    let after = fixture
        .repository
        .metadata()
        .list_event_envelopes("project-integration", 0)
        .expect("events");
    assert_eq!(before, after);
}

#[test]
fn legacy_v01_fixture_opens_reads_and_rejects_missing_lifecycle_without_mutation() {
    let project = tempdir().expect("project");
    let initialized = Repository::init(project.path()).expect("repository layout");
    let metadata_path = initialized.layout().metadata_path().to_path_buf();
    drop(initialized);
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut candidate = metadata_path.as_os_str().to_os_string();
        candidate.push(suffix);
        let candidate = std::path::PathBuf::from(candidate);
        match fs::remove_file(candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("remove initialized metadata: {error}"),
        }
    }
    let connection = Connection::open(&metadata_path).expect("legacy metadata");
    connection
        .execute_batch(include_str!("fixtures/CT-09-v01-metadata.sql"))
        .expect("legacy fixture");
    drop(connection);
    let mut repository = Repository::open(project.path()).expect("legacy open");
    assert_eq!(
        repository
            .metadata()
            .list_events("legacy-stream")
            .expect("legacy read")
            .len(),
        1
    );
    let status = WorkspaceManager::new(&mut repository, Redactor::default())
        .status("legacy-workspace", 0)
        .expect_err("legacy has no workspace lifecycle row");
    assert_eq!(status.code(), "NOT_FOUND");
    let transition_error = transition(
        &mut repository,
        &pong_core::LeaseToken {
            workspace_id: "legacy-workspace".into(),
            agent_id: "legacy-agent".into(),
            epoch: 1,
            expires_at_ms: 10_000,
        },
        0,
        WorkspaceLifecycleAction::Open,
        0,
        "legacy",
    )
    .expect_err("missing legacy workspace");
    assert_eq!(transition_error.code(), "NOT_FOUND");
    assert!(repository
        .metadata()
        .workspace("legacy-workspace")
        .expect("read")
        .is_none());
}
