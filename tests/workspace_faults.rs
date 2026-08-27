use pong_core::cas::{Cas, CasFailPoint, CasFailpoints, CasFaultAction};
use pong_core::environment::EnvironmentFacts;
use pong_core::redaction::Redactor;
use pong_core::workspace::{
    LocalWorkspace, SnapshotOptions, WorkspaceFailPoint, WorkspaceFailpoints, WorkspaceFaultAction,
    WorkspaceManager,
};
use pong_core::{MetadataFailpoint, MetadataFailpoints, Repository};
use std::fs;
use tempfile::tempdir;

fn source_with_snapshot() -> (tempfile::TempDir, LocalWorkspace, Cas, pong_core::Snapshot) {
    let root = tempdir().expect("root");
    let workspace = LocalWorkspace::create(
        "ws-faults",
        "project-faults",
        root.path().join("source"),
        Redactor::default(),
    )
    .expect("workspace");
    fs::write(workspace.root().join("file.txt"), b"fault payload").expect("source file");
    let cas = Cas::new(root.path().join("cas")).expect("cas");
    let snapshot = workspace
        .snapshot(&cas, SnapshotOptions::default())
        .expect("snapshot");
    (root, workspace, cas, snapshot)
}

fn temporary_siblings(parent: &std::path::Path, destination: &str) -> Vec<std::path::PathBuf> {
    let prefix = format!(".{destination}.snapshot-");
    fs::read_dir(parent)
        .expect("parent")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            name.to_str()
                .filter(|name| name.starts_with(&prefix))
                .map(|_| entry.path())
        })
        .collect()
}

#[test]
fn cas_blob_and_manifest_publication_faults_leave_no_partial_object() {
    let root = tempdir().expect("root");
    let workspace = LocalWorkspace::create(
        "ws-cas-fault",
        "project-cas-fault",
        root.path().join("source"),
        Redactor::default(),
    )
    .expect("workspace");
    let cas = Cas::new(root.path().join("cas")).expect("cas");

    fs::write(workspace.root().join("file.txt"), b"first").expect("first file");
    workspace
        .snapshot(&cas, SnapshotOptions::default())
        .expect("initial snapshot");

    fs::write(workspace.root().join("file.txt"), b"second").expect("second file");
    cas.set_failpoints(CasFailpoints::once(
        CasFailPoint::Publication,
        CasFaultAction::Fail,
    ));
    let blob_error = workspace
        .snapshot(&cas, SnapshotOptions::default())
        .expect_err("blob publication fault");
    assert_eq!(blob_error.code(), "FAULT_INJECTED");
    assert_eq!(cas.failpoints().armed(), None);

    // Publish the changed blob before arming the same boundary again. The
    // next publication is therefore the tree manifest, not the file blob.
    cas.put("workspace/blob/v1", b"second")
        .expect("prepublish changed blob");
    cas.set_failpoints(CasFailpoints::once(
        CasFailPoint::Publication,
        CasFaultAction::Fail,
    ));
    let manifest_error = workspace
        .snapshot(&cas, SnapshotOptions::default())
        .expect_err("manifest publication fault");
    assert_eq!(manifest_error.code(), "FAULT_INJECTED");
    assert_eq!(cas.failpoints().armed(), None);
    assert_eq!(cas.root().join("staging").read_dir().unwrap().count(), 0);
}

#[test]
fn manager_cas_failure_does_not_advance_workspace_head_or_revision() {
    let project = tempdir().expect("project");
    let external = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    let facts = EnvironmentFacts::capture(&Redactor::default(), &[]).expect("facts");
    facts
        .persist(
            repository.metadata_mut(),
            "env-fault-manager",
            "project-fault-manager",
            "t0",
        )
        .expect("environment");
    let path = external.path().join("managed");
    let (lease, before) = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws-fault-manager",
                "project-fault-manager",
                &path,
                Some("refs/heads/main"),
                Some("env-fault-manager"),
                "t0",
            )
            .expect("workspace");
        fs::write(path.join("file.txt"), b"manager payload").expect("file");
        let lease = manager
            .acquire_lease("ws-fault-manager", "agent-fault", 0, 10_000)
            .expect("lease");
        let before = manager.workspace("ws-fault-manager").unwrap().unwrap();
        (lease, before)
    };
    repository.cas().set_failpoints(CasFailpoints::once(
        CasFailPoint::Publication,
        CasFaultAction::Fail,
    ));
    let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
    let error = manager
        .snapshot_local(
            "ws-fault-manager",
            &lease,
            SnapshotOptions::default(),
            10,
            "t1",
        )
        .expect_err("CAS publication must fail");
    assert_eq!(error.code(), "FAULT_INJECTED");
    let after = manager.workspace("ws-fault-manager").unwrap().unwrap();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.head, before.head);
    assert_eq!(after.status, before.status);
}

#[test]
fn manager_head_update_commit_failure_leaves_head_and_revision_unchanged() {
    let project = tempdir().expect("project");
    let external = tempdir().expect("workspace parent");
    let mut repository = Repository::init(project.path()).expect("repository");
    let facts = EnvironmentFacts::capture(&Redactor::default(), &[]).expect("facts");
    facts
        .persist(
            repository.metadata_mut(),
            "env-metadata-fault",
            "project-metadata-fault",
            "t0",
        )
        .expect("environment");
    let path = external.path().join("managed");
    let (lease, before) = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws-metadata-fault",
                "project-metadata-fault",
                &path,
                Some("refs/heads/main"),
                Some("env-metadata-fault"),
                "t0",
            )
            .expect("workspace");
        fs::write(path.join("file.txt"), b"metadata payload").expect("file");
        let lease = manager
            .acquire_lease("ws-metadata-fault", "agent-metadata", 0, 10_000)
            .expect("lease");
        let before = manager.workspace("ws-metadata-fault").unwrap().unwrap();
        (lease, before)
    };
    repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::BeforeSqliteCommit,
        ));
    let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
    let error = manager
        .snapshot_local(
            "ws-metadata-fault",
            &lease,
            SnapshotOptions::default(),
            10,
            "t1",
        )
        .expect_err("metadata head update must fail");
    assert_eq!(error.code(), "FAULT_INJECTED");
    let after = manager.workspace("ws-metadata-fault").unwrap().unwrap();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.head, before.head);
    assert_eq!(after.status, before.status);
}

#[test]
fn materialization_write_sync_tree_sync_and_rename_failures_clean_temporary_only() {
    let (root, workspace, cas, snapshot) = source_with_snapshot();
    let cases = [
        WorkspaceFailPoint::MaterializeFileWrite,
        WorkspaceFailPoint::MaterializeFileSync,
        WorkspaceFailPoint::MaterializeTreeDirectorySync,
        WorkspaceFailPoint::MaterializeRename,
    ];
    for (index, point) in cases.into_iter().enumerate() {
        let name = format!("destination-{index}");
        let destination = root.path().join(&name);
        workspace.set_failpoints(WorkspaceFailpoints::once(point, WorkspaceFaultAction::Fail));
        let error = workspace
            .materialize(&cas, snapshot.digest, &destination)
            .expect_err("materialization fault");
        assert_eq!(error.code(), "FAULT_INJECTED", "{point:?}: {error:?}");
        assert_eq!(workspace.failpoints().armed(), None);
        assert!(!destination.exists(), "{point:?} published destination");
        assert!(
            temporary_siblings(root.path(), &name).is_empty(),
            "{point:?} left temp"
        );
    }
}

#[test]
fn materialization_fault_actions_have_stable_domain_codes() {
    let (root, workspace, cas, snapshot) = source_with_snapshot();
    let cases = [
        (WorkspaceFaultAction::PermissionDenied, "PERMISSION_DENIED"),
        (
            WorkspaceFaultAction::ResourceExhausted,
            "RESOURCE_EXHAUSTED",
        ),
        (WorkspaceFaultAction::ShortWrite(2), "FAULT_INJECTED"),
    ];
    for (index, (action, code)) in cases.into_iter().enumerate() {
        let name = format!("action-destination-{index}");
        let destination = root.path().join(&name);
        workspace.set_failpoints(WorkspaceFailpoints::once(
            WorkspaceFailPoint::MaterializeFileWrite,
            action,
        ));
        let error = workspace
            .materialize(&cas, snapshot.digest, &destination)
            .expect_err("materialization action fault");
        assert_eq!(error.code(), code, "{action:?}: {error:?}");
        assert!(!destination.exists());
        assert!(temporary_siblings(root.path(), &name).is_empty());
    }
}

#[test]
fn parent_sync_failure_is_outcome_unknown_and_preserves_published_destination() {
    let (root, workspace, cas, snapshot) = source_with_snapshot();
    let destination = root.path().join("parent-sync-destination");
    workspace.set_failpoints(WorkspaceFailpoints::once(
        WorkspaceFailPoint::MaterializeParentDirectorySync,
        WorkspaceFaultAction::Fail,
    ));
    let error = workspace
        .materialize(&cas, snapshot.digest, &destination)
        .expect_err("parent sync fault");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert!(destination.is_dir(), "published destination was removed");
    assert_eq!(
        fs::read(destination.join("file.txt")).unwrap(),
        b"fault payload"
    );
    assert!(temporary_siblings(root.path(), "parent-sync-destination").is_empty());
}

#[test]
fn orphan_materialization_cleanup_is_explicit_and_reopen_safe() {
    let (root, workspace, _cas, _snapshot) = source_with_snapshot();
    let workspace_path = workspace.root().to_path_buf();
    drop(workspace);
    let workspace = LocalWorkspace::open(
        "ws-faults",
        "project-faults",
        workspace_path,
        Redactor::default(),
    )
    .expect("cold reopen workspace");
    let destination = root.path().join("reopen-destination");
    let orphan = root.path().join(".reopen-destination.snapshot-crashed");
    fs::create_dir(&orphan).expect("orphan");
    fs::write(orphan.join("partial.txt"), b"partial").expect("partial");

    let removed = workspace
        .cleanup_orphan_materializations(&destination)
        .expect("cleanup");
    assert_eq!(removed, 1);
    assert!(!orphan.exists());
    assert!(!destination.exists());

    fs::create_dir(&orphan).expect("second orphan");
    workspace.set_failpoints(WorkspaceFailpoints::once(
        WorkspaceFailPoint::CleanupTemporary,
        WorkspaceFaultAction::Fail,
    ));
    let error = workspace
        .cleanup_orphan_materializations(&destination)
        .expect_err("injected cleanup fault");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert!(
        orphan.is_dir(),
        "failed cleanup must retain orphan for retry"
    );
    assert_eq!(workspace.failpoints().armed(), None);
    assert_eq!(
        workspace
            .cleanup_orphan_materializations(&destination)
            .expect("retry cleanup"),
        1
    );
}

#[test]
fn materialization_cleanup_failure_reports_recovery_required_and_retains_orphan() {
    let (root, workspace, cas, snapshot) = source_with_snapshot();
    let destination = root.path().join("cleanup-failure");
    let manifest = workspace
        .read_manifest(&cas, snapshot.digest)
        .expect("manifest");
    let blob = manifest
        .entries
        .iter()
        .find(|entry| entry.kind == "file")
        .and_then(|entry| entry.digest.as_deref())
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .and_then(|digest| pong_core::cas::Digest::from_hex(digest).ok())
        .expect("blob digest");
    fs::remove_file(cas.object_path(blob)).expect("remove blob for fault");
    workspace.set_failpoints(WorkspaceFailpoints::once(
        WorkspaceFailPoint::CleanupTemporary,
        WorkspaceFaultAction::Fail,
    ));
    let error = workspace
        .materialize(&cas, snapshot.digest, &destination)
        .expect_err("missing blob");
    assert_eq!(error.code(), "RECOVERY_REQUIRED");
    let orphans = temporary_siblings(root.path(), "cleanup-failure");
    assert_eq!(orphans.len(), 1);
    workspace.set_failpoints(WorkspaceFailpoints::disabled());
    assert_eq!(
        workspace
            .cleanup_orphan_materializations(&destination)
            .expect("cleanup retained orphan"),
        1
    );
    assert!(!destination.exists());
}
