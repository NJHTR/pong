use pong_core::cas::CasFailpoints;
use pong_core::metadata::{
    EventRecord, IdempotencyKey, JournalOperation, MetadataFailpoint, MetadataFailpoints, NewEvent,
};
use pong_core::{
    MigrationFailpoint, MigrationFailpoints, MigrationSpec, PongError, Repository,
    GENERATION_REPOSITORY_FORMAT, REPOSITORY_FORMAT,
};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::{
    Config, FileFailurePersistence, RngAlgorithm, TestCaseError, TestRng, TestRunner,
};
use serde_json::json;
use std::env;
use std::fmt::Debug;
use tempfile::tempdir;

type CaseResult<T = ()> = Result<T, String>;

const CI_PROPERTY_CASES: u32 = 10_000;

const MIGRATION_POINTS: [(MigrationFailpoint, bool, u8, &str); 9] = [
    (
        MigrationFailpoint::Preflight,
        false,
        0x31,
        "proptest-regressions/property_recovery_migration_pt14_preflight.txt",
    ),
    (
        MigrationFailpoint::TargetAllocated,
        false,
        0x32,
        "proptest-regressions/property_recovery_migration_pt14_target_allocated.txt",
    ),
    (
        MigrationFailpoint::AfterBackup,
        false,
        0x33,
        "proptest-regressions/property_recovery_migration_pt14_after_backup.txt",
    ),
    (
        MigrationFailpoint::AfterCheckpoint,
        false,
        0x34,
        "proptest-regressions/property_recovery_migration_pt14_after_checkpoint.txt",
    ),
    (
        MigrationFailpoint::AfterVerify,
        false,
        0x35,
        "proptest-regressions/property_recovery_migration_pt14_after_verify.txt",
    ),
    (
        MigrationFailpoint::BeforeSelectorReplace,
        false,
        0x36,
        "proptest-regressions/property_recovery_migration_pt14_before_selector.txt",
    ),
    (
        MigrationFailpoint::AfterSelectorReplace,
        true,
        0x37,
        "proptest-regressions/property_recovery_migration_pt14_after_selector.txt",
    ),
    (
        MigrationFailpoint::AfterSelectorDirectorySync,
        true,
        0x38,
        "proptest-regressions/property_recovery_migration_pt14_selector_sync.txt",
    ),
    (
        MigrationFailpoint::AfterJournalFinalize,
        true,
        0x39,
        "proptest-regressions/property_recovery_migration_pt14_journal_finalize.txt",
    ),
];

#[derive(Clone, Debug)]
struct RecoveryOperation {
    durable_outcome: bool,
    recover_after_record: bool,
    payload_tag: u16,
    schema_selector: u8,
}

#[derive(Clone, Copy, Debug)]
enum RecoveryBoundary {
    None,
    BeforeCommit,
    AfterCommit,
}

#[derive(Clone, Debug)]
struct RecoveryCase {
    operations: Vec<RecoveryOperation>,
    boundary: RecoveryBoundary,
    additional_passes: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecoverySnapshot {
    operations: Vec<Option<JournalOperation>>,
    unknown: Vec<JournalOperation>,
    unfinished: Vec<JournalOperation>,
    events: Vec<EventRecord>,
}

#[derive(Clone, Copy, Debug)]
enum UnknownSchedule {
    BeforeIntentCommit,
    AfterIntentCommit,
    BeforeOutcomeCommit,
    AfterOutcomeCommit,
    BeforeRecoveryCommit,
    AfterRecoveryCommit,
    IntentOnly,
    OutcomeDurable,
}

#[derive(Clone, Debug)]
struct UnknownCase {
    schedule: UnknownSchedule,
    nonce: u32,
    nested_payload: Vec<u8>,
}

#[derive(Clone, Debug)]
struct MigrationEvent {
    payload_tag: u16,
    flag: bool,
    schema_selector: u8,
}

#[derive(Clone, Debug)]
struct MigrationCase {
    events: Vec<MigrationEvent>,
    ref_values: Vec<u16>,
    opaque_tag: u32,
}

#[test]
fn pt09_recovery_converges_across_interleavings_and_commit_boundaries() {
    run_property(
        configured_cases(64),
        0x19,
        "proptest-regressions/property_recovery_migration_pt09.txt",
        "pt09_recovery_converges_across_interleavings_and_commit_boundaries",
        recovery_strategy(),
        run_recovery_case,
    );
}

#[test]
fn pt10_crash_schedules_never_turn_an_unresolved_intent_into_success() {
    run_property(
        configured_cases(64),
        0x20,
        "proptest-regressions/property_recovery_migration_pt10.txt",
        "pt10_crash_schedules_never_turn_an_unresolved_intent_into_success",
        unknown_strategy(),
        run_unknown_case,
    );
}

#[test]
fn pt14_migration_failpoints_are_old_or_verified_new_and_retry_idempotently() {
    let total_cases = configured_cases(18);
    // The single PT-14 property distributes its deterministic corpus across
    // all named interruption points so every point is exercised while the
    // aggregate property count remains the configured 10,000 CI cases.
    let cases_per_point = total_cases.div_ceil(MIGRATION_POINTS.len() as u32);

    for (point, selector_committed, seed, persistence) in MIGRATION_POINTS {
        run_property(
            cases_per_point,
            seed,
            persistence,
            "pt14_migration_failpoints_are_old_or_verified_new_and_retry_idempotently",
            migration_strategy(),
            move |case| run_migration_case(case, point, selector_committed),
        );
    }
}

fn recovery_strategy() -> impl Strategy<Value = RecoveryCase> {
    (
        vec((any::<bool>(), any::<bool>(), any::<u16>(), 0_u8..2), 1..9),
        0_u8..3,
        0_u8..5,
    )
        .prop_map(|(operations, boundary, additional_passes)| RecoveryCase {
            operations: operations
                .into_iter()
                .map(
                    |(durable_outcome, recover_after_record, payload_tag, schema_selector)| {
                        RecoveryOperation {
                            durable_outcome,
                            recover_after_record,
                            payload_tag,
                            schema_selector,
                        }
                    },
                )
                .collect(),
            boundary: match boundary {
                0 => RecoveryBoundary::None,
                1 => RecoveryBoundary::BeforeCommit,
                _ => RecoveryBoundary::AfterCommit,
            },
            additional_passes,
        })
}

fn unknown_strategy() -> impl Strategy<Value = UnknownCase> {
    (0_u8..8, any::<u32>(), vec(any::<u8>(), 0..9)).prop_map(|(schedule, nonce, nested_payload)| {
        UnknownCase {
            schedule: match schedule {
                0 => UnknownSchedule::BeforeIntentCommit,
                1 => UnknownSchedule::AfterIntentCommit,
                2 => UnknownSchedule::BeforeOutcomeCommit,
                3 => UnknownSchedule::AfterOutcomeCommit,
                4 => UnknownSchedule::BeforeRecoveryCommit,
                5 => UnknownSchedule::AfterRecoveryCommit,
                6 => UnknownSchedule::IntentOnly,
                _ => UnknownSchedule::OutcomeDurable,
            },
            nonce,
            nested_payload,
        }
    })
}

fn migration_strategy() -> impl Strategy<Value = MigrationCase> {
    (
        vec((any::<u16>(), any::<bool>(), 0_u8..2), 0..6),
        vec(any::<u16>(), 0..5),
        any::<u32>(),
    )
        .prop_map(|(events, ref_values, opaque_tag)| MigrationCase {
            events: events
                .into_iter()
                .map(|(payload_tag, flag, schema_selector)| MigrationEvent {
                    payload_tag,
                    flag,
                    schema_selector,
                })
                .collect(),
            ref_values,
            opaque_tag,
        })
}

fn run_recovery_case(case: RecoveryCase) -> CaseResult {
    let project = tempdir().map_err(|error| format!("create PT-09 project: {error}"))?;
    let mut repository = Repository::init(project.path())
        .map_err(|error| format!("initialize PT-09 repository: {error}"))?;
    let mut keys = Vec::with_capacity(case.operations.len());
    let mut expected_events = Vec::with_capacity(case.operations.len());

    for (index, operation) in case.operations.iter().enumerate() {
        let key = recovery_key(index);
        repository
            .metadata_mut()
            .record_intent(
                &key.project_id,
                &key.actor_id,
                &key.request_id,
                &format!("operation-{index:04}"),
                &json!({
                    "action": "property-recovery",
                    "opaque_extension": {
                        "tag": operation.payload_tag,
                        "variant": operation.schema_selector,
                    }
                }),
                &format!("time-{index:04}"),
            )
            .map_err(|error| format!("record PT-09 intent {index}: {error}"))?;
        if operation.durable_outcome {
            repository
                .metadata_mut()
                .record_outcome(
                    &key,
                    "outcome_durable",
                    &json!({"status": "accepted", "tag": operation.payload_tag}),
                )
                .map_err(|error| format!("record PT-09 outcome {index}: {error}"))?;
        }
        let event = repository
            .metadata_mut()
            .append_event(NewEvent {
                event_id: format!("event-recovery-{index:04}"),
                project_id: "project-property-recovery".into(),
                stream_id: "stream-property-recovery".into(),
                schema_version: if operation.schema_selector == 0 {
                    "0.0".into()
                } else {
                    "0.1".into()
                },
                actor_id: Some(key.actor_id.clone()),
                request_id: Some(key.request_id.clone()),
                occurred_at: format!("time-{index:04}"),
                payload: json!({
                    "type": "property.recovery",
                    "opaque_extension": {
                        "tag": operation.payload_tag,
                        "open_enum": format!("vendor-{}", operation.schema_selector),
                    }
                }),
            })
            .map_err(|error| format!("append PT-09 event {index}: {error}"))?;
        expected_events.push(event);
        keys.push(key);

        if operation.recover_after_record {
            repository
                .recover_unfinished_operations()
                .map_err(|error| format!("interleaved PT-09 recovery {index}: {error}"))?;
        }
    }
    drop(repository);

    if let Some(point) = recovery_failpoint(case.boundary) {
        let error = match Repository::open_with_failpoints(
            project.path(),
            MetadataFailpoints::once(point),
            CasFailpoints::disabled(),
        ) {
            Ok(_) => return Err(format!("PT-09 failpoint {point:?} did not fire")),
            Err(error) => error,
        };
        require_equal(error.code(), "FAULT_INJECTED", "PT-09 injected error code")?;
    }

    // This open is the normal startup pass. The first snapshot therefore
    // represents zero additional recovery calls by the caller.
    let mut repository = Repository::open(project.path())
        .map_err(|error| format!("cold-open PT-09 repository: {error}"))?;
    let zero_additional = recovery_snapshot(&repository, &keys)?;
    require_equal(
        &zero_additional.events,
        &expected_events,
        "PT-09 immutable event set after startup recovery",
    )?;
    assert_recovery_phases(&zero_additional, &case.operations)?;

    let first_repeat = repository
        .recover_unfinished_operations()
        .map_err(|error| format!("first repeated PT-09 recovery: {error}"))?;
    require(
        first_repeat.is_empty(),
        "PT-09 repeated recovery changed state",
    )?;
    let one_additional = recovery_snapshot(&repository, &keys)?;
    require_equal(
        &one_additional,
        &zero_additional,
        "PT-09 zero and one additional recovery passes",
    )?;

    // Always execute at least one more pass, then add the generated number.
    // This compares startup, one explicit pass, and many explicit passes.
    for pass in 0..=case.additional_passes {
        let recovered = repository
            .recover_unfinished_operations()
            .map_err(|error| format!("repeated PT-09 recovery pass {pass}: {error}"))?;
        require(
            recovered.is_empty(),
            format!("PT-09 recovery pass {pass} was not convergent"),
        )?;
    }
    let many_additional = recovery_snapshot(&repository, &keys)?;
    require_equal(
        &many_additional,
        &zero_additional,
        "PT-09 repeated recovery snapshot",
    )
}

fn run_unknown_case(case: UnknownCase) -> CaseResult {
    let project = tempdir().map_err(|error| format!("create PT-10 project: {error}"))?;
    let mut repository = Repository::init(project.path())
        .map_err(|error| format!("initialize PT-10 repository: {error}"))?;
    let key = IdempotencyKey {
        project_id: "project-property-unknown".into(),
        actor_id: "actor-property-unknown".into(),
        request_id: format!("request-{:08x}", case.nonce),
    };
    let payload = json!({
        "action": "external-write",
        "nonce": case.nonce,
        "opaque_extension": case.nested_payload,
    });

    match case.schedule {
        UnknownSchedule::BeforeIntentCommit | UnknownSchedule::AfterIntentCommit => {
            let point = if matches!(case.schedule, UnknownSchedule::BeforeIntentCommit) {
                MetadataFailpoint::BeforeIntentCommit
            } else {
                MetadataFailpoint::AfterIntentCommit
            };
            repository
                .metadata_mut()
                .set_failpoints(MetadataFailpoints::once(point));
            let error = repository
                .metadata_mut()
                .record_intent(
                    &key.project_id,
                    &key.actor_id,
                    &key.request_id,
                    "operation-property-unknown",
                    &payload,
                    "time-unknown",
                )
                .expect_err("PT-10 intent failpoint must fire");
            require_equal(error.code(), "FAULT_INJECTED", "PT-10 intent fault code")?;
        }
        UnknownSchedule::BeforeOutcomeCommit | UnknownSchedule::AfterOutcomeCommit => {
            record_unknown_intent(&mut repository, &key, &payload)?;
            let point = if matches!(case.schedule, UnknownSchedule::BeforeOutcomeCommit) {
                MetadataFailpoint::BeforeOutcomeCommit
            } else {
                MetadataFailpoint::AfterOutcomeCommit
            };
            repository
                .metadata_mut()
                .set_failpoints(MetadataFailpoints::once(point));
            let error = repository
                .metadata_mut()
                .record_outcome(&key, "outcome_durable", &json!({"status": "accepted"}))
                .expect_err("PT-10 outcome failpoint must fire");
            require_equal(error.code(), "FAULT_INJECTED", "PT-10 outcome fault code")?;
        }
        UnknownSchedule::BeforeRecoveryCommit | UnknownSchedule::AfterRecoveryCommit => {
            record_unknown_intent(&mut repository, &key, &payload)?;
            drop(repository);
            let point = if matches!(case.schedule, UnknownSchedule::BeforeRecoveryCommit) {
                MetadataFailpoint::BeforeRecoveryCommit
            } else {
                MetadataFailpoint::AfterRecoveryCommit
            };
            let error = match Repository::open_with_failpoints(
                project.path(),
                MetadataFailpoints::once(point),
                CasFailpoints::disabled(),
            ) {
                Ok(_) => return Err(format!("PT-10 recovery failpoint {point:?} did not fire")),
                Err(error) => error,
            };
            require_equal(error.code(), "FAULT_INJECTED", "PT-10 recovery fault code")?;
            return assert_unknown_after_restart(project.path(), &key, case.schedule);
        }
        UnknownSchedule::IntentOnly => record_unknown_intent(&mut repository, &key, &payload)?,
        UnknownSchedule::OutcomeDurable => {
            record_unknown_intent(&mut repository, &key, &payload)?;
            repository
                .metadata_mut()
                .record_outcome(&key, "outcome_durable", &json!({"status": "accepted"}))
                .map_err(|error| format!("record PT-10 durable outcome: {error}"))?;
        }
    }
    drop(repository);
    assert_unknown_after_restart(project.path(), &key, case.schedule)
}

fn assert_unknown_after_restart(
    root: &std::path::Path,
    key: &IdempotencyKey,
    schedule: UnknownSchedule,
) -> CaseResult {
    let mut repository =
        Repository::open(root).map_err(|error| format!("cold-open PT-10 repository: {error}"))?;
    let operation = repository
        .metadata()
        .operation(key)
        .map_err(|error| format!("read PT-10 operation: {error}"))?;

    let expected_phase = match schedule {
        UnknownSchedule::BeforeIntentCommit => None,
        UnknownSchedule::AfterOutcomeCommit | UnknownSchedule::OutcomeDurable => {
            Some("outcome_durable")
        }
        _ => Some("unknown"),
    };
    require_equal(
        operation.as_ref().map(|operation| operation.phase.as_str()),
        expected_phase,
        "PT-10 post-restart phase",
    )?;
    require(
        operation
            .as_ref()
            .map_or(true, |operation| operation.phase != "succeeded"),
        "PT-10 fabricated a succeeded phase",
    )?;

    if expected_phase == Some("unknown") {
        let unknown = repository
            .metadata()
            .unknown_operations()
            .map_err(|error| format!("list PT-10 unknown operations: {error}"))?;
        require_equal(unknown.len(), 1, "PT-10 unknown operation count")?;
        let error = repository
            .metadata_mut()
            .record_outcome(key, "published", &json!({"status": "must-not-publish"}))
            .expect_err("unknown must remain terminal");
        require_equal(
            error.code(),
            "CONFLICT",
            "PT-10 unknown terminal transition",
        )?;
    }
    Ok(())
}

fn run_migration_case(
    case: MigrationCase,
    point: MigrationFailpoint,
    selector_committed: bool,
) -> CaseResult {
    let project = tempdir().map_err(|error| format!("create PT-14 project: {error}"))?;
    let mut repository = Repository::init(project.path())
        .map_err(|error| format!("initialize PT-14 repository: {error}"))?;
    let mut expected_events = Vec::with_capacity(case.events.len());
    for (index, event) in case.events.iter().enumerate() {
        let record = repository
            .metadata_mut()
            .append_event(NewEvent {
                event_id: format!("event-migration-{index:04}"),
                project_id: "project-property-migration".into(),
                stream_id: "stream-property-migration".into(),
                schema_version: if event.schema_selector == 0 {
                    "0.0".into()
                } else {
                    "0.1".into()
                },
                actor_id: Some("actor-property-migration".into()),
                request_id: Some(format!("request-migration-{index:04}")),
                occurred_at: format!("time-migration-{index:04}"),
                payload: json!({
                    "type": "property.migration",
                    "flag": event.flag,
                    "opaque_extension": {
                        "tag": event.payload_tag,
                        "case": case.opaque_tag,
                        "future_enum": format!("provider-{}", event.schema_selector),
                    }
                }),
            })
            .map_err(|error| format!("append PT-14 event {index}: {error}"))?;
        expected_events.push(record);
    }
    let mut expected_refs = Vec::with_capacity(case.ref_values.len());
    for (index, value) in case.ref_values.iter().enumerate() {
        let name = format!("refs/property/{index:04}");
        let value = format!("commit-{:04x}-{:08x}", value, case.opaque_tag);
        repository
            .metadata_mut()
            .compare_and_swap_ref(&name, None, &value, &format!("time-ref-{index:04}"))
            .map_err(|error| format!("record PT-14 ref {index}: {error}"))?;
        expected_refs.push((name, value));
    }
    drop(repository);

    let spec = MigrationSpec::new("migration-property", "generation-property");
    let error = Repository::migrate_with_failpoints(
        project.path(),
        spec.clone(),
        MigrationFailpoints::once(point),
    )
    .expect_err("PT-14 migration failpoint must fire");
    if env::var_os("PONG_PT14_TRACE").is_some() {
        eprintln!(
            "{}",
            json!({
                "record": "pt14-failpoint",
                "operation": "migration",
                "stage": format!("{point:?}"),
                "failpoint": format!("{point:?}"),
                "code": error.code(),
                "raw_os_error": error.raw_os_error(),
                "expected_selector_committed": selector_committed
            })
        );
    }
    // On Windows, flushing a directory can be rejected by the host while the
    // selector boundary is being exercised. Treat that as a real storage
    // interruption at the same atomic point; the cold-open oracle below still
    // requires old-only or new-only visibility and the retry must converge.
    let host_sync_interruption = point == MigrationFailpoint::AfterSelectorDirectorySync
        && error.code() == "PERMISSION_DENIED";
    require(
        error.code() == "FAULT_INJECTED" || host_sync_interruption,
        format!("PT-14 injected error code was {} ({error:?})", error.code()),
    )?;

    let visible = retry_repository_open(
        project.path(),
        &format!("cold_open_after_failpoint/{point:?}"),
    )
    .map_err(|error| format!("cold-open PT-14 repository after {point:?}: {error}"))?;
    let observed_selector_committed = visible.active_generation_id().is_some();
    if !host_sync_interruption {
        require_equal(
            observed_selector_committed,
            selector_committed,
            "PT-14 failpoint selector visibility",
        )?;
    }
    assert_migration_state(
        &visible,
        observed_selector_committed,
        &expected_events,
        &expected_refs,
    )?;
    if env::var_os("PONG_PT14_TRACE").is_some() {
        eprintln!(
            "{}",
            json!({
                "record": "pt14-cold-reopen",
                "operation": "repository_open",
                "stage": format!("cold_open_after_failpoint/{point:?}"),
                "failpoint": format!("{point:?}"),
                "active_generation": observed_selector_committed,
                "old_or_new_only": true
            })
        );
    }
    drop(visible);

    let first_retry = retry_repository_migrate(
        project.path(),
        spec.clone(),
        &format!("migration_retry_after_failpoint/{point:?}"),
    )
    .map_err(|error| format!("retry PT-14 migration after {point:?}: {error}"))?;
    require_equal(
        first_retry.already_active,
        observed_selector_committed,
        "PT-14 first retry active status",
    )?;
    let migrated = retry_repository_open(
        project.path(),
        &format!("cold_open_after_first_retry/{point:?}"),
    )
    .map_err(|error| format!("open PT-14 migrated repository after {point:?}: {error}"))?;
    assert_migration_state(&migrated, true, &expected_events, &expected_refs)?;
    drop(migrated);

    let second_retry = retry_repository_migrate(
        project.path(),
        spec,
        &format!("migration_second_retry/{point:?}"),
    )
    .map_err(|error| format!("second PT-14 retry after {point:?}: {error}"))?;
    require(
        second_retry.already_active,
        "PT-14 second retry rebuilt state",
    )?;
    require_equal(
        second_retry.plan_digest,
        first_retry.plan_digest,
        "PT-14 stable plan digest",
    )?;
    require_equal(
        second_retry.selector_digest,
        first_retry.selector_digest,
        "PT-14 stable selector digest",
    )?;
    require_equal(
        second_retry.generation_manifest_digest,
        first_retry.generation_manifest_digest,
        "PT-14 stable generation manifest digest",
    )?;

    let final_repository =
        retry_repository_open(project.path(), &format!("final_cold_open/{point:?}"))
            .map_err(|error| format!("final PT-14 cold open after {point:?}: {error}"))?;
    assert_migration_state(&final_repository, true, &expected_events, &expected_refs)?;
    if env::var_os("PONG_PT14_TRACE").is_some() {
        eprintln!(
            "{}",
            json!({
                "record": "pt14-final-cold-reopen",
                "operation": "repository_open",
                "stage": format!("final_cold_open/{point:?}"),
                "failpoint": format!("{point:?}"),
                "active_generation": true,
                "old_or_new_only": true,
                "selector_manifest_identity": "verified"
            })
        );
    }
    require(
        project.path().join(".pong/metadata.sqlite").is_file(),
        "PT-14 migration removed the legacy source",
    )
}

fn retry_repository_open(root: &std::path::Path, operation: &str) -> Result<Repository, PongError> {
    retry_storage_operation(operation, || Repository::open(root))
}

fn retry_repository_migrate(
    root: &std::path::Path,
    spec: MigrationSpec,
    operation: &str,
) -> Result<pong_core::MigrationOutcome, PongError> {
    retry_storage_operation(operation, || Repository::migrate(root, spec.clone()))
}

/// Retry only Windows host-level sharing/access races. Protocol, integrity,
/// ACL, and resource failures remain immediate property failures. Set
/// `PONG_PT14_TRACE=1` to emit one redacted JSON record per operation with
/// stage, retry count, raw OS error, and the terminal Pong status.
fn retry_storage_operation<T, F>(operation_name: &str, mut operation: F) -> Result<T, PongError>
where
    F: FnMut() -> Result<T, PongError>,
{
    let tracing = env::var_os("PONG_PT14_TRACE").is_some();
    let mut failures = Vec::new();
    for attempt in 0..8u32 {
        match operation() {
            Ok(value) => {
                if tracing {
                    eprintln!(
                        "{}",
                        json!({
                            "record": "pt14-retry",
                            "operation": operation_name,
                            "attempts": attempt + 1,
                            "failures": failures,
                            "result": "success"
                        })
                    );
                }
                return Ok(value);
            }
            Err(error) if is_retryable_storage_race(&error) && attempt + 1 < 8 => {
                failures.push(json!({
                    "attempt": attempt + 1,
                    "code": error.code(),
                    "raw_os_error": error.raw_os_error(),
                    "retryable": true
                }));
                std::thread::sleep(std::time::Duration::from_millis(5 * u64::from(attempt + 1)));
            }
            Err(error) => {
                if tracing {
                    eprintln!(
                        "{}",
                        json!({
                            "record": "pt14-retry",
                            "operation": operation_name,
                            "attempts": attempt + 1,
                            "failures": failures,
                            "terminal": {
                                "code": error.code(),
                                "raw_os_error": error.raw_os_error()
                            },
                            "result": "error"
                        })
                    );
                }
                return Err(error);
            }
        }
    }
    unreachable!("retry loop returns on success or terminal error")
}

fn is_retryable_storage_race(error: &PongError) -> bool {
    // A protected boundary deliberately maps a terminal access-denied result
    // to `PermissionDeniedWithOsError`; retrying that domain error would hide
    // a real ACL failure. Only an unclassified I/O error with one of the
    // documented Windows sharing codes is eligible for this test-level retry.
    cfg!(windows)
        && matches!(error, PongError::Io(_))
        && matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

#[test]
fn pt14_retry_does_not_retry_terminal_protected_denial() {
    let mut calls = 0;
    let result: Result<(), PongError> = retry_storage_operation("terminal_acl", || {
        calls += 1;
        Err(PongError::PermissionDeniedWithOsError {
            message: "protected repository access was denied".into(),
            raw_os_error: 5,
        })
    });
    let error = result.expect_err("terminal protected denial must be returned");
    assert_eq!(calls, 1);
    assert_eq!(error.code(), "PERMISSION_DENIED");
    assert_eq!(error.raw_os_error(), Some(5));
}

fn assert_migration_state(
    repository: &Repository,
    active_generation: bool,
    expected_events: &[EventRecord],
    expected_refs: &[(String, String)],
) -> CaseResult {
    let expected_format = if active_generation {
        GENERATION_REPOSITORY_FORMAT
    } else {
        REPOSITORY_FORMAT
    };
    require_equal(
        repository.marker().repository_format.as_str(),
        expected_format,
        "PT-14 visible repository format",
    )?;
    require_equal(
        repository.active_generation_id(),
        active_generation.then_some("generation-property"),
        "PT-14 active generation selector",
    )?;
    let actual_events = repository
        .metadata()
        .list_events("stream-property-migration")
        .map_err(|error| format!("list PT-14 events: {error}"))?;
    require_equal(
        actual_events.as_slice(),
        expected_events,
        "PT-14 migrated event semantics",
    )?;
    for (name, value) in expected_refs {
        require_equal(
            repository
                .metadata()
                .get_ref(name)
                .map_err(|error| format!("read PT-14 ref {name}: {error}"))?,
            Some(value.clone()),
            "PT-14 migrated ref semantics",
        )?;
    }
    if active_generation {
        let selector = repository
            .selector()
            .ok_or_else(|| "PT-14 active repository has no selector".to_string())?;
        let manifest = repository
            .generation_manifest()
            .ok_or_else(|| "PT-14 active repository has no manifest".to_string())?;
        require_equal(
            selector.active_generation.as_str(),
            manifest.generation_id.as_str(),
            "PT-14 selector/manifest generation identity",
        )?;
        require_equal(
            selector.migration_id.as_str(),
            manifest.migration_id.as_str(),
            "PT-14 selector/manifest migration identity",
        )?;
    }
    Ok(())
}

fn recovery_key(index: usize) -> IdempotencyKey {
    IdempotencyKey {
        project_id: "project-property-recovery".into(),
        actor_id: "actor-property-recovery".into(),
        request_id: format!("request-recovery-{index:04}"),
    }
}

fn recovery_snapshot(
    repository: &Repository,
    keys: &[IdempotencyKey],
) -> CaseResult<RecoverySnapshot> {
    let operations = keys
        .iter()
        .map(|key| {
            repository
                .metadata()
                .operation(key)
                .map_err(|error| format!("read PT-09 operation {}: {error}", key.request_id))
        })
        .collect::<CaseResult<Vec<_>>>()?;
    Ok(RecoverySnapshot {
        operations,
        unknown: repository
            .metadata()
            .unknown_operations()
            .map_err(|error| format!("list PT-09 unknown operations: {error}"))?,
        unfinished: repository
            .metadata()
            .unfinished_operations()
            .map_err(|error| format!("list PT-09 unfinished operations: {error}"))?,
        events: repository
            .metadata()
            .list_events("stream-property-recovery")
            .map_err(|error| format!("list PT-09 events: {error}"))?,
    })
}

fn assert_recovery_phases(
    snapshot: &RecoverySnapshot,
    operations: &[RecoveryOperation],
) -> CaseResult {
    for (index, (actual, expected)) in snapshot.operations.iter().zip(operations).enumerate() {
        let expected_phase = if expected.durable_outcome {
            "outcome_durable"
        } else {
            "unknown"
        };
        let actual_phase = actual
            .as_ref()
            .ok_or_else(|| format!("PT-09 operation {index} disappeared"))?
            .phase
            .as_str();
        require_equal(
            actual_phase,
            expected_phase,
            "PT-09 recovered operation phase",
        )?;
        require(
            actual_phase != "succeeded",
            format!("PT-09 operation {index} fabricated success"),
        )?;
    }
    Ok(())
}

fn record_unknown_intent(
    repository: &mut Repository,
    key: &IdempotencyKey,
    payload: &serde_json::Value,
) -> CaseResult {
    repository
        .metadata_mut()
        .record_intent(
            &key.project_id,
            &key.actor_id,
            &key.request_id,
            "operation-property-unknown",
            payload,
            "time-unknown",
        )
        .map_err(|error| format!("record PT-10 intent: {error}"))
}

fn recovery_failpoint(boundary: RecoveryBoundary) -> Option<MetadataFailpoint> {
    match boundary {
        RecoveryBoundary::None => None,
        RecoveryBoundary::BeforeCommit => Some(MetadataFailpoint::BeforeRecoveryCommit),
        RecoveryBoundary::AfterCommit => Some(MetadataFailpoint::AfterRecoveryCommit),
    }
}

fn configured_cases(local_cases: u32) -> u32 {
    if let Ok(value) = env::var("PONG_PROPTEST_CASES") {
        if value.trim().is_empty() {
            return CI_PROPERTY_CASES;
        }
        return value
            .parse::<u32>()
            .ok()
            .filter(|cases| *cases > 0)
            .unwrap_or_else(|| {
                panic!("PONG_PROPTEST_CASES must be a positive u32, got {value:?}")
            });
    }
    if env::var_os("CI").is_some() {
        CI_PROPERTY_CASES
    } else {
        local_cases
    }
}

fn run_property<S, F>(
    cases: u32,
    seed_tag: u8,
    persistence_path: &'static str,
    test_name: &'static str,
    strategy: S,
    test: F,
) where
    S: Strategy,
    S::Value: Debug,
    F: Fn(S::Value) -> CaseResult,
{
    let config = Config {
        cases,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(persistence_path))),
        source_file: Some(file!()),
        test_name: Some(test_name),
        max_shrink_iters: 10_000,
        ..Config::default()
    };
    let seed = [seed_tag; 32];
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &seed);
    let mut runner = TestRunner::new_with_rng(config, rng);
    if let Err(error) = runner.run(&strategy, |value| test(value).map_err(TestCaseError::fail)) {
        panic!(
            "property {test_name} failed (seed byte 0x{seed_tag:02x}, persistence {persistence_path}): {error}"
        );
    }
}

fn require(condition: bool, message: impl Into<String>) -> CaseResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn require_equal<T>(actual: T, expected: T, label: &str) -> CaseResult
where
    T: Debug + PartialEq,
{
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{label} mismatch: actual {actual:?}, expected {expected:?}"
        ))
    }
}
