use pong_core::metadata::{MetadataStore, NewEvent};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn event(id: &str, sequence_hint: &str) -> NewEvent {
    NewEvent {
        event_id: id.into(),
        project_id: "project-wal-tail".into(),
        stream_id: "project:project-wal-tail".into(),
        schema_version: "0.1".into(),
        actor_id: Some("agent-wal-tail".into()),
        request_id: Some(sequence_hint.into()),
        occurred_at: "2026-08-19T00:00:00Z".into(),
        payload: json!({"type":"test.event","id":id}),
    }
}

#[test]
fn truncated_wal_tail_never_fabricates_an_event_after_cold_restart() {
    let directory = tempdir().expect("directory");
    let path = directory.path().join("metadata.sqlite");

    // Establish a committed base generation before creating the damaged WAL
    // tail. This gives the recovery assertion an unambiguous pre-state.
    {
        let mut metadata = MetadataStore::open(&path).expect("metadata");
        metadata
            .append_event(event("event-base", "request-base"))
            .expect("base event");
    }
    let baseline = MetadataStore::open(&path).expect("baseline reopen");
    assert_eq!(
        baseline
            .list_events("project:project-wal-tail")
            .expect("baseline events")
            .len(),
        1
    );
    drop(baseline);

    let wal_path = wal_path(&path);
    let mut metadata = MetadataStore::open(&path).expect("metadata for wal");
    metadata
        .append_event(event("event-tail", "request-tail"))
        .expect("tail event");
    let wal = fs::read(&wal_path).expect("WAL must be present while connection is open");
    assert!(wal.len() > 1, "WAL fixture must contain bytes to truncate");
    fs::write(&wal_path, &wal[..wal.len() - 1]).expect("truncate WAL tail");
    drop(metadata);

    // SQLite owns WAL checksum/commit-marker recovery. The Pong boundary must
    // expose either the complete pre-state or a complete committed state, and
    // never return a fabricated partial event from the torn tail.
    match MetadataStore::open(&path) {
        Ok(reopened) => {
            let events = reopened
                .list_events("project:project-wal-tail")
                .expect("events after WAL recovery");
            assert!(events.len() <= 2);
            assert!(events.iter().all(|record| {
                record.event_id == "event-base" || record.event_id == "event-tail"
            }));
            if events.len() == 2 {
                assert_eq!(events[0].sequence, 1);
                assert_eq!(events[1].sequence, 2);
            }
        }
        Err(error) => {
            // A torn tail may be rejected by SQLite on a particular platform;
            // this is still fail-closed evidence as long as no success was
            // reported for a partial record.
            assert!(matches!(
                error.code(),
                "STORAGE_ERROR" | "INTEGRITY_ERROR" | "RECOVERY_REQUIRED"
            ));
        }
    }
}

fn wal_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push("-wal");
    PathBuf::from(value)
}
