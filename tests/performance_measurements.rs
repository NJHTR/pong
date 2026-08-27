//! Explicit, measurement-only M1 performance probe.
//!
//! This is intentionally an ignored integration test: it never turns timing
//! noise into a pass/fail release gate. Run it on a quiet machine with
//! `cargo test --locked --test performance_measurements -- --ignored --nocapture`
//! and `PONG_PERF_OUTPUT=artifacts/m1-performance.json` to emit a portable
//! JSON record containing the fixed workload, seed, platform, and raw samples.

use pong_core::cas::Cas;
use pong_core::metadata::{IdempotencyKey, MetadataStore, NewEvent};
use pong_core::repository::{MigrationSpec, REPOSITORY_FORMAT};
use pong_core::workspace::{LocalWorkspace, SnapshotOptions};
use pong_core::Repository;
use serde::Serialize;
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tempfile::tempdir;

const SEED: u64 = 0x4D31_5045_5246_0001;
const OPERATION_COUNT: usize = 64;
const RECOVERY_REPEATS: usize = 8;
const MIGRATION_REPEATS: usize = 8;
const OBJECT_COUNT: usize = 32;
const OBJECT_SIZE: usize = 16 * 1024;

#[derive(Debug, Serialize)]
struct Measurement {
    operation: &'static str,
    samples_ns: Vec<u128>,
    p50_ns: u128,
    p95_ns: u128,
    p99_ns: u128,
    max_ns: u128,
}

#[derive(Debug, Serialize)]
struct PerformanceRecord {
    record_version: &'static str,
    repository_format: &'static str,
    seed: u64,
    workload: Workload,
    platform: Platform,
    measurements: Vec<Measurement>,
    scales: Vec<ScaleMeasurement>,
    cas_bytes: usize,
    cas_elapsed_ns: u128,
    cas_bytes_per_second: u64,
    migration: MigrationMeasurement,
    quota_behavior: &'static str,
    bounded_resource_behavior: &'static str,
    note: &'static str,
}

#[derive(Debug, Serialize)]
struct ScaleMeasurement {
    label: &'static str,
    file_count: usize,
    file_size: usize,
    snapshot_elapsed_ns: u128,
    snapshot_digest: String,
    total_bytes: u64,
}

#[derive(Debug, Serialize)]
struct MigrationMeasurement {
    source_repository_format: &'static str,
    target_repository_format: &'static str,
    source_metadata_bytes: u64,
    target_metadata_bytes: u64,
    samples_ns: Vec<u128>,
    p50_ns: u128,
    p95_ns: u128,
    p99_ns: u128,
    max_ns: u128,
}

#[derive(Debug, Serialize)]
struct Workload {
    metadata_operations: usize,
    recovery_runs: usize,
    cas_objects: usize,
    cas_object_size: usize,
}

#[derive(Debug, Serialize)]
struct Platform {
    os: &'static str,
    arch: &'static str,
    family: &'static str,
    temp_root: String,
    rust_version: String,
}

#[test]
#[ignore = "measurement-only; run explicitly with --ignored"]
fn m1_performance_probe_writes_reproducible_record() {
    let output = env::var_os("PONG_PERF_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/m1-performance.json"));
    let project = tempdir().expect("performance project");
    let metadata_path = project.path().join("metadata.sqlite");
    let mut metadata = MetadataStore::open(metadata_path).expect("metadata");

    let open_project = tempdir().expect("repository-open project");
    let repository = Repository::init(open_project.path()).expect("repository init");
    drop(repository);
    let mut open_samples = Vec::with_capacity(OPERATION_COUNT);
    for _ in 0..OPERATION_COUNT {
        let started = Instant::now();
        let repository = Repository::open(open_project.path()).expect("repository open");
        drop(repository);
        open_samples.push(started.elapsed().as_nanos());
    }

    let mut journal_samples = Vec::with_capacity(OPERATION_COUNT);
    for index in 0..OPERATION_COUNT {
        let key = IdempotencyKey {
            project_id: format!("perf-project-{index}"),
            actor_id: "perf-actor".into(),
            request_id: format!("perf-request-{index}"),
        };
        let started = Instant::now();
        metadata
            .record_intent(
                &key.project_id,
                &key.actor_id,
                &key.request_id,
                &format!("perf-operation-{index}"),
                &json!({"seed": SEED, "index": index}),
                &format!("perf-time-{index}"),
            )
            .expect("intent");
        metadata
            .record_outcome(&key, "outcome_durable", &json!({"status": "accepted"}))
            .expect("outcome");
        journal_samples.push(started.elapsed().as_nanos());
    }

    let mut event_samples = Vec::with_capacity(OPERATION_COUNT);
    for index in 0..OPERATION_COUNT {
        let started = Instant::now();
        metadata
            .append_event(NewEvent {
                event_id: format!("performance-timed-event-{index}"),
                project_id: "performance-timed-project".into(),
                stream_id: format!("performance-timed-stream-{index}"),
                schema_version: "0.1".into(),
                actor_id: Some("perf-actor".into()),
                request_id: Some(format!("performance-timed-request-{index}")),
                occurred_at: format!("performance-timed-time-{index}"),
                payload: json!({"seed": SEED, "index": index}),
            })
            .expect("timed event append");
        event_samples.push(started.elapsed().as_nanos());
    }

    let mut recovery_samples = Vec::with_capacity(RECOVERY_REPEATS);
    for run in 0..RECOVERY_REPEATS {
        let recovery_project = project.path().join(format!("recovery-{run}.sqlite"));
        let mut recovery = MetadataStore::open(&recovery_project).expect("recovery metadata");
        for index in 0..OPERATION_COUNT {
            recovery
                .record_intent(
                    "perf-recovery-project",
                    "perf-actor",
                    &format!("recovery-request-{run}-{index}"),
                    &format!("recovery-operation-{run}-{index}"),
                    &json!({"seed": SEED, "index": index}),
                    &format!("recovery-time-{run}-{index}"),
                )
                .expect("recovery intent");
        }
        let started = Instant::now();
        recovery.recover_unfinished_operations().expect("recovery");
        recovery_samples.push(started.elapsed().as_nanos());
    }

    let cas = Cas::new(project.path().join("cas")).expect("CAS");
    let object = deterministic_bytes(SEED, OBJECT_SIZE);
    let started = Instant::now();
    for index in 0..OBJECT_COUNT {
        let mut bytes = object.clone();
        // Make each object distinct without changing the fixed workload size.
        bytes[..8].copy_from_slice(&(index as u64).to_le_bytes());
        cas.put("performance/object-v1", &bytes).expect("CAS put");
    }
    let cas_elapsed = started.elapsed();
    let cas_bytes = OBJECT_COUNT * OBJECT_SIZE;

    // Include one event append in the probe so the record exercises the same
    // canonical payload path as the event/WAL contract tests.
    metadata
        .append_event(NewEvent {
            event_id: "performance-event".into(),
            project_id: "performance-project".into(),
            stream_id: "performance-stream".into(),
            schema_version: "0.1".into(),
            actor_id: Some("perf-actor".into()),
            request_id: None,
            occurred_at: "performance-time".into(),
            payload: json!({"seed": SEED, "kind": "probe"}),
        })
        .expect("event append");

    let migration = migration_measurements(SEED);

    let scales = snapshot_scale_measurements(SEED);

    let record = PerformanceRecord {
        record_version: "m1-perf-0.3",
        repository_format: REPOSITORY_FORMAT,
        seed: SEED,
        workload: Workload {
            metadata_operations: OPERATION_COUNT,
            recovery_runs: RECOVERY_REPEATS,
            cas_objects: OBJECT_COUNT,
            cas_object_size: OBJECT_SIZE,
        },
        platform: Platform {
            os: env::consts::OS,
            arch: env::consts::ARCH,
            family: env::consts::FAMILY,
            temp_root: "<ephemeral-tempdir>".into(),
            rust_version: rustc_version(),
        },
        measurements: vec![
            summarize("repository_open", open_samples),
            summarize("metadata_intent_plus_outcome", journal_samples),
            summarize("event_append", event_samples),
            summarize("recovery_mark_intents_unknown", recovery_samples),
        ],
        scales,
        cas_bytes,
        cas_elapsed_ns: cas_elapsed.as_nanos(),
        cas_bytes_per_second: bytes_per_second(cas_bytes, cas_elapsed),
        migration,
        quota_behavior: "not measured by this probe; see FI-14 host matrix",
        bounded_resource_behavior: "not measured by this probe",
        note: "measurement-only; no accepted budget or pass/fail threshold",
    };

    write_record(&output, &record).expect("write performance record");
    println!("wrote {}", output.display());
}

fn migration_measurements(seed: u64) -> MigrationMeasurement {
    let mut samples_ns = Vec::with_capacity(MIGRATION_REPEATS);
    let mut source_metadata_bytes = 0;
    let mut target_metadata_bytes = 0;

    for run in 0..MIGRATION_REPEATS {
        let migration_project = tempdir().expect("migration project");
        let mut migration_repository =
            Repository::init(migration_project.path()).expect("migration repository");
        for index in 0..OPERATION_COUNT {
            migration_repository
                .metadata_mut()
                .append_event(NewEvent {
                    event_id: format!("performance-migration-event-{run}-{index}"),
                    project_id: "performance-migration-project".into(),
                    stream_id: "performance-migration-stream".into(),
                    schema_version: "0.1".into(),
                    actor_id: Some("perf-actor".into()),
                    request_id: Some(format!("performance-migration-request-{run}-{index}")),
                    occurred_at: format!("performance-migration-time-{run}-{index}"),
                    payload: json!({"seed": seed, "index": index}),
                })
                .expect("migration event");
        }
        drop(migration_repository);

        let migration_source = migration_project
            .path()
            .join(".pong")
            .join("metadata.sqlite");
        if run == 0 {
            source_metadata_bytes = fs::metadata(migration_source)
                .expect("migration source metadata")
                .len();
        }

        let target_generation = format!("performance-generation-{run}");
        let migration_id = format!("performance-migration-{run}");
        let started = Instant::now();
        Repository::migrate(
            migration_project.path(),
            MigrationSpec::new(migration_id, target_generation.clone()),
        )
        .expect("migration");
        samples_ns.push(started.elapsed().as_nanos());

        if run == 0 {
            target_metadata_bytes = fs::metadata(
                migration_project
                    .path()
                    .join(".pong")
                    .join("generations")
                    .join(target_generation)
                    .join("metadata.sqlite"),
            )
            .expect("migration target metadata")
            .len();
        }
    }

    samples_ns.sort_unstable();
    let p50_ns = percentile(&samples_ns, 50);
    let p95_ns = percentile(&samples_ns, 95);
    let p99_ns = percentile(&samples_ns, 99);
    let max_ns = samples_ns.last().copied().unwrap_or(0);
    MigrationMeasurement {
        source_repository_format: REPOSITORY_FORMAT,
        target_repository_format: "0.2",
        source_metadata_bytes,
        target_metadata_bytes,
        samples_ns,
        p50_ns,
        p95_ns,
        p99_ns,
        max_ns,
    }
}

fn summarize(operation: &'static str, mut samples_ns: Vec<u128>) -> Measurement {
    samples_ns.sort_unstable();
    let p50_ns = percentile(&samples_ns, 50);
    let p95_ns = percentile(&samples_ns, 95);
    let p99_ns = percentile(&samples_ns, 99);
    let max_ns = samples_ns.last().copied().unwrap_or(0);
    Measurement {
        operation,
        samples_ns,
        p50_ns,
        p95_ns,
        p99_ns,
        max_ns,
    }
}

fn snapshot_scale_measurements(seed: u64) -> Vec<ScaleMeasurement> {
    const SCALES: [(&str, usize, usize); 3] = [
        ("small", 4, 1024),
        ("medium", 32, 4096),
        ("large", 128, 16 * 1024),
    ];

    SCALES
        .into_iter()
        .map(|(label, file_count, file_size)| {
            let project = tempdir().expect("snapshot scale project");
            let workspace_root = project.path().join("workspace");
            let workspace = LocalWorkspace::create(
                format!("workspace-performance-{label}"),
                "project-performance",
                &workspace_root,
                pong_core::redaction::Redactor::default(),
            )
            .expect("workspace");
            let bytes = deterministic_bytes(seed ^ file_count as u64, file_size);
            for index in 0..file_count {
                fs::write(workspace_root.join(format!("file-{index:04}.bin")), &bytes)
                    .expect("snapshot scale file");
            }
            let cas = Cas::new(project.path().join("cas")).expect("snapshot scale CAS");
            let started = Instant::now();
            let snapshot = workspace
                .snapshot(&cas, SnapshotOptions::default())
                .expect("snapshot scale");
            let elapsed = started.elapsed().as_nanos();
            ScaleMeasurement {
                label,
                file_count,
                file_size,
                snapshot_elapsed_ns: elapsed,
                snapshot_digest: format!("sha256:{}", snapshot.digest),
                total_bytes: snapshot.total_bytes,
            }
        })
        .collect()
}

fn percentile(samples: &[u128], percentile: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let index = ((samples.len() - 1) * percentile) / 100;
    samples[index]
}

fn bytes_per_second(bytes: usize, elapsed: Duration) -> u64 {
    let nanos = elapsed.as_nanos();
    if nanos == 0 {
        return 0;
    }
    ((bytes as u128 * 1_000_000_000) / nanos).min(u64::MAX as u128) as u64
}

fn deterministic_bytes(seed: u64, length: usize) -> Vec<u8> {
    let mut state = seed;
    let mut bytes = Vec::with_capacity(length);
    for _ in 0..length {
        // A tiny fixed LCG is sufficient for workload bytes; this is not used
        // for identity, security, or production randomness.
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        bytes.push((state >> 32) as u8);
    }
    bytes
}

fn write_record(path: &Path, record: &PerformanceRecord) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(record)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    fs::write(path, bytes)
}

fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "unknown".into())
}
