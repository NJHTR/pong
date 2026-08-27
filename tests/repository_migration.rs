use pong_core::metadata::{IdempotencyKey, IdempotencyResult, MetadataStore, NewEvent};
use pong_core::redaction::Redactor;
use pong_core::repository::{
    MigrationFailpoint, MigrationFailpoints, MigrationSpec, Repository, RepositoryMarker,
    GENERATION_REPOSITORY_FORMAT, REPOSITORY_FORMAT, REPOSITORY_MARKER_VERSION,
};
use pong_core::PongError;
use serde_json::json;
use std::fs;
use tempfile::{tempdir, TempDir};

fn spec() -> MigrationSpec {
    MigrationSpec::new("migration-0002", "generation-0002")
}

fn idempotency_key() -> IdempotencyKey {
    IdempotencyKey {
        project_id: "project-migration".into(),
        actor_id: "agent-migration".into(),
        request_id: "request-migration".into(),
    }
}

fn legacy_repository() -> TempDir {
    let project = tempdir().expect("project");
    let mut repository = Repository::init(project.path()).expect("legacy init");
    repository
        .metadata_mut()
        .append_event(NewEvent {
            event_id: "event-migration-1".into(),
            project_id: "project-migration".into(),
            stream_id: "stream-migration".into(),
            schema_version: "0.1".into(),
            actor_id: Some("agent-migration".into()),
            request_id: Some("request-migration".into()),
            occurred_at: "2026-08-20T00:00:00Z".into(),
            payload: json!({
                "type": "legacy.checkpoint",
                "opaque_extension": {"provider": "keep-this"}
            }),
        })
        .expect("event");
    repository
        .metadata_mut()
        .compare_and_swap_ref("refs/heads/main", None, "commit-migration-1", "t1")
        .expect("ref");
    assert_eq!(
        repository
            .metadata_mut()
            .record_idempotency(
                &idempotency_key(),
                "sha256:migration",
                &json!({"ok": true}),
                "t1"
            )
            .expect("idempotency"),
        IdempotencyResult::NewlyRecorded
    );
    repository
        .metadata_mut()
        .record_intent(
            "project-migration",
            "agent-migration",
            "request-journal-migration",
            "operation-migration",
            &json!({"action": "write"}),
            "t1",
        )
        .expect("journal intent");
    drop(repository);
    project
}

#[test]
fn legacy_migration_builds_a_generation_and_preserves_durable_state() {
    let project = legacy_repository();
    let outcome = Repository::migrate(project.path(), spec()).expect("migration");
    assert!(!outcome.already_active);

    let legacy_metadata = project.path().join(".pong/metadata.sqlite");
    assert!(legacy_metadata.is_file(), "legacy source remains available");
    let mut legacy_writer = MetadataStore::open(&legacy_metadata).expect("legacy source reader");
    legacy_writer
        .compare_and_swap_ref(
            "refs/heads/main",
            Some("commit-migration-1"),
            "legacy-writer-must-not-win",
            "t-legacy",
        )
        .expect("legacy source mutation");
    drop(legacy_writer);
    let repository = Repository::open(project.path()).expect("open active generation");
    assert_eq!(repository.active_generation_id(), Some("generation-0002"));
    assert_eq!(
        repository.marker().repository_format,
        GENERATION_REPOSITORY_FORMAT
    );
    assert!(repository
        .active_metadata_path()
        .ends_with("generations/generation-0002/metadata.sqlite"));
    assert_eq!(
        repository
            .metadata()
            .get_ref("refs/heads/main")
            .expect("ref"),
        Some("commit-migration-1".into())
    );
    let events = repository
        .metadata()
        .list_events("stream-migration")
        .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_id, "event-migration-1");
    assert!(events[0].payload_json.contains("opaque_extension"));
    assert_eq!(
        repository
            .metadata()
            .operation(&IdempotencyKey {
                project_id: "project-migration".into(),
                actor_id: "agent-migration".into(),
                request_id: "request-journal-migration".into(),
            })
            .expect("operation")
            .expect("journal copied")
            .operation_id,
        "operation-migration"
    );
    drop(repository);

    // A post-migration append proves that startup does not compare the live,
    // mutable SQLite bytes to the backup attestation in generation.json.
    let mut repository = Repository::open(project.path()).expect("reopen active generation");
    repository
        .metadata_mut()
        .append_event(NewEvent {
            event_id: "event-migration-2".into(),
            project_id: "project-migration".into(),
            stream_id: "stream-migration".into(),
            schema_version: "0.1".into(),
            actor_id: None,
            request_id: None,
            occurred_at: "2026-08-20T00:00:01Z".into(),
            payload: json!({"type": "post-migration"}),
        })
        .expect("new generation event");
    drop(repository);

    let repository = Repository::open(project.path()).expect("cold reopen after append");
    assert_eq!(
        repository
            .metadata()
            .list_events("stream-migration")
            .expect("events")
            .len(),
        2
    );
    drop(repository);

    let mut repository = Repository::open(project.path()).expect("idempotency reopen");
    assert_eq!(
        repository
            .metadata_mut()
            .record_idempotency(
                &idempotency_key(),
                "sha256:migration",
                &json!({"ok": true}),
                "t2"
            )
            .expect("same idempotency"),
        IdempotencyResult::Existing(json!({"ok": true}))
    );
}

#[test]
fn migration_failpoints_expose_only_legacy_or_active_generation_after_cold_restart() {
    let old_only = [
        MigrationFailpoint::Preflight,
        MigrationFailpoint::TargetAllocated,
        MigrationFailpoint::AfterBackup,
        MigrationFailpoint::AfterCheckpoint,
        MigrationFailpoint::AfterVerify,
        MigrationFailpoint::BeforeSelectorReplace,
    ];
    for point in old_only {
        let project = legacy_repository();
        let error = Repository::migrate_with_failpoints(
            project.path(),
            spec(),
            MigrationFailpoints::once(point),
        )
        .expect_err("injected migration fault");
        assert_eq!(error.code(), "FAULT_INJECTED", "{point:?}");
        let repository = Repository::open(project.path()).expect("legacy cold reopen");
        assert_eq!(repository.active_generation_id(), None, "{point:?}");
        assert_eq!(repository.marker().repository_format, "0.1", "{point:?}");
    }

    let new_only = [
        MigrationFailpoint::AfterSelectorReplace,
        MigrationFailpoint::AfterSelectorDirectorySync,
        MigrationFailpoint::AfterJournalFinalize,
    ];
    for point in new_only {
        let project = legacy_repository();
        let error = Repository::migrate_with_failpoints(
            project.path(),
            spec(),
            MigrationFailpoints::once(point),
        )
        .expect_err("injected migration fault");
        assert_eq!(error.code(), "FAULT_INJECTED", "{point:?}");
        let repository = Repository::open(project.path()).expect("active cold reopen");
        assert_eq!(
            repository.active_generation_id(),
            Some("generation-0002"),
            "{point:?}"
        );
        assert_eq!(
            repository.marker().repository_format,
            GENERATION_REPOSITORY_FORMAT,
            "{point:?}"
        );
        drop(repository);
        let retry = Repository::migrate(project.path(), spec()).expect("idempotent retry");
        assert!(retry.already_active, "{point:?}");
    }
}

#[test]
fn changed_source_rejects_retry_and_does_not_reuse_an_unpublished_target() {
    let project = legacy_repository();
    let error = Repository::migrate_with_failpoints(
        project.path(),
        spec(),
        MigrationFailpoints::once(MigrationFailpoint::AfterVerify),
    )
    .expect_err("fault after verified target");
    assert_eq!(error.code(), "FAULT_INJECTED");

    let mut legacy = Repository::open(project.path()).expect("legacy remains visible");
    legacy
        .metadata_mut()
        .compare_and_swap_ref(
            "refs/heads/main",
            Some("commit-migration-1"),
            "commit-written-after-fault",
            "t2",
        )
        .expect("source mutation");
    drop(legacy);

    let error = Repository::migrate(project.path(), spec()).expect_err("stale plan rejected");
    assert_eq!(error.code(), "CONFLICT");
    let legacy = Repository::open(project.path()).expect("legacy is still authoritative");
    assert_eq!(legacy.active_generation_id(), None);
    assert_eq!(
        legacy.metadata().get_ref("refs/heads/main").expect("ref"),
        Some("commit-written-after-fault".into())
    );
}

#[test]
fn corrupted_selector_or_active_manifest_fails_closed() {
    let project = legacy_repository();
    Repository::migrate(project.path(), spec()).expect("migration");
    let selector_path = project.path().join(".pong/repository.json");
    fs::write(selector_path, b"not-json").expect("corrupt selector");
    let error = Repository::open(project.path()).expect_err("selector corruption");
    assert_eq!(error.code(), "INTEGRITY_ERROR");

    let project = legacy_repository();
    Repository::migrate(project.path(), spec()).expect("migration");
    let manifest_path = project
        .path()
        .join(".pong/generations/generation-0002/generation.json");
    fs::write(manifest_path, b"not-json").expect("corrupt manifest");
    let error = Repository::open(project.path()).expect_err("manifest corruption");
    assert_eq!(error.code(), "INTEGRITY_ERROR");

    let project = legacy_repository();
    Repository::migrate(project.path(), spec()).expect("migration");
    let database_path = project
        .path()
        .join(".pong/generations/generation-0002/metadata.sqlite");
    fs::write(database_path, b"not a SQLite database").expect("corrupt active database");
    let error = Repository::open(project.path()).expect_err("database corruption");
    assert_eq!(error.code(), "INTEGRITY_ERROR");

    let project = legacy_repository();
    Repository::migrate(project.path(), spec()).expect("migration");
    let active_database = project
        .path()
        .join(".pong/generations/generation-0002/metadata.sqlite");
    let replacement = project.path().join(".pong/metadata.sqlite");
    fs::copy(replacement, active_database).expect("replace active file with legacy source");
    let error = Repository::open(project.path()).expect_err("wrong valid database");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}

#[test]
fn foreign_valid_generation_database_cannot_replace_the_active_identity() {
    let project = legacy_repository();
    Repository::migrate(project.path(), spec()).expect("primary migration");

    // Build a second, independently valid 0.2 generation.  Its SQLite file
    // has the same schema and repository format as the active target, but its
    // durable generation/migration identity belongs to another repository.
    let foreign_project = legacy_repository();
    Repository::migrate(
        foreign_project.path(),
        MigrationSpec::new("migration-foreign", "generation-foreign"),
    )
    .expect("foreign migration");

    let foreign_metadata = foreign_project
        .path()
        .join(".pong/generations/generation-foreign/metadata.sqlite");
    let active_metadata = project
        .path()
        .join(".pong/generations/generation-0002/metadata.sqlite");
    fs::copy(foreign_metadata, active_metadata).expect("replace active database");

    let error = Repository::open(project.path()).expect_err("foreign identity must fail closed");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
    assert!(error
        .to_string()
        .contains("metadata identity does not match its manifest"));
}

#[test]
fn invalid_migration_paths_are_rejected_before_the_repository_is_touched() {
    let project = legacy_repository();
    let error = Repository::migrate(
        project.path(),
        MigrationSpec::new("migration/unsafe", "generation-0002"),
    )
    .expect_err("unsafe migration id");
    assert!(matches!(error, PongError::InvalidInput(_)));
    assert!(project.path().join(".pong/metadata.sqlite").is_file());
}

#[test]
fn migration_preserves_the_redaction_profile_and_full_tree_secret_boundary() {
    let project = tempdir().expect("project");
    let secret = "migration-secret-7a218";
    let mut redactor = Redactor::new("migration-profile", "0.1").expect("profile");
    redactor.register_secret(secret).expect("secret");
    let mut repository =
        Repository::init_with_redactor(project.path(), redactor.clone()).expect("legacy init");
    repository
        .metadata_mut()
        .append_event(NewEvent {
            event_id: format!("event-{secret}"),
            project_id: "project-redaction".into(),
            stream_id: "stream-redaction".into(),
            schema_version: "0.1".into(),
            actor_id: None,
            request_id: None,
            occurred_at: "t1".into(),
            payload: json!({"token": secret}),
        })
        .expect("redacted source event");
    drop(repository);

    Repository::migrate_with_redactor(project.path(), spec(), redactor.clone()).expect("migration");
    let repository =
        Repository::open_with_redactor(project.path(), redactor).expect("generation reopen");
    let manifest = repository.generation_manifest().expect("manifest");
    assert_eq!(manifest.redaction_profile_id, "migration-profile");
    assert_eq!(manifest.redaction_profile_version, "0.1");
    drop(repository);

    assert_no_secret(project.path().join(".pong").as_path(), secret);
}

#[test]
fn legacy_v01_reader_boundary_rejects_the_generation_selector_before_mutation() {
    let project = legacy_repository();
    Repository::migrate(project.path(), spec()).expect("migration");
    let marker: RepositoryMarker = serde_json::from_slice(
        &fs::read(project.path().join(".pong/repository.json")).expect("selector"),
    )
    .expect("legacy reader parses the marker-shaped prefix");

    // This is the exact compatibility gate used by the former v0.1 reader:
    // it checks the marker identity before opening the legacy SQLite path.
    assert_ne!(marker.marker_version, REPOSITORY_MARKER_VERSION);
    assert_ne!(marker.repository_format, REPOSITORY_FORMAT);
    assert!(marker.marker_version > REPOSITORY_MARKER_VERSION);
}

fn assert_no_secret(path: &std::path::Path, secret: &str) {
    for entry in fs::read_dir(path).expect("owned directory") {
        let path = entry.expect("directory entry").path();
        let metadata = fs::symlink_metadata(&path).expect("metadata");
        if metadata.is_dir() {
            assert_no_secret(&path, secret);
        } else if metadata.is_file()
            && path.file_name().and_then(|name| name.to_str()) != Some("repository.lock")
        {
            let bytes = fs::read(&path).expect("owned bytes");
            assert!(
                !bytes
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes()),
                "secret appeared in {}",
                path.display()
            );
        }
    }
}
