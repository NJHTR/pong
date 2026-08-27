use pong_core::cas::CasFailpoints;
use pong_core::metadata::{MetadataFailpoint, MetadataFailpoints};
use pong_core::{PongError, Repository};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn startup_recovery_fault_is_retried_after_cold_reopen_and_converges() {
    let project = tempdir().expect("project");
    let mut repository = Repository::init(project.path()).expect("init");
    repository
        .metadata_mut()
        .record_intent(
            "project-startup-fault",
            "agent-startup-fault",
            "request-startup-fault",
            "operation-startup-fault",
            &json!({"action":"write"}),
            "2026-08-19T00:00:00Z",
        )
        .expect("intent");
    drop(repository);

    let error = Repository::open_with_failpoints(
        project.path(),
        MetadataFailpoints::once(MetadataFailpoint::BeforeRecoveryCommit),
        CasFailpoints::disabled(),
    )
    .expect_err("startup recovery fault");
    assert_eq!(error.code(), "FAULT_INJECTED");
    assert!(matches!(error, PongError::FaultInjected(_)));

    // A cold reopen with the default, disabled schedules must classify the
    // durable orphan as unknown exactly once and then converge.
    let repository = Repository::open(project.path()).expect("retry startup");
    let unknown = repository
        .metadata()
        .unknown_operations()
        .expect("unknown operations");
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].request_id, "request-startup-fault");
    assert!(repository
        .metadata()
        .unfinished_operations()
        .expect("pending")
        .is_empty());
}

#[test]
fn repository_cas_quota_fixture_is_one_shot_and_cleans_staging() {
    let project = tempdir().expect("project");
    let repository = Repository::init(project.path()).expect("init");
    repository.cas().set_failpoints(CasFailpoints::once(
        pong_core::cas::CasFailPoint::StagingWrite,
        pong_core::cas::CasFaultAction::QuotaExhausted(2),
    ));

    let error = repository
        .cas()
        .put("snapshot/v1", b"repository quota fixture")
        .expect_err("quota fixture must fail closed");
    assert_eq!(error.code(), "RESOURCE_EXHAUSTED");
    assert!(error.to_string().contains("quota_exhausted"));
    assert_eq!(repository.cas().failpoints().armed(), None);
    assert_eq!(
        repository
            .layout()
            .staging_dir()
            .read_dir()
            .unwrap()
            .count(),
        0
    );
}
