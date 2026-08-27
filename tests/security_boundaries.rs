use pong_core::metadata::{IdempotencyKey, NewEvent};
use pong_core::redaction::Redactor;
use pong_core::repository::Repository;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn assert_tree_has_no_bytes(root: &Path, needle: &[u8]) {
    let metadata = fs::symlink_metadata(root).expect("tree entry");
    if metadata.is_dir() {
        for entry in fs::read_dir(root).expect("tree") {
            assert_tree_has_no_bytes(&entry.expect("entry").path(), needle);
        }
    } else if metadata.is_file() {
        let bytes = fs::read(root).expect("file bytes");
        assert!(!bytes.windows(needle.len()).any(|window| window == needle));
    }
}

#[test]
fn full_pong_tree_is_secret_free_and_unknown_files_fail_closed() {
    let project = tempdir().expect("project");
    let mut redactor = Redactor::new("security-profile", "0.1").expect("profile");
    redactor
        .register_secret("top-secret-value")
        .expect("secret");

    let mut repository =
        Repository::init_with_redactor(project.path(), redactor.clone()).expect("init");
    let key = IdempotencyKey {
        project_id: "project-security".into(),
        actor_id: "agent-security".into(),
        request_id: "request-security".into(),
    };
    repository
        .metadata_mut()
        .record_intent(
            &key.project_id,
            &key.actor_id,
            &key.request_id,
            "operation-security",
            &serde_json::json!({
                "argv": ["--token", "top-secret-value"],
                "nested": {"value": "top-secret-value"}
            }),
            "2026-08-19T00:00:00Z",
        )
        .expect("redacted intent");
    repository
        .metadata_mut()
        .record_idempotency(
            &key,
            "sha256:command",
            &serde_json::json!({"authorization": "top-secret-value"}),
            "2026-08-19T00:00:00Z",
        )
        .expect("redacted idempotency");
    repository
        .metadata_mut()
        .append_event(NewEvent {
            event_id: "event-security".into(),
            project_id: key.project_id.clone(),
            stream_id: "project:project-security".into(),
            schema_version: "0.1".into(),
            actor_id: Some(key.actor_id.clone()),
            request_id: Some(key.request_id.clone()),
            occurred_at: "2026-08-19T00:00:00Z".into(),
            payload: serde_json::json!({"result": "top-secret-value"}),
        })
        .expect("redacted event");
    assert!(repository
        .cas()
        .put("artifact/v1", b"top-secret-value")
        .is_err());
    drop(repository);

    let secret = b"top-secret-value";
    assert_tree_has_no_bytes(&project.path().join(".pong"), secret);

    // Future exports and temporary files are part of the owned boundary even
    // when this version does not know their names yet.
    let export_dir = project.path().join(".pong").join("exports");
    fs::create_dir_all(&export_dir).expect("export directory");
    fs::write(export_dir.join("event.tmp"), secret).expect("secret-bearing export");
    let error = Repository::open_with_redactor(project.path(), redactor)
        .expect_err("unknown secret-bearing file must block startup");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
    assert!(!error.to_string().contains("top-secret-value"));
}
