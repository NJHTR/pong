use pong_core::metadata::{
    MetadataFailpoint, MetadataFailpoints, MetadataStore, OperationEnvelope, OperationError,
    OperationOutcome, OperationRef,
};
use pong_core::redaction::Redactor;
use pong_core::WorkspaceRecord;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn envelope(operation_id: &str, request_id: &str) -> OperationEnvelope {
    OperationEnvelope {
        operation_id: operation_id.into(),
        project_id: "project-ledger".into(),
        request_id: request_id.into(),
        agent_id: "agent-ledger".into(),
        session_id: "session-ledger".into(),
        workspace_id: None,
        environment_id: None,
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "2026-08-25T12:00:00Z".into(),
        tool: "filesystem".into(),
        action: "write".into(),
        input_refs: vec![OperationRef {
            kind: "blob".into(),
            reference: "sha256:input".into(),
            media_type: Some("text/plain".into()),
        }],
        output_refs: Vec::new(),
        resource: Some(json!({"path":"README.md"})),
        before_state: Some(json!({"digest":"sha256:before"})),
        after_state: None,
        reversibility: "REVERSIBLE".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "WORKSPACE".into(),
        policy_decision: Some(json!({"allowed":true})),
    }
}

fn completed() -> OperationOutcome {
    OperationOutcome {
        status: "completed".into(),
        finished_at: "2026-08-25T12:00:01Z".into(),
        output_refs: Some(vec![OperationRef {
            kind: "blob".into(),
            reference: "sha256:output".into(),
            media_type: None,
        }]),
        after_state: Some(json!({"digest":"sha256:after"})),
        result: Some(json!({"bytes": 7})),
        error: None,
    }
}

#[test]
fn operation_start_finish_is_durable_and_emits_ordered_events() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    let started = metadata
        .start_operation(envelope("op-ledger-1", "req-ledger-1"))
        .expect("start");
    assert_eq!(started.lifecycle_status, "started");
    assert_eq!(started.recording_status, "durable");
    assert_eq!(started.input_refs.len(), 1);
    assert_eq!(started.output_refs, Vec::<OperationRef>::new());

    let finished = metadata
        .finish_operation("op-ledger-1", completed())
        .expect("finish");
    assert_eq!(finished.lifecycle_status, "completed");
    assert_eq!(finished.result, Some(json!({"bytes": 7})));
    assert_eq!(finished.after_state, Some(json!({"digest":"sha256:after"})));
    assert_eq!(finished.output_refs.len(), 1);

    let events = metadata
        .list_events("operation:op-ledger-1")
        .expect("operation events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].sequence, 1);
    assert_eq!(events[1].sequence, 2);
    assert!(events[0].payload_json.contains("started"));
    assert!(events[1].payload_json.contains("completed"));
}

#[test]
fn operation_request_retry_is_idempotent_but_changed_content_is_rejected() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    let first = metadata
        .start_operation(envelope("op-ledger-2", "req-ledger-2"))
        .expect("start");
    let retry = metadata
        .start_operation(envelope("op-ledger-2", "req-ledger-2"))
        .expect("same request retry");
    assert_eq!(retry, first);

    let mut changed = envelope("op-ledger-2", "req-ledger-2");
    changed.action = "delete".into();
    let error = metadata
        .start_operation(changed)
        .expect_err("changed request must fail closed");
    assert_eq!(error.code(), "IDEMPOTENCY_KEY_REUSE");

    let other_request_same_id = envelope("op-ledger-2", "req-ledger-3");
    let error = metadata
        .start_operation(other_request_same_id)
        .expect_err("operation id cannot be reused");
    assert_eq!(error.code(), "CONFLICT");
}

#[test]
fn operation_bindings_require_existing_same_project_records() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    metadata
        .create_workspace(&WorkspaceRecord {
            workspace_id: "ws-ledger".into(),
            project_id: "project-ledger".into(),
            driver: "local".into(),
            locator: "C:/workspace-ledger".into(),
            branch_ref: None,
            head: None,
            environment_id: None,
            status: "created".into(),
            revision: 0,
            created_at: "t0".into(),
            updated_at: "t0".into(),
        })
        .expect("workspace");
    metadata
        .record_environment(
            "env-ledger",
            "project-ledger",
            &json!({"schema_version":1}),
            "t0",
        )
        .expect("environment");

    let mut valid = envelope("op-ledger-3", "req-ledger-3");
    valid.workspace_id = Some("ws-ledger".into());
    valid.environment_id = Some("env-ledger".into());
    metadata
        .start_operation(valid)
        .expect("same-project bindings");

    metadata
        .start_operation(envelope("op-ledger-parent", "req-ledger-parent"))
        .expect("parent operation");
    let mut child = envelope("op-ledger-child", "req-ledger-child");
    child.parent_operation_id = Some("op-ledger-parent".into());
    metadata
        .start_operation(child)
        .expect("same-project parent");

    let mut missing = envelope("op-ledger-4", "req-ledger-4");
    missing.workspace_id = Some("ws-missing".into());
    let error = metadata
        .start_operation(missing)
        .expect_err("missing workspace");
    assert_eq!(error.code(), "CONFLICT");

    let mut wrong = envelope("op-ledger-5", "req-ledger-5");
    wrong.environment_id = Some("env-ledger".into());
    wrong.project_id = "other-project".into();
    let error = metadata
        .start_operation(wrong)
        .expect_err("cross-project environment");
    assert_eq!(error.code(), "CONFLICT");

    let mut missing_parent = envelope("op-ledger-missing-parent", "req-ledger-missing-parent");
    missing_parent.parent_operation_id = Some("op-does-not-exist".into());
    let error = metadata
        .start_operation(missing_parent)
        .expect_err("missing parent");
    assert_eq!(error.code(), "CONFLICT");
}

#[test]
fn terminal_outcome_is_idempotent_and_cannot_be_rewritten() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    metadata
        .start_operation(envelope("op-ledger-6", "req-ledger-6"))
        .expect("start");
    let failed = OperationOutcome {
        status: "failed".into(),
        finished_at: "t1".into(),
        output_refs: None,
        after_state: None,
        result: None,
        error: Some(OperationError {
            code: "PERMISSION_DENIED".into(),
            message: "denied".into(),
            retryable: false,
            details: Some(json!({"safe":true})),
            safe_to_expose: true,
        }),
    };
    let first = metadata
        .finish_operation("op-ledger-6", failed.clone())
        .expect("failed outcome");
    let retry = metadata
        .finish_operation("op-ledger-6", failed)
        .expect("same failed outcome retry");
    assert_eq!(retry, first);

    let mut changed = completed();
    changed.finished_at = "t2".into();
    let error = metadata
        .finish_operation("op-ledger-6", changed)
        .expect_err("terminal lifecycle cannot change");
    assert_eq!(error.code(), "CONFLICT");
}

#[test]
fn cancelled_unknown_and_recording_quality_are_explicit_states() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    metadata
        .start_operation(envelope("op-ledger-cancelled", "req-ledger-cancelled"))
        .expect("start cancelled");
    let cancelled = metadata
        .finish_operation(
            "op-ledger-cancelled",
            OperationOutcome {
                status: "cancelled".into(),
                finished_at: "t-cancelled".into(),
                output_refs: None,
                after_state: None,
                result: None,
                error: Some(OperationError {
                    code: "CANCELLED".into(),
                    message: "operator cancelled".into(),
                    retryable: true,
                    details: None,
                    safe_to_expose: true,
                }),
            },
        )
        .expect("cancelled outcome");
    assert_eq!(cancelled.lifecycle_status, "cancelled");

    metadata
        .start_operation(envelope("op-ledger-unknown", "req-ledger-unknown"))
        .expect("start unknown");
    let unknown = metadata
        .finish_operation(
            "op-ledger-unknown",
            OperationOutcome {
                status: "unknown".into(),
                finished_at: "t-unknown".into(),
                output_refs: None,
                after_state: None,
                result: None,
                error: Some(OperationError {
                    code: "OUTCOME_UNKNOWN".into(),
                    message: "process ended before observation".into(),
                    retryable: false,
                    details: Some(json!({"reconcile":true})),
                    safe_to_expose: false,
                }),
            },
        )
        .expect("unknown outcome");
    assert_eq!(unknown.lifecycle_status, "unknown");

    let updated = metadata
        .set_operation_recording_status("op-ledger-unknown", "unreconciled", "t-reconcile")
        .expect("recording quality");
    assert_eq!(updated.recording_status, "unreconciled");
    let retry = metadata
        .set_operation_recording_status("op-ledger-unknown", "unreconciled", "ignored")
        .expect("same recording status retry");
    assert_eq!(retry, updated);
}

#[test]
fn operation_redaction_applies_before_sqlite_and_event_persistence() {
    let secret = "ledger-secret-value";
    let mut redactor = Redactor::new("ledger-profile", "0.1").expect("profile");
    redactor.register_secret(secret).expect("secret");
    let directory = tempdir().expect("directory");
    let path = directory.path().join("metadata.sqlite");
    let mut metadata = MetadataStore::open_with_redactor(&path, redactor.clone()).expect("open");
    let mut value = envelope("op-ledger-secret", "req-ledger-secret");
    value.tool = format!("filesystem-{secret}");
    value.input_refs[0].reference = format!("sha256:{secret}");
    value.resource = Some(json!({"token": secret, "safe": "value"}));
    let record = metadata.start_operation(value).expect("start");
    assert!(!record.tool.contains(secret));
    assert!(!record.input_refs[0].reference.contains(secret));
    assert_eq!(
        record.resource,
        Some(json!({"token":"[REDACTED]","safe":"value"}))
    );
    let events = metadata
        .list_events("operation:op-ledger-secret")
        .expect("events");
    assert!(events
        .iter()
        .all(|event| !event.payload_json.contains(secret)));
    drop(metadata);
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut candidate = path.as_os_str().to_os_string();
        candidate.push(suffix);
        if let Ok(bytes) = fs::read(candidate) {
            assert!(!bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()));
        }
    }
}

#[test]
fn operation_commit_faults_are_pre_or_post_state_after_cold_reopen() {
    let directory = tempdir().expect("directory");
    let path = directory.path().join("metadata.sqlite");
    let error = MetadataStore::open_with_redactor_and_failpoints(
        &path,
        Redactor::default(),
        MetadataFailpoints::once(MetadataFailpoint::BeforeSqliteCommit),
    )
    .expect("open")
    .start_operation(envelope("op-ledger-before", "req-ledger-before"))
    .expect_err("before commit fault");
    assert_eq!(error.code(), "FAULT_INJECTED");
    let metadata = MetadataStore::open(&path).expect("cold reopen before");
    assert!(metadata
        .operation_record("op-ledger-before")
        .expect("lookup")
        .is_none());
    drop(metadata);

    let mut metadata = MetadataStore::open_with_redactor_and_failpoints(
        &path,
        Redactor::default(),
        MetadataFailpoints::once(MetadataFailpoint::AfterSqliteCommit),
    )
    .expect("reopen");
    let error = metadata
        .start_operation(envelope("op-ledger-after", "req-ledger-after"))
        .expect_err("after commit fault");
    assert_eq!(error.code(), "FAULT_INJECTED");
    drop(metadata);
    let metadata = MetadataStore::open(&path).expect("cold reopen after");
    assert_eq!(
        metadata
            .operation_record("op-ledger-after")
            .expect("lookup")
            .expect("durable operation")
            .lifecycle_status,
        "started"
    );
}

#[test]
fn operation_finish_commit_faults_preserve_pre_or_post_state() {
    let directory = tempdir().expect("directory");
    let path = directory.path().join("metadata.sqlite");
    let mut metadata = MetadataStore::open(&path).expect("metadata");
    metadata
        .start_operation(envelope("op-finish-before", "req-finish-before"))
        .expect("start before");
    metadata.set_failpoints(MetadataFailpoints::once(
        MetadataFailpoint::BeforeSqliteCommit,
    ));
    let error = metadata
        .finish_operation("op-finish-before", completed())
        .expect_err("finish before commit fault");
    assert_eq!(error.code(), "FAULT_INJECTED");
    drop(metadata);
    let mut metadata = MetadataStore::open(&path).expect("reopen before");
    assert_eq!(
        metadata
            .operation_record("op-finish-before")
            .expect("lookup")
            .expect("operation")
            .lifecycle_status,
        "started"
    );
    metadata.set_failpoints(MetadataFailpoints::once(
        MetadataFailpoint::AfterSqliteCommit,
    ));
    let error = metadata
        .finish_operation("op-finish-before", completed())
        .expect_err("finish after commit fault");
    assert_eq!(error.code(), "FAULT_INJECTED");
    drop(metadata);
    let metadata = MetadataStore::open(&path).expect("reopen after");
    assert_eq!(
        metadata
            .operation_record("op-finish-before")
            .expect("lookup")
            .expect("operation")
            .lifecycle_status,
        "completed"
    );
}
