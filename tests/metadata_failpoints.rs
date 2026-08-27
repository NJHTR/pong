use pong_core::error::PongError;
use pong_core::metadata::{IdempotencyKey, MetadataFailpoint, MetadataFailpoints, MetadataStore};
use serde_json::json;
use tempfile::tempdir;

fn key(request_id: &str) -> IdempotencyKey {
    IdempotencyKey {
        project_id: "project-faults".into(),
        actor_id: "agent-faults".into(),
        request_id: request_id.into(),
    }
}

fn assert_fault(error: PongError, label: &str) {
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert!(
        error.to_string().contains(label),
        "unexpected fault: {error}"
    );
}

fn record_intent(store: &mut MetadataStore, request_id: &str) {
    let request = key(request_id);
    store
        .record_intent(
            &request.project_id,
            &request.actor_id,
            &request.request_id,
            &format!("operation-{request_id}"),
            &json!({"action":"write"}),
            "2026-08-19T00:00:00Z",
        )
        .expect("intent");
}

#[test]
fn failpoints_are_disabled_by_default_and_one_shot() {
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    assert_eq!(metadata.failpoints().armed(), None);
    metadata.set_failpoints(MetadataFailpoints::once(
        MetadataFailpoint::BeforeIntentCommit,
    ));
    let request = key("req_default");
    let error = metadata
        .record_intent(
            &request.project_id,
            &request.actor_id,
            &request.request_id,
            "operation-default",
            &json!({"action":"write"}),
            "t1",
        )
        .expect_err("armed fault");
    assert_fault(error, "before_intent_commit");
    assert_eq!(metadata.failpoints().armed(), None);
    assert!(metadata.operation(&request).expect("lookup").is_none());

    // The same request is retryable because the pre-commit failure rolled back
    // the transaction rather than leaving a half-written journal row.
    metadata
        .record_intent(
            &request.project_id,
            &request.actor_id,
            &request.request_id,
            "operation-default",
            &json!({"action":"write"}),
            "t1",
        )
        .expect("retry after rollback");
}

#[test]
fn fi01_before_intent_commit_cold_reopen_preserves_pre_state_and_retry_publishes_once() {
    let directory = tempdir().expect("directory");
    let path = directory.path().join("metadata.sqlite");
    let request = key("req_fi01_cold_reopen");

    {
        let mut metadata = MetadataStore::open_with_failpoints(
            &path,
            MetadataFailpoints::once(MetadataFailpoint::BeforeIntentCommit),
        )
        .expect("metadata");
        let error = metadata
            .record_intent(
                &request.project_id,
                &request.actor_id,
                &request.request_id,
                "operation-fi01-cold-reopen",
                &json!({"action":"write"}),
                "t-fi01",
            )
            .expect_err("pre-commit fault");
        assert_fault(error, "before_intent_commit");
        assert_eq!(metadata.failpoints().armed(), None);
        assert!(metadata
            .operation(&request)
            .expect("same-process pre-state lookup")
            .is_none());
    }

    // The handle is dropped before this open, so the assertion exercises the
    // on-disk SQLite/WAL state rather than a same-connection transaction view.
    let mut reopened = MetadataStore::open(&path).expect("cold reopen");
    assert!(reopened
        .operation(&request)
        .expect("cold pre-state lookup")
        .is_none());
    assert!(reopened
        .unfinished_operations()
        .expect("cold unfinished lookup")
        .is_empty());

    reopened
        .record_intent(
            &request.project_id,
            &request.actor_id,
            &request.request_id,
            "operation-fi01-cold-reopen",
            &json!({"action":"write"}),
            "t-fi01",
        )
        .expect("retry after cold rollback");
    assert_eq!(
        reopened
            .operation(&request)
            .expect("post-retry lookup")
            .expect("durable retry intent")
            .phase,
        "intent_durable"
    );
    drop(reopened);

    let final_open = MetadataStore::open(&path).expect("second cold reopen");
    assert_eq!(
        final_open
            .operation(&request)
            .expect("final lookup")
            .expect("exactly one durable intent")
            .operation_id,
        "operation-fi01-cold-reopen"
    );
}

#[test]
fn intent_after_commit_returns_fault_but_survives_cold_reopen() {
    let directory = tempdir().expect("directory");
    let path = directory.path().join("metadata.sqlite");
    let request = key("req_intent_after");
    {
        let mut metadata = MetadataStore::open_with_failpoints(
            &path,
            MetadataFailpoints::once(MetadataFailpoint::AfterIntentCommit),
        )
        .expect("metadata");
        let error = metadata
            .record_intent(
                &request.project_id,
                &request.actor_id,
                &request.request_id,
                "operation-intent-after",
                &json!({"action":"write"}),
                "t1",
            )
            .expect_err("post-commit fault");
        assert_fault(error, "after_intent_commit");
        assert_eq!(
            metadata
                .operation(&request)
                .expect("same-process lookup")
                .expect("durable intent")
                .phase,
            "intent_durable"
        );
    }

    let metadata = MetadataStore::open(&path).expect("cold reopen");
    assert_eq!(
        metadata
            .operation(&request)
            .expect("reopened lookup")
            .expect("intent after restart")
            .phase,
        "intent_durable"
    );
}

#[test]
fn outcome_boundaries_preserve_pre_state_or_post_state() {
    let request = key("req_outcome_boundaries");
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    record_intent(&mut metadata, &request.request_id);

    metadata.set_failpoints(MetadataFailpoints::once(
        MetadataFailpoint::BeforeOutcomeCommit,
    ));
    let error = metadata
        .record_outcome(&request, "outcome_durable", &json!({"status":"ok"}))
        .expect_err("pre-outcome fault");
    assert_fault(error, "before_outcome_commit");
    assert_eq!(
        metadata
            .operation(&request)
            .expect("pre-state lookup")
            .expect("intent")
            .phase,
        "intent_durable"
    );

    metadata.set_failpoints(MetadataFailpoints::once(
        MetadataFailpoint::AfterOutcomeCommit,
    ));
    let error = metadata
        .record_outcome(&request, "outcome_durable", &json!({"status":"ok"}))
        .expect_err("post-outcome fault");
    assert_fault(error, "after_outcome_commit");
    assert_eq!(
        metadata
            .operation(&request)
            .expect("post-state lookup")
            .expect("outcome")
            .phase,
        "outcome_durable"
    );
}

#[test]
fn recovery_boundaries_never_fabricate_success_and_converge_to_unknown() {
    let request = key("req_recovery_boundaries");
    let mut metadata = MetadataStore::in_memory().expect("metadata");
    record_intent(&mut metadata, &request.request_id);

    metadata.set_failpoints(MetadataFailpoints::once(
        MetadataFailpoint::BeforeRecoveryCommit,
    ));
    let error = metadata
        .recover_unfinished_operations()
        .expect_err("pre-recovery fault");
    assert_fault(error, "before_recovery_commit");
    assert_eq!(
        metadata
            .operation(&request)
            .expect("pre-recovery lookup")
            .expect("intent")
            .phase,
        "intent_durable"
    );

    metadata.set_failpoints(MetadataFailpoints::once(
        MetadataFailpoint::AfterRecoveryCommit,
    ));
    let error = metadata
        .recover_unfinished_operations()
        .expect_err("post-recovery fault");
    assert_fault(error, "after_recovery_commit");
    assert_eq!(
        metadata
            .operation(&request)
            .expect("post-recovery lookup")
            .expect("unknown operation")
            .phase,
        "unknown"
    );
    assert_eq!(
        metadata.unknown_operations().expect("unknown list").len(),
        1
    );
    assert!(metadata
        .recover_unfinished_operations()
        .expect("convergent retry")
        .is_empty());
}

#[test]
fn sqlite_commit_failpoints_cover_wal_pre_and_post_visibility() {
    let directory = tempdir().expect("directory");
    let path = directory.path().join("metadata.sqlite");

    {
        let mut metadata = MetadataStore::open_with_failpoints(
            &path,
            MetadataFailpoints::once(MetadataFailpoint::BeforeSqliteCommit),
        )
        .expect("metadata");
        let error = metadata
            .compare_and_swap_ref("refs/heads/main", None, "commit-before", "t1")
            .expect_err("pre-commit fault");
        assert_fault(error, "before_sqlite_commit");
        assert_eq!(metadata.get_ref("refs/heads/main").expect("ref"), None);
    }
    assert_eq!(
        MetadataStore::open(&path)
            .expect("reopen after rollback")
            .get_ref("refs/heads/main")
            .expect("ref after rollback"),
        None
    );

    {
        let mut metadata = MetadataStore::open_with_failpoints(
            &path,
            MetadataFailpoints::once(MetadataFailpoint::AfterSqliteCommit),
        )
        .expect("metadata");
        let error = metadata
            .compare_and_swap_ref("refs/heads/main", None, "commit-after", "t2")
            .expect_err("post-commit fault");
        assert_fault(error, "after_sqlite_commit");
        assert_eq!(
            metadata.get_ref("refs/heads/main").expect("ref"),
            Some("commit-after".into())
        );
    }
    assert_eq!(
        MetadataStore::open(&path)
            .expect("reopen after commit")
            .get_ref("refs/heads/main")
            .expect("ref after commit"),
        Some("commit-after".into())
    );
}
