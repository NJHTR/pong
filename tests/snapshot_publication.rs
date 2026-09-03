use pong_core::metadata::{MetadataFailpoint, MetadataFailpoints};
use pong_core::redaction::Redactor;
use pong_core::repository::Repository;
use pong_core::workspace::{SnapshotOptions, WorkspaceManager};
use pong_core::LocalWorkspace;
use std::fs;
use tempfile::tempdir;

fn setup() -> (tempfile::TempDir, tempfile::TempDir, Repository) {
    let project = tempdir().expect("project");
    let workspace_root = tempdir().expect("workspace parent");
    let repository = Repository::init(project.path()).expect("repository");
    let mut repository = repository;
    repository
        .metadata_mut()
        .record_environment(
            "env-publication",
            "project-publication",
            &serde_json::json!({"schema_version": 1}),
            "t0",
        )
        .expect("environment");
    (project, workspace_root, repository)
}

fn create_workspace(
    repository: &mut Repository,
    workspace_parent: &tempfile::TempDir,
) -> (std::path::PathBuf, pong_core::LeaseToken) {
    let path = workspace_parent.path().join("workspace");
    let mut manager = WorkspaceManager::new(repository, Redactor::default());
    manager
        .create_local(
            "ws-publication",
            "project-publication",
            &path,
            None,
            Some("env-publication"),
            "t0",
        )
        .expect("workspace");
    let lease = manager
        .acquire_lease("ws-publication", "agent-publication", 0, 10_000)
        .expect("lease");
    (path, lease)
}

#[test]
fn snapshot_publication_persists_metadata_head_and_linked_event() {
    let (project, workspace_parent, mut repository) = setup();
    let (path, lease) = create_workspace(&mut repository, &workspace_parent);
    fs::write(path.join("state.txt"), b"durable state").expect("file");

    let snapshot = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local(
            "ws-publication",
            &lease,
            SnapshotOptions::default(),
            10,
            "t1",
        )
        .expect("snapshot publication");

    assert!(snapshot.snapshot_id.starts_with("snp-"));
    assert_ne!(snapshot.snapshot_id, snapshot.digest.to_string());
    let metadata = repository
        .metadata()
        .snapshot_record(&snapshot.snapshot_id)
        .expect("snapshot metadata read")
        .expect("snapshot metadata");
    assert_eq!(metadata.snapshot_id, snapshot.snapshot_id);
    assert_eq!(metadata.root_digest, format!("sha256:{}", snapshot.digest));
    assert_eq!(metadata.workspace_id, "ws-publication");
    assert_eq!(metadata.project_id, "project-publication");

    let workspace = repository
        .metadata()
        .workspace("ws-publication")
        .expect("workspace read")
        .expect("workspace row");
    assert_eq!(
        workspace.head.as_deref(),
        Some(metadata.root_digest.as_str())
    );
    assert_eq!(workspace.revision, 1);

    let operation = repository
        .metadata()
        .operation_record(&metadata.operation_id)
        .expect("operation read")
        .expect("operation row");
    assert_eq!(operation.lifecycle_status, "completed");
    assert_eq!(operation.workspace_id.as_deref(), Some("ws-publication"));
    let events = repository
        .metadata()
        .list_event_envelopes("project-publication", 0)
        .expect("events");
    let snapshot_event = events
        .iter()
        .find(|event| event.event_id == metadata.event_id)
        .expect("snapshot event");
    assert_eq!(snapshot_event.event_type, "snapshot.created");
    assert_eq!(
        snapshot_event.operation_id.as_deref(),
        Some(metadata.operation_id.as_str())
    );
    assert_eq!(
        snapshot_event.workspace_id.as_deref(),
        Some("ws-publication")
    );
    assert_eq!(
        snapshot_event.correlation_id.as_deref(),
        Some(metadata.operation_id.as_str())
    );
    assert!(snapshot_event.causation_id.is_some());
    assert!(snapshot_event.payload_json.contains(&snapshot.snapshot_id));
    let local = LocalWorkspace::open(
        "ws-publication",
        "project-publication",
        &path,
        Redactor::default(),
    )
    .expect("workspace reopen for CAS verification");
    let manifest = local
        .read_manifest(repository.cas(), snapshot.digest)
        .expect("published CAS manifest");
    assert_eq!(manifest.workspace_id, "ws-publication");
    assert!(manifest
        .entries
        .iter()
        .any(|entry| entry.path == "state.txt"));

    drop(repository);
    let reopened = Repository::open(project.path()).expect("cold reopen");
    let reopened_workspace = reopened
        .metadata()
        .workspace("ws-publication")
        .expect("workspace reopen")
        .expect("workspace row");
    let reopened_metadata = reopened
        .metadata()
        .snapshot_record(&snapshot.snapshot_id)
        .expect("snapshot metadata reopen")
        .expect("snapshot metadata row");
    assert_eq!(
        reopened_workspace.head.as_deref(),
        Some(reopened_metadata.root_digest.as_str())
    );
    let reopened_local = LocalWorkspace::open(
        "ws-publication",
        "project-publication",
        &path,
        Redactor::default(),
    )
    .expect("workspace reopen for post-restart CAS verification");
    let reopened_manifest = reopened_local
        .read_manifest(reopened.cas(), snapshot.digest)
        .expect("published CAS manifest after cold reopen");
    assert_eq!(reopened_manifest.workspace_id, "ws-publication");
    assert!(reopened_manifest
        .entries
        .iter()
        .any(|entry| entry.path == "state.txt"));
}

#[test]
fn snapshot_publication_cold_reopen_keeps_metadata_and_head_atomic() {
    let (project, workspace_parent, mut repository) = setup();
    let (path, lease) = create_workspace(&mut repository, &workspace_parent);
    fs::write(path.join("state.txt"), b"post-commit state").expect("file");
    repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterSnapshotPublicationCommit,
        ));

    let error = WorkspaceManager::new(&mut repository, Redactor::default())
        .snapshot_local(
            "ws-publication",
            &lease,
            SnapshotOptions::default(),
            10,
            "t1",
        )
        .expect_err("post-commit publication is unconfirmed");
    assert_eq!(error.code(), "FAULT_INJECTED");
    drop(repository);

    let reopened = Repository::open(project.path()).expect("cold reopen");
    let workspace = reopened
        .metadata()
        .workspace("ws-publication")
        .expect("workspace read")
        .expect("workspace row");
    let head = workspace.head.clone().expect("published head");
    let snapshot_id = reopened
        .metadata()
        .snapshot_id_for_root(&head)
        .expect("snapshot lookup")
        .expect("snapshot metadata for head");
    let metadata = reopened
        .metadata()
        .snapshot_record(&snapshot_id)
        .expect("snapshot metadata read")
        .expect("snapshot metadata");
    assert_eq!(metadata.root_digest, head);
    assert_eq!(workspace.revision, 1);
    assert!(reopened
        .metadata()
        .list_event_envelopes("project-publication", 0)
        .expect("events")
        .iter()
        .any(|event| event.event_id == metadata.event_id));
    let reopened_local = LocalWorkspace::open(
        "ws-publication",
        "project-publication",
        &path,
        Redactor::default(),
    )
    .expect("workspace reopen for post-commit CAS verification");
    reopened_local
        .read_manifest(
            reopened.cas(),
            pong_core::cas::Digest::from_hex(&head[7..]).unwrap(),
        )
        .expect("published CAS manifest after post-commit reopen");
}

#[test]
fn snapshot_publication_retry_after_post_commit_fault_is_idempotent() {
    let (project, workspace_parent, mut repository) = setup();
    let (path, lease) = create_workspace(&mut repository, &workspace_parent);
    fs::write(path.join("state.txt"), b"retryable state").expect("file");
    repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterSnapshotPublicationCommit,
        ));

    let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
    let first = manager.snapshot_local(
        "ws-publication",
        &lease,
        SnapshotOptions::default(),
        10,
        "t1",
    );
    assert_eq!(
        first.expect_err("post-commit fault").code(),
        "FAULT_INJECTED"
    );

    let retried = manager
        .snapshot_local(
            "ws-publication",
            &lease,
            SnapshotOptions::default(),
            11,
            "t1",
        )
        .expect("exact publication retry");
    assert_eq!(retried.snapshot_id, format!("snp-{}", retried.digest));
    let workspace = repository
        .metadata()
        .workspace("ws-publication")
        .expect("workspace")
        .expect("workspace row");
    let expected_head = format!("sha256:{}", retried.digest);
    assert_eq!(workspace.revision, 1);
    assert_eq!(workspace.head.as_deref(), Some(expected_head.as_str()));
    assert_eq!(
        repository
            .metadata()
            .list_event_envelopes("project-publication", 0)
            .expect("events")
            .iter()
            .filter(|event| event.event_type == "snapshot.created")
            .count(),
        1
    );
    drop(repository);
    Repository::open(project.path()).expect("cold reopen after idempotent retry");
}

#[test]
fn snapshot_publication_precommit_metadata_and_head_failures_leave_old_state() {
    for failpoint in [
        MetadataFailpoint::BeforeSnapshotMetadataInsert,
        MetadataFailpoint::BeforeSnapshotHeadUpdate,
    ] {
        let (_project, workspace_parent, mut repository) = setup();
        let (path, lease) = create_workspace(&mut repository, &workspace_parent);
        fs::write(path.join("state.txt"), b"failed publication").expect("file");
        repository
            .metadata_mut()
            .set_failpoints(MetadataFailpoints::once(failpoint));

        let error = WorkspaceManager::new(&mut repository, Redactor::default())
            .snapshot_local(
                "ws-publication",
                &lease,
                SnapshotOptions::default(),
                10,
                "t1",
            )
            .expect_err("pre-commit publication failure");
        assert_eq!(error.code(), "FAULT_INJECTED");
        let workspace = repository
            .metadata()
            .workspace("ws-publication")
            .expect("workspace read")
            .expect("workspace row");
        assert_eq!(workspace.head, None);
        assert_eq!(workspace.revision, 0);
        assert_eq!(
            repository
                .metadata()
                .list_event_envelopes("project-publication", 0)
                .expect("events")
                .iter()
                .filter(|event| event.event_type == "snapshot.created")
                .count(),
            0
        );
    }
}
