use pong_core::metadata::{MetadataStore, NewEventEnvelope, ProjectionDefinition};
use serde_json::json;

fn event(id: &str, project: &str, stream: &str, kind: &str, value: i64) -> NewEventEnvelope {
    NewEventEnvelope {
        event_id: id.into(),
        project_id: project.into(),
        stream_id: stream.into(),
        event_type: kind.into(),
        schema_version: "0.1".into(),
        occurred_at: format!("2026-08-27T00:00:0{value}Z"),
        recorded_at: format!("2026-08-27T00:00:0{value}Z"),
        actor_id: Some("agent".into()),
        workspace_id: None,
        task_id: None,
        operation_id: None,
        causation_id: None,
        correlation_id: Some("request".into()),
        parent_event_ids: Vec::new(),
        capture_confidence: Some("observed".into()),
        redaction_status: "clean".into(),
        payload: json!({"value": value}),
    }
}

#[test]
fn envelopes_have_project_cursor_and_legacy_event_compatibility() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    let first = metadata
        .append_event_envelope(event("e1", "p", "s1", "counter.add", 1))
        .expect("first");
    let second = metadata
        .append_event_envelope(event("e2", "p", "s2", "counter.add", 2))
        .expect("second");
    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 1);
    assert_eq!(first.project_sequence, 1);
    assert_eq!(second.project_sequence, 2);
    let listed = metadata.list_event_envelopes("p", 0).expect("envelopes");
    assert_eq!(listed.len(), 2);
    assert_eq!(metadata.list_events("s1").expect("legacy list").len(), 1);
}

#[test]
fn projection_apply_is_idempotent_and_rebuild_is_stable() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    metadata.register_projection_handler("counter.add", |event, state| {
        let value = serde_json::from_str::<serde_json::Value>(&event.payload_json)
            .map_err(|error| pong_core::PongError::Serialization(error.to_string()))?["value"]
            .as_i64()
            .ok_or_else(|| pong_core::PongError::InvalidInput("missing value".into()))?;
        let total = state["total"].as_i64().unwrap_or(0) + value;
        state["total"] = json!(total);
        Ok(())
    });
    metadata
        .append_event_envelope(event("e1", "p", "s1", "counter.add", 1))
        .expect("event");
    metadata
        .append_event_envelope(event("e2", "p", "s2", "counter.add", 2))
        .expect("event");
    let definition = ProjectionDefinition {
        projection_id: "counter".into(),
        project_id: "p".into(),
        schema_version: "0.1".into(),
        initial_state: json!({"total": 0}),
    };
    let first = metadata
        .create_projection(definition, "t0")
        .expect("projection");
    let applied = metadata.apply_projection("counter", "t1").expect("apply");
    assert_eq!(applied.status, "ready");
    assert_eq!(applied.event_count, 2);
    assert_eq!(applied.cursor.as_ref().unwrap().project_sequence, 2);
    assert_eq!(applied.state_json, r#"{"total":3}"#);
    let retry = metadata.apply_projection("counter", "t2").expect("retry");
    assert_eq!(retry.state_digest, applied.state_digest);
    assert_eq!(retry.event_count, 2);
    let rebuilt = metadata
        .rebuild_projection("counter", "t3")
        .expect("rebuild");
    assert_eq!(rebuilt.state_digest, applied.state_digest);
    assert_eq!(rebuilt.event_count, 2);
    assert_eq!(first.generation_id, rebuilt.generation_id);
}

#[test]
fn unknown_event_type_degrades_projection_without_dropping_source() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    metadata
        .append_event_envelope(event("e1", "p", "s", "future.event", 1))
        .expect("event");
    metadata
        .create_projection(
            ProjectionDefinition {
                projection_id: "p1".into(),
                project_id: "p".into(),
                schema_version: "0.1".into(),
                initial_state: json!({}),
            },
            "t0",
        )
        .expect("projection");
    let result = metadata.apply_projection("p1", "t1").expect("apply");
    assert_eq!(result.status, "degraded");
    let retry = metadata.apply_projection("p1", "t2").expect("retry");
    assert_eq!(retry.status, "degraded");
    assert_eq!(
        metadata.list_event_envelopes("p", 0).expect("source").len(),
        1
    );
}
