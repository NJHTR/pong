use pong_core::metadata::{
    MetadataStore, NewEventEnvelope, ProjectionDefinition, ProjectionFailpoint,
    ProjectionFailpoints, ProjectionRecord,
};
use pong_core::{MigrationSpec, PongError, Repository};
use serde_json::{json, Value};
use std::fs;
use tempfile::{tempdir, TempDir};

fn event(
    id: &str,
    value: i64,
    generation_id: Option<&str>,
    migration_id: Option<&str>,
) -> NewEventEnvelope {
    NewEventEnvelope {
        event_id: id.into(),
        project_id: "project-pt13".into(),
        stream_id: "stream-pt13".into(),
        event_type: "counter.add".into(),
        schema_version: "0.1".into(),
        occurred_at: format!("2026-08-27T00:00:{value:02}Z"),
        recorded_at: format!("2026-08-27T00:00:{value:02}Z"),
        actor_id: Some("agent-pt13".into()),
        workspace_id: None,
        task_id: None,
        operation_id: None,
        causation_id: None,
        correlation_id: Some("request-pt13".into()),
        parent_event_ids: Vec::new(),
        capture_confidence: Some("observed".into()),
        redaction_status: "clean".into(),
        generation_id: generation_id.map(str::to_owned),
        migration_id: migration_id.map(str::to_owned),
        payload: json!({"value": value}),
    }
}

fn install_counter_handler(metadata: &mut MetadataStore) {
    metadata.register_projection_handler("counter.add", |event, state| {
        let payload: Value = serde_json::from_str(&event.payload_json)
            .map_err(|error| PongError::Serialization(error.to_string()))?;
        let value = payload["value"]
            .as_i64()
            .ok_or_else(|| PongError::InvalidInput("counter event has no value".into()))?;
        let total = state["total"].as_i64().unwrap_or_default() + value;
        state["total"] = json!(total);
        Ok(())
    });
}

fn create_projection(metadata: &mut MetadataStore) {
    metadata
        .create_projection(
            ProjectionDefinition {
                projection_id: "counter".into(),
                project_id: "project-pt13".into(),
                schema_version: "0.1".into(),
                generation_id: None,
                migration_id: None,
                initial_state: json!({"total": 0}),
            },
            "projection-created",
        )
        .expect("projection");
}

fn legacy_fixture() -> TempDir {
    let project = tempdir().expect("project");
    let mut repository = Repository::init(project.path()).expect("init");
    install_counter_handler(repository.metadata_mut());
    for value in 1..=5 {
        repository
            .metadata_mut()
            .append_event_envelope(event(&format!("e{value}"), value, None, None))
            .expect("event");
    }
    create_projection(repository.metadata_mut());
    repository
        .metadata_mut()
        .apply_projection("counter", "projection-applied")
        .expect("projection apply");
    drop(repository);
    project
}

fn migration_spec(migration_id: &str, generation_id: &str) -> MigrationSpec {
    MigrationSpec::new(migration_id, generation_id)
}

fn projection_snapshot(metadata: &MetadataStore) -> (ProjectionRecord, Vec<String>) {
    let record = metadata
        .projection_record("counter")
        .expect("projection lookup")
        .expect("projection exists");
    let applied = metadata
        .projection_applied_events("counter")
        .expect("projection ledger")
        .into_iter()
        .map(|entry| format!("{}:{}", entry.event_id, entry.payload_digest))
        .collect();
    (record, applied)
}

fn assert_projection_equal(
    actual: &ProjectionRecord,
    expected: &ProjectionRecord,
    actual_ledger: &[String],
    expected_ledger: &[String],
) {
    assert_eq!(actual.project_id, expected.project_id);
    assert_eq!(actual.schema_version, expected.schema_version);
    assert_eq!(actual.state_json, expected.state_json);
    assert_eq!(actual.state_digest, expected.state_digest);
    assert_eq!(actual.cursor, expected.cursor);
    assert_eq!(actual.event_count, expected.event_count);
    assert_eq!(actual_ledger, expected_ledger);
}

#[test]
fn pt13_migration_a_to_b_preserves_projection_semantics_and_source_events() {
    let project = legacy_fixture();
    Repository::migrate(
        project.path(),
        migration_spec("migration-a", "generation-a"),
    )
    .expect("legacy to generation A");

    let mut repository = Repository::open(project.path()).expect("open A");
    install_counter_handler(repository.metadata_mut());
    let source_events = repository
        .metadata()
        .list_event_envelopes("project-pt13", 0)
        .expect("source events");
    let (a_projection, a_ledger) = projection_snapshot(repository.metadata());
    assert_eq!(a_projection.generation_id, "generation-a");
    assert_eq!(a_projection.migration_id, "migration-a");
    assert_eq!(a_projection.event_count, 5);
    assert_eq!(
        a_projection
            .cursor
            .as_ref()
            .map(|cursor| cursor.event_id.as_str()),
        Some("e5")
    );
    drop(repository);

    Repository::migrate(
        project.path(),
        migration_spec("migration-b", "generation-b"),
    )
    .expect("generation A to B");

    let mut repository = Repository::open(project.path()).expect("cold reopen B");
    install_counter_handler(repository.metadata_mut());
    let (b_projection, b_ledger) = projection_snapshot(repository.metadata());
    assert_eq!(repository.active_generation_id(), Some("generation-b"));
    assert_eq!(b_projection.generation_id, "generation-b");
    assert_eq!(b_projection.migration_id, "migration-b");
    assert_projection_equal(&b_projection, &a_projection, &b_ledger, &a_ledger);
    assert_eq!(
        repository
            .metadata()
            .list_event_envelopes("project-pt13", 0)
            .expect("B source events"),
        source_events,
        "migration must preserve immutable source envelopes"
    );
    let rebuilt = repository
        .metadata_mut()
        .rebuild_projection("counter", "projection-rebuilt")
        .expect("rebuild B");
    let rebuilt_ledger = repository
        .metadata()
        .projection_applied_events("counter")
        .expect("rebuilt ledger");
    assert_eq!(rebuilt.state_digest, a_projection.state_digest);
    assert_eq!(rebuilt.cursor, a_projection.cursor);
    assert_eq!(rebuilt.event_count, 5);
    assert_eq!(rebuilt_ledger.len(), 5);
    assert!(rebuilt_ledger
        .windows(2)
        .all(|pair| pair[0].event_id != pair[1].event_id));
}

#[test]
fn pt13_generation_assertions_fail_closed_and_matching_identity_succeeds() {
    let project = legacy_fixture();
    Repository::migrate(
        project.path(),
        migration_spec("migration-a", "generation-a"),
    )
    .expect("migration");
    let mut repository = Repository::open(project.path()).expect("open");

    let wrong_event = repository.metadata_mut().append_event_envelope(event(
        "wrong-generation",
        10,
        Some("generation-b"),
        Some("migration-b"),
    ));
    assert!(matches!(wrong_event, Err(PongError::Integrity(_))));

    repository
        .metadata_mut()
        .append_event_envelope(event(
            "matching-generation",
            10,
            Some("generation-a"),
            Some("migration-a"),
        ))
        .expect("matching identity event");

    let wrong_projection = repository.metadata_mut().create_projection(
        ProjectionDefinition {
            projection_id: "wrong-projection".into(),
            project_id: "project-pt13".into(),
            schema_version: "0.1".into(),
            generation_id: Some("generation-b".into()),
            migration_id: Some("migration-b".into()),
            initial_state: json!({}),
        },
        "t1",
    );
    assert!(matches!(wrong_projection, Err(PongError::Integrity(_))));
}

fn unprojected_fixture() -> TempDir {
    let project = tempdir().expect("project");
    let mut repository = Repository::init(project.path()).expect("init");
    install_counter_handler(repository.metadata_mut());
    for value in 1..=5 {
        repository
            .metadata_mut()
            .append_event_envelope(event(&format!("e{value}"), value, None, None))
            .expect("event");
    }
    create_projection(repository.metadata_mut());
    drop(repository);
    project
}

#[test]
fn fi10_projection_failpoint_schedule_converges_to_clean_replay_after_reopen() {
    let points = [
        ProjectionFailpoint::BeforeApply,
        ProjectionFailpoint::AfterValidation,
        ProjectionFailpoint::AfterIdempotencyCheck,
        ProjectionFailpoint::AfterStateMutation,
        ProjectionFailpoint::BeforeCursorUpdate,
        ProjectionFailpoint::AfterCursorUpdate,
        ProjectionFailpoint::BeforeLedgerCommit,
        ProjectionFailpoint::AfterLedgerCommit,
        ProjectionFailpoint::BeforeTransactionCommit,
        ProjectionFailpoint::AfterTransactionCommit,
    ];

    let clean_project = unprojected_fixture();
    let mut clean = Repository::open(clean_project.path()).expect("clean open");
    install_counter_handler(clean.metadata_mut());
    let clean_projection = clean
        .metadata_mut()
        .apply_projection("counter", "clean")
        .expect("clean replay");
    let clean_ledger = clean
        .metadata()
        .projection_applied_events("counter")
        .expect("clean ledger");
    let clean_ledger: Vec<String> = clean_ledger
        .into_iter()
        .map(|entry| format!("{}:{}", entry.event_id, entry.payload_digest))
        .collect();

    for point in points {
        let project = unprojected_fixture();
        let mut repository = Repository::open(project.path()).expect("fault open");
        install_counter_handler(repository.metadata_mut());
        repository
            .metadata_mut()
            .set_projection_failpoints(ProjectionFailpoints::once(point));
        let result = repository
            .metadata_mut()
            .apply_projection("counter", "faulted");
        assert!(
            matches!(result, Err(PongError::FaultInjected(_))),
            "{point:?}"
        );
        drop(repository);

        let mut reopened = Repository::open(project.path()).expect("cold reopen");
        install_counter_handler(reopened.metadata_mut());
        reopened
            .metadata_mut()
            .set_projection_failpoints(ProjectionFailpoints::disabled());
        let recovered = reopened
            .metadata_mut()
            .rebuild_projection("counter", "recovered")
            .expect("rebuild after injected crash");
        let ledger = reopened
            .metadata()
            .projection_applied_events("counter")
            .expect("recovered ledger");
        let ledger: Vec<String> = ledger
            .into_iter()
            .map(|entry| format!("{}:{}", entry.event_id, entry.payload_digest))
            .collect();
        assert_projection_equal(&recovered, &clean_projection, &ledger, &clean_ledger);
        assert_eq!(
            recovered.status, "ready",
            "rebuild must clear prior degraded state"
        );
    }
}

#[test]
fn fi10_repeated_crash_reopen_is_idempotent() {
    let project = unprojected_fixture();
    for attempt in 0..2 {
        let mut repository = Repository::open(project.path()).expect("open");
        install_counter_handler(repository.metadata_mut());
        repository
            .metadata_mut()
            .set_projection_failpoints(ProjectionFailpoints::once(
                ProjectionFailpoint::BeforeTransactionCommit,
            ));
        let error = repository
            .metadata_mut()
            .apply_projection("counter", format!("crash-{attempt}"));
        assert!(matches!(error, Err(PongError::FaultInjected(_))));
        drop(repository);
    }

    let mut repository = Repository::open(project.path()).expect("final reopen");
    install_counter_handler(repository.metadata_mut());
    let projection = repository
        .metadata_mut()
        .rebuild_projection("counter", "final-recovery")
        .expect("final recovery");
    assert_eq!(projection.state_json, r#"{"total":15}"#);
    assert_eq!(projection.event_count, 5);
    assert_eq!(
        repository
            .metadata()
            .projection_applied_events("counter")
            .expect("ledger")
            .len(),
        5
    );
}

#[test]
fn fi10_migration_interruption_keeps_generation_and_projection_aligned() {
    let points = [
        pong_core::MigrationFailpoint::BeforeProjectionRebind,
        pong_core::MigrationFailpoint::AfterProjectionRebind,
        pong_core::MigrationFailpoint::AfterCheckpoint,
        pong_core::MigrationFailpoint::AfterVerify,
        pong_core::MigrationFailpoint::BeforeSelectorReplace,
        pong_core::MigrationFailpoint::AfterSelectorReplace,
        pong_core::MigrationFailpoint::AfterJournalFinalize,
    ];
    for point in points {
        let project = legacy_fixture();
        let result = Repository::migrate_with_failpoints(
            project.path(),
            migration_spec("migration-a", "generation-a"),
            pong_core::MigrationFailpoints::once(point),
        );
        assert!(
            matches!(result, Err(PongError::FaultInjected(_))),
            "{point:?}"
        );
        let mut repository = Repository::open(project.path()).expect("cold reopen");
        install_counter_handler(repository.metadata_mut());
        let active = repository.active_generation_id();
        let projection = repository
            .metadata()
            .projection_record("counter")
            .expect("projection")
            .expect("projection exists");
        match active {
            None => {
                assert_eq!(projection.generation_id, "legacy-v0.1", "{point:?}");
                assert_eq!(projection.migration_id, "legacy", "{point:?}");
            }
            Some("generation-a") => {
                assert_eq!(projection.generation_id, "generation-a", "{point:?}");
                assert_eq!(projection.migration_id, "migration-a", "{point:?}");
            }
            other => panic!("unexpected active generation {other:?} at {point:?}"),
        }
        drop(repository);
        let retry = Repository::migrate(
            project.path(),
            migration_spec("migration-a", "generation-a"),
        )
        .expect("retry migration");
        assert!(retry.already_active || retry.target_generation_id == "generation-a");
        let repository = Repository::open(project.path()).expect("reopen after retry");
        let projection = repository
            .metadata()
            .projection_record("counter")
            .expect("projection")
            .expect("projection exists");
        assert_eq!(projection.generation_id, "generation-a", "{point:?}");
        assert_eq!(projection.migration_id, "migration-a", "{point:?}");
        assert_eq!(projection.event_count, 5, "{point:?}");
    }
}

#[test]
fn pt13_malformed_envelope_and_corrupted_projection_fail_closed() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    let mut malformed = event("malformed", 1, None, None);
    malformed.event_type.clear();
    let error = metadata
        .append_event_envelope(malformed)
        .expect_err("malformed envelope");
    assert_eq!(error.code(), "INVALID_INPUT");

    let project = unprojected_fixture();
    Repository::migrate(
        project.path(),
        migration_spec("migration-a", "generation-a"),
    )
    .expect("migration");
    let active_metadata = Repository::open(project.path())
        .expect("open active")
        .active_metadata_path();
    let connection = rusqlite::Connection::open(active_metadata.clone()).expect("sqlite");
    connection
        .execute(
            r#"UPDATE projections SET state_json = '{"total":999}' WHERE projection_id = 'counter'"#,
            [],
        )
        .expect("corrupt projection state");
    drop(connection);

    let error = Repository::open(project.path()).expect_err("corrupt projection must fail closed");
    assert_eq!(error.code(), "INTEGRITY_ERROR");

    let mut metadata = MetadataStore::open(&active_metadata).expect("direct metadata open");
    install_counter_handler(&mut metadata);
    let error = metadata
        .apply_projection("counter", "corrupt-apply")
        .expect_err("corrupt projection apply must fail closed");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}

#[test]
fn pt13_duplicate_event_identity_and_stale_cursor_fail_closed() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    let original = event("duplicate", 1, None, None);
    metadata
        .append_event_envelope(original.clone())
        .expect("original event");
    metadata
        .append_event_envelope(original)
        .expect("identical duplicate is idempotent");
    let conflicting = event("duplicate", 2, None, None);
    let error = metadata
        .append_event_envelope(conflicting)
        .expect_err("different duplicate payload");
    assert_eq!(error.code(), "INTEGRITY_ERROR");

    let project = unprojected_fixture();
    Repository::migrate(
        project.path(),
        migration_spec("migration-a", "generation-a"),
    )
    .expect("migration");
    let active_metadata = Repository::open(project.path())
        .expect("open active")
        .active_metadata_path();
    let connection = rusqlite::Connection::open(active_metadata.clone()).expect("sqlite");
    connection
        .execute(
            "UPDATE projections SET cursor_project_sequence = 99 WHERE projection_id = 'counter'",
            [],
        )
        .expect("corrupt cursor");
    drop(connection);
    let error = Repository::open(project.path()).expect_err("stale cursor must fail closed");
    assert_eq!(error.code(), "INTEGRITY_ERROR");

    let project = unprojected_fixture();
    Repository::migrate(
        project.path(),
        migration_spec("migration-a", "generation-a"),
    )
    .expect("migration");
    let active_metadata = Repository::open(project.path())
        .expect("open active")
        .active_metadata_path();
    let connection = rusqlite::Connection::open(active_metadata.clone()).expect("sqlite");
    connection
        .execute(
            "UPDATE projections SET generation_id = 'generation-stale' WHERE projection_id = 'counter'",
            [],
        )
        .expect("corrupt projection generation");
    drop(connection);
    let mut metadata = MetadataStore::open(active_metadata).expect("direct metadata open");
    install_counter_handler(&mut metadata);
    let error = metadata
        .apply_projection("counter", "stale-generation")
        .expect_err("stale projection generation must fail closed");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
}

#[test]
fn pt13_source_generation_remains_available_for_audit_after_b_is_active() {
    let project = legacy_fixture();
    Repository::migrate(
        project.path(),
        migration_spec("migration-a", "generation-a"),
    )
    .expect("A");
    Repository::migrate(
        project.path(),
        migration_spec("migration-b", "generation-b"),
    )
    .expect("B");
    let layout = pong_core::RepositoryLayout::from_root(project.path());
    let a = MetadataStore::open(layout.generation_metadata_path("generation-a"))
        .expect("open source A");
    let b = MetadataStore::open(layout.generation_metadata_path("generation-b"))
        .expect("open target B");
    assert_eq!(
        a.list_event_envelopes("project-pt13", 0).expect("A events"),
        b.list_event_envelopes("project-pt13", 0).expect("B events")
    );
    let a_projection = a
        .projection_record("counter")
        .expect("A projection")
        .expect("A row");
    let b_projection = b
        .projection_record("counter")
        .expect("B projection")
        .expect("B row");
    assert_eq!(a_projection.state_digest, b_projection.state_digest);
    assert_eq!(a_projection.cursor, b_projection.cursor);
    assert_eq!(a_projection.event_count, b_projection.event_count);
    assert!(fs::metadata(layout.generation_manifest_path("generation-a")).is_ok());
}
