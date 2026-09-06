use pong_core::canonical::canonical_bytes;
use pong_core::environment::EnvironmentFacts;
use pong_core::metadata::{WorkspaceRecord, WorkspaceUpdate};
use pong_core::redaction::Redactor;
use pong_core::workspace::{
    LocalWorkspace, SnapshotOptions, TreeEntry, TreeManifest, WorkspaceManager,
};
use pong_core::{MetadataFailpoint, MetadataFailpoints, PongError, Repository};
use std::fs;
use tempfile::tempdir;

fn project() -> tempfile::TempDir {
    tempdir().expect("project")
}

fn workspace_record(id: &str) -> WorkspaceRecord {
    WorkspaceRecord {
        workspace_id: id.into(),
        project_id: "project-test".into(),
        driver: "local".into(),
        locator: "C:/workspace-test".into(),
        branch_ref: Some("refs/heads/main".into()),
        head: None,
        version_head_id: None,
        environment_id: None,
        status: "created".into(),
        revision: 0,
        created_at: "t0".into(),
        updated_at: "t0".into(),
    }
}

#[test]
fn lease_epoch_and_workspace_revision_reject_stale_writers() {
    let directory = project();
    let path = directory.path().join("metadata.sqlite");
    let mut metadata = pong_core::metadata::MetadataStore::open(path).expect("metadata");
    metadata
        .create_workspace(&workspace_record("ws-1"))
        .expect("workspace");
    let first = metadata
        .acquire_workspace_lease("ws-1", "agent-a", 100, 100)
        .expect("lease");
    assert!(matches!(
        metadata.acquire_workspace_lease("ws-1", "agent-b", 150, 100),
        Err(PongError::Conflict(_))
    ));
    let updated = metadata
        .update_workspace(WorkspaceUpdate {
            workspace_id: "ws-1",
            expected_revision: 0,
            lease: &first,
            branch_ref: Some("refs/heads/main"),
            head: Some("sha256:head-1"),
            environment_id: None,
            status: "preparing",
            updated_at: "t1",
            now_ms: 150,
        })
        .expect("workspace update");
    assert_eq!(updated.revision, 1);
    assert!(matches!(
        metadata.update_workspace(WorkspaceUpdate {
            workspace_id: "ws-1",
            expected_revision: 0,
            lease: &first,
            branch_ref: Some("refs/heads/main"),
            head: Some("sha256:head-stale"),
            environment_id: None,
            status: "preparing",
            updated_at: "t2",
            now_ms: 160,
        }),
        Err(PongError::Conflict(_))
    ));
    metadata
        .release_workspace_lease(&first, 170)
        .expect("release");
    let second = metadata
        .acquire_workspace_lease("ws-1", "agent-b", 180, 100)
        .expect("takeover");
    assert!(second.epoch > first.epoch);
    assert!(matches!(
        metadata.renew_workspace_lease(&first, 190, 100),
        Err(PongError::Conflict(_))
    ));
}

#[test]
fn local_snapshot_is_canonical_and_materializes_without_partial_destination() {
    let project = project();
    let workspace_root = tempdir().expect("workspace root");
    let cas_root = project.path().join("cas");
    let cas = pong_core::cas::Cas::new(cas_root).expect("cas");
    let workspace = LocalWorkspace::create(
        "ws-snapshot",
        "project-test",
        workspace_root.path().join("source"),
        Redactor::default(),
    )
    .expect("workspace");
    fs::create_dir(workspace.root().join("nested")).expect("nested");
    fs::write(workspace.root().join("nested").join("b.txt"), b"beta").expect("file b");
    fs::write(workspace.root().join("a.txt"), b"alpha").expect("file a");
    let first = workspace
        .snapshot(&cas, SnapshotOptions::default())
        .expect("snapshot");
    let second = workspace
        .snapshot(&cas, SnapshotOptions::default())
        .expect("repeat snapshot");
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.file_count, 2);
    assert_eq!(first.total_bytes, 9);
    let manifest = workspace
        .read_manifest(&cas, first.digest)
        .expect("manifest");
    assert_eq!(manifest.entries[0].path, "a.txt");
    assert!(manifest.entries.iter().any(|entry| entry.path == "nested"));

    let destination = workspace_root.path().join("materialized");
    workspace
        .materialize(&cas, first.digest, &destination)
        .expect("materialize");
    assert_eq!(fs::read(destination.join("a.txt")).unwrap(), b"alpha");
    assert_eq!(
        fs::read(destination.join("nested").join("b.txt")).unwrap(),
        b"beta"
    );
    assert!(matches!(
        workspace.materialize(&cas, first.digest, &destination),
        Err(PongError::Conflict(_))
    ));
}

#[test]
fn local_snapshot_enforces_file_count_and_single_file_size_limits() {
    let root = tempdir().expect("workspace root");
    let cas = pong_core::cas::Cas::new(root.path().join("cas")).expect("cas");

    let count_limited = LocalWorkspace::create(
        "ws-count-limit",
        "project-limits",
        root.path().join("count-limited"),
        Redactor::default(),
    )
    .expect("count-limited workspace");
    fs::write(count_limited.root().join("a.txt"), b"a").expect("first file");
    fs::write(count_limited.root().join("b.txt"), b"b").expect("second file");
    let count_error = count_limited
        .snapshot(
            &cas,
            SnapshotOptions {
                max_files: 1,
                max_file_bytes: 1,
            },
        )
        .expect_err("second file must exceed the file-count limit");
    assert!(matches!(count_error, PongError::ResourceExhausted(_)));

    let size_limited = LocalWorkspace::create(
        "ws-size-limit",
        "project-limits",
        root.path().join("size-limited"),
        Redactor::default(),
    )
    .expect("size-limited workspace");
    fs::write(size_limited.root().join("large.bin"), b"12345").expect("large file");
    let size_error = size_limited
        .snapshot(
            &cas,
            SnapshotOptions {
                max_files: 1,
                max_file_bytes: 4,
            },
        )
        .expect_err("file must exceed the single-file size limit");
    assert!(matches!(size_error, PongError::ResourceExhausted(_)));
}

#[test]
fn workspace_manager_keeps_physical_path_outside_pong_and_advances_head_under_lease() {
    let project = project();
    let workspace_root = tempdir().expect("workspace root");
    let mut repository = Repository::init(project.path()).expect("repository");
    let facts = EnvironmentFacts::capture(&Redactor::default(), &[]).expect("environment facts");
    facts
        .persist(
            repository.metadata_mut(),
            "env-manager",
            "project-manager",
            "t0",
        )
        .expect("persist environment");
    let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
    let path = workspace_root.path().join("managed");
    let record = manager
        .create_local(
            "ws-manager",
            "project-manager",
            &path,
            Some("refs/heads/main"),
            Some("env-manager"),
            "t0",
        )
        .expect("create local workspace");
    assert_eq!(record.driver, "local");
    assert!(!record.locator.contains(".pong"));
    fs::write(path.join("README.md"), b"hello").expect("write workspace");
    let lease = manager
        .acquire_lease("ws-manager", "agent-manager", 0, 1_000)
        .expect("lease");
    let snapshot = manager
        .snapshot_local("ws-manager", &lease, SnapshotOptions::default(), 10, "t1")
        .expect("snapshot through manager");
    let updated = manager.workspace("ws-manager").unwrap().unwrap();
    assert_eq!(updated.revision, 1);
    assert_eq!(updated.status, "ready");
    let expected_head = format!("sha256:{}", snapshot.digest);
    assert_eq!(updated.head.as_deref(), Some(expected_head.as_str()));
}

#[test]
fn registered_secret_blocks_manager_snapshot_without_advancing_workspace() {
    let project = project();
    let workspace_root = tempdir().expect("workspace root");
    let secret = "workspace-secret-value";
    let mut redactor = Redactor::default();
    redactor.register_secret(secret).expect("register secret");
    let mut repository =
        Repository::init_with_redactor(project.path(), redactor.clone()).expect("repository");
    let facts = EnvironmentFacts::capture(&redactor, &[]).expect("environment facts");
    facts
        .persist(
            repository.metadata_mut(),
            "env-secret",
            "project-secret",
            "t0",
        )
        .expect("persist environment");

    let mut manager = WorkspaceManager::new(&mut repository, redactor);
    let path = workspace_root.path().join("managed-secret");
    manager
        .create_local(
            "ws-secret",
            "project-secret",
            &path,
            Some("refs/heads/main"),
            Some("env-secret"),
            "t0",
        )
        .expect("create workspace");
    fs::write(path.join("credentials.txt"), format!("token={secret}"))
        .expect("write secret-bearing file");
    let lease = manager
        .acquire_lease("ws-secret", "agent-secret", 0, 1_000)
        .expect("lease");
    let before = manager.workspace("ws-secret").unwrap().unwrap();

    let error = manager
        .snapshot_local("ws-secret", &lease, SnapshotOptions::default(), 10, "t1")
        .expect_err("registered secret must block snapshot publication");
    assert!(matches!(error, PongError::Integrity(_)));
    assert!(!error.to_string().contains(secret));

    let after = manager.workspace("ws-secret").unwrap().unwrap();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.head, before.head);
    assert_eq!(after.status, before.status);
}

#[test]
fn workspace_update_rejects_missing_and_wrong_project_environment_bindings() {
    let directory = project();
    let path = directory.path().join("metadata.sqlite");
    let mut metadata = pong_core::metadata::MetadataStore::open(path).expect("metadata");
    metadata
        .create_workspace(&workspace_record("ws-environment"))
        .expect("workspace");
    let lease = metadata
        .acquire_workspace_lease("ws-environment", "agent-environment", 0, 1_000)
        .expect("lease");

    let missing = metadata
        .update_workspace(WorkspaceUpdate {
            workspace_id: "ws-environment",
            expected_revision: 0,
            lease: &lease,
            branch_ref: Some("refs/heads/main"),
            head: Some("sha256:head-missing"),
            environment_id: Some("env-missing"),
            status: "preparing",
            updated_at: "t1",
            now_ms: 10,
        })
        .expect_err("missing environment must be rejected");
    assert!(matches!(missing, PongError::Conflict(_)));

    metadata
        .record_environment(
            "env-other-project",
            "project-other",
            &serde_json::json!({"schema_version": 1}),
            "t0",
        )
        .expect("other-project environment");
    let wrong_project = metadata
        .update_workspace(WorkspaceUpdate {
            workspace_id: "ws-environment",
            expected_revision: 0,
            lease: &lease,
            branch_ref: Some("refs/heads/main"),
            head: Some("sha256:head-wrong-project"),
            environment_id: Some("env-other-project"),
            status: "preparing",
            updated_at: "t2",
            now_ms: 20,
        })
        .expect_err("environment from another project must be rejected");
    assert!(matches!(wrong_project, PongError::Conflict(_)));

    let unchanged = metadata.workspace("ws-environment").unwrap().unwrap();
    assert_eq!(unchanged.revision, 0);
    assert_eq!(unchanged.head, None);
    assert_eq!(unchanged.environment_id, None);
}

#[test]
fn workspace_creation_rejects_missing_or_wrong_project_environment_without_leaking_directory() {
    let project = project();
    let workspace_root = tempdir().expect("workspace root");
    let mut repository = Repository::init(project.path()).expect("repository");

    let missing_path = workspace_root.path().join("missing-environment");
    let missing_error = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws-create-missing",
                "project-create",
                &missing_path,
                None,
                Some("env-does-not-exist"),
                "t0",
            )
            .expect_err("missing environment must reject creation")
    };
    assert!(matches!(missing_error, PongError::Conflict(_)));
    assert!(!missing_path.exists());
    assert!(repository
        .metadata()
        .workspace("ws-create-missing")
        .unwrap()
        .is_none());

    repository
        .metadata_mut()
        .record_environment(
            "env-create-other",
            "project-other",
            &serde_json::json!({"schema_version": 1}),
            "t0",
        )
        .expect("other environment");
    let wrong_path = workspace_root.path().join("wrong-environment");
    let wrong_error = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws-create-wrong",
                "project-create",
                &wrong_path,
                None,
                Some("env-create-other"),
                "t0",
            )
            .expect_err("cross-project environment must reject creation")
    };
    assert!(matches!(wrong_error, PongError::Conflict(_)));
    assert!(!wrong_path.exists());
    assert!(repository
        .metadata()
        .workspace("ws-create-wrong")
        .unwrap()
        .is_none());
}

#[test]
fn expired_lease_takeover_and_workspace_records_survive_cold_reopen() {
    let project = project();
    let (workspace, environment, first) = {
        let mut repository = Repository::init(project.path()).expect("repository");
        let environment = repository
            .metadata_mut()
            .record_environment(
                "env-persistent",
                "project-test",
                &serde_json::json!({"schema_version": 1, "os": "test"}),
                "t0",
            )
            .expect("environment");
        let mut record = workspace_record("ws-persistent");
        record.environment_id = Some(environment.environment_id.clone());
        let workspace = repository
            .metadata_mut()
            .create_workspace(&record)
            .expect("workspace");
        let first = repository
            .metadata_mut()
            .acquire_workspace_lease("ws-persistent", "agent-a", 100, 50)
            .expect("first lease");
        (workspace, environment, first)
    };

    let second = {
        let mut reopened = Repository::open(project.path()).expect("cold reopen");
        assert_eq!(
            reopened.metadata().workspace("ws-persistent").unwrap(),
            Some(workspace.clone())
        );
        assert_eq!(
            reopened.metadata().environment("env-persistent").unwrap(),
            Some(environment.clone())
        );
        let second = reopened
            .metadata_mut()
            .acquire_workspace_lease("ws-persistent", "agent-b", 151, 50)
            .expect("take over expired lease");
        assert!(second.epoch > first.epoch);
        second
    };

    let mut reopened = Repository::open(project.path()).expect("second cold reopen");
    assert!(matches!(
        reopened
            .metadata_mut()
            .acquire_workspace_lease("ws-persistent", "agent-c", 175, 50),
        Err(PongError::Conflict(_))
    ));
    assert_eq!(second.agent_id, "agent-b");
}

#[cfg(unix)]
#[test]
fn workspace_snapshot_rejects_symlinks() {
    let root = tempdir().expect("workspace root");
    let workspace = LocalWorkspace::create(
        "ws-safe",
        "project-safe",
        root.path().join("source"),
        Redactor::default(),
    )
    .expect("workspace");
    std::os::unix::fs::symlink(root.path().join("outside"), workspace.root().join("link"))
        .expect("symlink");
    let cas = pong_core::cas::Cas::new(root.path().join("cas")).unwrap();
    assert!(matches!(
        workspace.snapshot(&cas, SnapshotOptions::default()),
        Err(PongError::Integrity(_))
    ));
}

#[test]
fn environment_facts_never_persist_sensitive_allowlist_keys() {
    let redactor = Redactor::default();
    let facts =
        EnvironmentFacts::capture(&redactor, &["PATH", "PONG_API_TOKEN", "HOME"]).expect("facts");
    assert!(facts.variables.is_empty());
    let bytes = facts.canonical_bytes().unwrap();
    assert!(!bytes.windows(9).any(|window| window == b"API_TOKEN"));
}

#[test]
fn malformed_tree_paths_fail_closed_before_materialization() {
    let root = tempdir().expect("workspace root");
    let workspace = LocalWorkspace::create(
        "ws-manifest-validation",
        "project-manifest",
        root.path().join("source"),
        Redactor::default(),
    )
    .expect("workspace");
    let cas = pong_core::cas::Cas::new(root.path().join("cas")).expect("cas");

    for path in ["../escape", "CON.txt", "nested\\file.txt", "/absolute"] {
        let manifest = TreeManifest {
            manifest_version: 1,
            workspace_id: "ws-manifest-validation".into(),
            project_id: "project-manifest".into(),
            entries: vec![TreeEntry {
                path: path.into(),
                kind: "directory".into(),
                size: 0,
                digest: None,
            }],
            redaction_profile_id: "default".into(),
            redaction_profile_version: "0.1".into(),
        };
        let bytes = canonical_bytes(&serde_json::to_value(&manifest).expect("manifest value"))
            .expect("canonical manifest");
        let digest = cas
            .put("workspace/tree/v1", &bytes)
            .expect("manifest object");
        let error = workspace
            .read_manifest(&cas, digest)
            .expect_err("unsafe path must be rejected");
        assert!(
            matches!(error, PongError::Integrity(_)),
            "{path}: {error:?}"
        );
    }
}

#[test]
fn post_commit_workspace_creation_error_keeps_the_committed_materialization() {
    let project = project();
    let workspace_root = tempdir().expect("workspace root");
    let mut repository = Repository::init(project.path()).expect("repository");
    repository
        .metadata_mut()
        .set_failpoints(MetadataFailpoints::once(
            MetadataFailpoint::AfterSqliteCommit,
        ));
    let path = workspace_root.path().join("post-commit");
    let error = {
        let mut manager = WorkspaceManager::new(&mut repository, Redactor::default());
        manager
            .create_local(
                "ws-post-commit",
                "project-post-commit",
                &path,
                None,
                None,
                "t0",
            )
            .expect_err("after-commit fault is reported as unconfirmed")
    };
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert!(
        path.is_dir(),
        "committed metadata must retain its directory"
    );
    assert!(repository
        .metadata()
        .workspace("ws-post-commit")
        .unwrap()
        .is_some());
}

#[cfg(unix)]
#[test]
fn local_driver_rejects_reparse_ancestors_for_create_and_materialize() {
    let root = tempdir().expect("workspace root");
    let outside = tempdir().expect("outside");
    let linked_parent = root.path().join("linked-parent");
    std::os::unix::fs::symlink(outside.path(), &linked_parent).expect("parent symlink");
    let error = LocalWorkspace::create(
        "ws-linked-parent",
        "project-linked-parent",
        linked_parent.join("workspace"),
        Redactor::default(),
    )
    .expect_err("symlink ancestor must be rejected");
    assert!(matches!(error, PongError::Integrity(_)));

    let source = LocalWorkspace::create(
        "ws-materialize-parent",
        "project-linked-parent",
        root.path().join("source"),
        Redactor::default(),
    )
    .expect("source workspace");
    fs::write(source.root().join("file.txt"), b"content").expect("source file");
    let cas = pong_core::cas::Cas::new(root.path().join("cas")).expect("cas");
    let snapshot = source
        .snapshot(&cas, SnapshotOptions::default())
        .expect("snapshot");
    let destination_parent = root.path().join("destination-link");
    std::os::unix::fs::symlink(outside.path(), &destination_parent)
        .expect("destination parent symlink");
    let error = source
        .materialize(
            &cas,
            snapshot.digest,
            destination_parent.join("materialized"),
        )
        .expect_err("materialization must reject symlink parent");
    assert!(matches!(error, PongError::Integrity(_)));
    assert!(!outside.path().join("materialized").exists());
}
