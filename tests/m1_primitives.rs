use pong_core::cas::Cas;
use pong_core::metadata::{IdempotencyKey, IdempotencyResult, MetadataStore, NewEvent};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn cas_and_metadata_ports_share_m1_identity_rules() {
    let directory = tempdir().expect("temporary repository");
    let cas = Cas::new(directory.path().join("objects")).expect("cas");
    let digest = cas
        .put("snapshot/v1", br#"{"files":[]}"#)
        .expect("object publication");
    assert_eq!(
        cas.get("snapshot/v1", digest).expect("object read"),
        br#"{"files":[]}"#
    );

    let mut metadata =
        MetadataStore::open(directory.path().join("metadata.sqlite")).expect("metadata");
    metadata
        .compare_and_swap_ref(
            "refs/heads/main",
            None,
            &digest.to_hex(),
            "2026-08-19T00:00:00Z",
        )
        .expect("ref publication");
    assert_eq!(
        metadata.get_ref("refs/heads/main").expect("ref read"),
        Some(digest.to_hex())
    );
}

#[test]
fn event_and_idempotency_records_are_replay_safe() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    let key = IdempotencyKey {
        project_id: "prj_test".into(),
        actor_id: "agt_test".into(),
        request_id: "req_test".into(),
    };
    let result = json!({"status":"accepted","commit":"c1"});
    assert_eq!(
        metadata
            .record_idempotency(&key, "sha256:command", &result, "2026-08-19T00:00:00Z")
            .expect("idempotency record"),
        IdempotencyResult::NewlyRecorded
    );
    assert_eq!(
        metadata
            .record_idempotency(
                &key,
                "sha256:command",
                &json!({"commit":"c1","status":"accepted"}),
                "later"
            )
            .expect("idempotent retry"),
        IdempotencyResult::Existing(result)
    );

    let event = NewEvent {
        event_id: "evt_1".into(),
        project_id: "prj_test".into(),
        stream_id: "project:prj_test".into(),
        schema_version: "0.1".into(),
        actor_id: Some("agt_test".into()),
        request_id: Some("req_test".into()),
        occurred_at: "2026-08-19T00:00:00Z".into(),
        payload: json!({"type":"commit.created"}),
    };
    assert_eq!(
        metadata
            .append_event(event.clone())
            .expect("event")
            .sequence,
        1
    );
    assert_eq!(
        metadata
            .append_event(event)
            .expect("duplicate event")
            .sequence,
        1
    );
    assert_eq!(
        metadata
            .list_events("project:prj_test")
            .expect("event list")
            .len(),
        1
    );
}
