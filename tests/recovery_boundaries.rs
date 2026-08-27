use pong_core::cas::Cas;
use pong_core::error::PongError;
use pong_core::metadata::{IdempotencyKey, IdempotencyResult, MetadataStore, NewEvent};
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn key(request_id: &str) -> IdempotencyKey {
    IdempotencyKey {
        project_id: "prj_recovery".into(),
        actor_id: "agt_recovery".into(),
        request_id: request_id.into(),
    }
}

#[test]
fn orphan_intents_are_marked_unknown_once_and_keep_the_intent_payload() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    let request = key("req_orphan");
    let intent = json!({"tool":"filesystem.write","path":"notes.txt"});
    metadata
        .record_intent(
            &request.project_id,
            &request.actor_id,
            &request.request_id,
            "op_orphan",
            &intent,
            "2026-08-19T00:00:00Z",
        )
        .expect("intent");

    assert_eq!(metadata.unfinished_operations().expect("pending").len(), 1);
    let recovered = metadata
        .recover_unfinished_operations()
        .expect("recover orphan");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].phase, "unknown");
    assert_eq!(
        recovered[0].payload_json,
        r#"{"path":"notes.txt","tool":"filesystem.write"}"#
    );

    let unknown = metadata.unknown_operations().expect("unknown operations");
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].request_id, request.request_id);
    assert!(metadata
        .unfinished_operations()
        .expect("pending after recovery")
        .is_empty());

    // Recovery is convergent: no second mutation or duplicate unknown record.
    assert!(metadata
        .recover_unfinished_operations()
        .expect("repeat recovery")
        .is_empty());
    assert_eq!(
        metadata.unknown_operations().expect("unknown again").len(),
        1
    );

    // Unknown is terminal until a separate reconciliation event is appended.
    let error = metadata
        .record_outcome(&request, "published", &json!({"status":"ok"}))
        .expect_err("unknown effect must not be published");
    assert_eq!(error.code(), "CONFLICT");
}

#[test]
fn outcome_and_published_transitions_are_idempotent() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    let request = key("req_transition");
    metadata
        .record_intent(
            &request.project_id,
            &request.actor_id,
            &request.request_id,
            "op_transition",
            &json!({"action":"write"}),
            "2026-08-19T00:00:00Z",
        )
        .expect("intent");

    let durable = json!({"status":"accepted","effect":"applied"});
    metadata
        .record_outcome(&request, "outcome_durable", &durable)
        .expect("outcome");
    // Equivalent retry is a no-op, even when object key order differs.
    metadata
        .record_outcome(
            &request,
            "outcome_durable",
            &json!({"effect":"applied","status":"accepted"}),
        )
        .expect("idempotent outcome retry");

    let published = json!({"status":"published","event_id":"evt_1"});
    metadata
        .record_outcome(&request, "published", &published)
        .expect("publish");
    metadata
        .record_outcome(&request, "published", &published)
        .expect("idempotent publish retry");
    assert_eq!(metadata.unfinished_operations().expect("pending").len(), 0);

    let changed = metadata
        .record_outcome(&request, "published", &json!({"status":"different"}))
        .expect_err("changed terminal payload must fail");
    assert_eq!(changed.code(), "INTEGRITY_ERROR");

    let rewind = metadata
        .record_outcome(&request, "outcome_durable", &durable)
        .expect_err("terminal phase must not rewind");
    assert_eq!(rewind.code(), "CONFLICT");
}

#[test]
fn outcome_durable_rows_remain_available_for_publication_recovery() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    let request = key("req_outbox");
    metadata
        .record_intent(
            &request.project_id,
            &request.actor_id,
            &request.request_id,
            "op_outbox",
            &json!({"action":"commit"}),
            "2026-08-19T00:00:00Z",
        )
        .expect("intent");
    metadata
        .record_outcome(&request, "outcome_durable", &json!({"status":"accepted"}))
        .expect("durable outcome");

    let pending = metadata
        .unfinished_operations()
        .expect("pending publication");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].phase, "outcome_durable");
    assert!(metadata
        .recover_unfinished_operations()
        .expect("orphan recovery")
        .is_empty());
    assert_eq!(
        metadata
            .operation(&request)
            .expect("operation")
            .expect("row")
            .phase,
        "outcome_durable"
    );
}

#[test]
fn unsupported_repository_format_is_rejected_before_open() {
    let directory = tempdir().expect("temporary repository");
    let path = directory.path().join("metadata.sqlite");
    let connection = Connection::open(&path).expect("sqlite");
    connection
        .execute_batch(
            "CREATE TABLE repository_meta (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
             INSERT INTO repository_meta(key, value) VALUES ('repository_format', '9.0');",
        )
        .expect("fixture");
    drop(connection);

    let error = match MetadataStore::open(&path) {
        Ok(_) => panic!("unsupported format opened successfully"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "UNSUPPORTED");
    assert!(error.to_string().contains("repository format"));
}

#[test]
fn committed_metadata_survives_reopen_and_uses_full_wal_durability() {
    let directory = tempdir().expect("temporary repository");
    let path = directory.path().join("metadata.sqlite");
    let request = key("req_restart");
    {
        let mut metadata = MetadataStore::open(&path).expect("metadata");
        metadata
            .record_intent(
                &request.project_id,
                &request.actor_id,
                &request.request_id,
                "op_restart",
                &json!({"action":"checkpoint"}),
                "2026-08-19T00:00:00Z",
            )
            .expect("intent");
        metadata
            .record_outcome(&request, "outcome_durable", &json!({"status":"accepted"}))
            .expect("outcome");
        metadata
            .append_event(NewEvent {
                event_id: "evt_restart".into(),
                project_id: request.project_id.clone(),
                stream_id: "project:prj_recovery".into(),
                schema_version: "0.1".into(),
                actor_id: Some(request.actor_id.clone()),
                request_id: Some(request.request_id.clone()),
                occurred_at: "2026-08-19T00:00:00Z".into(),
                payload: json!({"type":"checkpoint.created"}),
            })
            .expect("event");
    }

    let metadata = MetadataStore::open(&path).expect("reopen");
    assert_eq!(metadata.repository_format().expect("format"), "0.1");
    assert_eq!(
        metadata
            .list_events("project:prj_recovery")
            .expect("events")
            .len(),
        1
    );
    assert_eq!(
        metadata
            .operation(&request)
            .expect("operation")
            .expect("row")
            .phase,
        "outcome_durable"
    );

    let connection = Connection::open(&path).expect("sqlite inspect");
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal mode");
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    let synchronous: i64 = connection
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .expect("synchronous mode");
    assert_eq!(synchronous, 2, "SQLite FULL synchronous mode is 2");
}

#[test]
fn corrupt_cas_object_is_quarantined_and_never_read_as_success() {
    let directory = tempdir().expect("temporary repository");
    let cas = Cas::new(directory.path().join("objects")).expect("cas");
    let digest = cas.put("snapshot/v1", b"committed bytes").expect("put");
    fs::write(cas.object_path(digest), b"corrupt bytes").expect("tamper");

    let error = cas.get("snapshot/v1", digest).expect_err("corruption");
    assert!(matches!(error, PongError::Integrity(_)));
    assert!(!cas.object_path(digest).exists());
    assert_eq!(
        cas.root()
            .join("quarantine")
            .read_dir()
            .expect("quarantine")
            .count(),
        1
    );
}

#[test]
fn idempotency_record_survives_restart_without_second_mutation() {
    let directory = tempdir().expect("temporary repository");
    let path = directory.path().join("metadata.sqlite");
    let request = key("req_idempotent_restart");
    let result = json!({"status":"accepted","commit":"c1"});
    {
        let mut metadata = MetadataStore::open(&path).expect("metadata");
        assert_eq!(
            metadata
                .record_idempotency(&request, "sha256:cmd", &result, "t1")
                .expect("first request"),
            IdempotencyResult::NewlyRecorded
        );
    }
    let mut metadata = MetadataStore::open(&path).expect("reopen");
    assert_eq!(
        metadata
            .record_idempotency(
                &request,
                "sha256:cmd",
                &json!({"commit":"c1","status":"accepted"}),
                "t2",
            )
            .expect("retry"),
        IdempotencyResult::Existing(result)
    );
}
