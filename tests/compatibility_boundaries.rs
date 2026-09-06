use pong_core::metadata::{
    MetadataStore, NewEvent, OperationEnvelope, OperationRef, WorkspaceRecord, WorkspaceUpdate,
};
use pong_core::redaction::Redactor;
use pong_core::repository::{
    MigrationSpec, Repository, RepositoryMarker, GENERATION_REPOSITORY_FORMAT, REPOSITORY_FORMAT,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

// This fixture intentionally models the on-disk v0.1 *database* contract, not
// a v0.1 binary. Passing these tests proves only that the current M2/M3 tables
// are additive to the old SQLite tables. It must not be reported as evidence
// that an old reader understands, exposes, or safely mutates M2/M3 records;
// the selector/marker boundary remains the separate old-reader gate.
const V01_TABLES: &[&str] = &[
    "events",
    "idempotency",
    "operation_journal",
    "refs",
    "repository_meta",
];

const ADDITIVE_TABLES: &[&str] = &[
    "operations",
    "workspaces",
    "workspace_leases",
    "environments",
    "snapshots",
];

fn fixture(name: &str) -> Value {
    let path = format!("tests/fixtures/{name}");
    serde_json::from_str(&fs::read_to_string(path).expect("fixture file"))
        .expect("valid fixture JSON")
}

fn create_v01_database(path: &Path) {
    let connection = Connection::open(path).expect("v0.1 database");
    connection
        .execute_batch(include_str!("fixtures/CT-09-v01-metadata.sql"))
        .expect("v0.1 schema and rows");
}

fn table_names(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("table listing");
    statement
        .query_map([], |row| row.get(0))
        .expect("table rows")
        .collect::<Result<Vec<String>, _>>()
        .expect("table names")
}

fn table_columns(connection: &Connection, table: &str) -> Vec<String> {
    let sql = format!("PRAGMA table_info({table})");
    let mut statement = connection.prepare(&sql).expect("table info");
    statement
        .query_map([], |row| row.get(1))
        .expect("column rows")
        .collect::<Result<Vec<String>, _>>()
        .expect("column names")
}

fn index_columns(connection: &Connection, index: &str) -> Vec<String> {
    let sql = format!("PRAGMA index_info({index})");
    let mut statement = connection.prepare(&sql).expect("index info");
    statement
        .query_map([], |row| row.get(2))
        .expect("index columns")
        .collect::<Result<Vec<String>, _>>()
        .expect("index column names")
}

fn compatibility_operation() -> OperationEnvelope {
    OperationEnvelope {
        operation_id: "op-compatibility".into(),
        project_id: "legacy-project".into(),
        request_id: "req-compatibility".into(),
        agent_id: "agent-compatibility".into(),
        session_id: "session-compatibility".into(),
        workspace_id: None,
        environment_id: None,
        parent_operation_id: None,
        schema_version: "0.1".into(),
        started_at: "compatibility-time".into(),
        tool: "filesystem".into(),
        action: "inspect".into(),
        input_refs: vec![OperationRef {
            kind: "blob".into(),
            reference: "sha256:compat-input".into(),
            media_type: None,
        }],
        output_refs: Vec::new(),
        resource: Some(json!({"path":"README.md"})),
        before_state: None,
        after_state: None,
        reversibility: "REVERSIBLE".into(),
        replayability: "REPLAYABLE".into(),
        side_effect: "NONE".into(),
        policy_decision: Some(json!({"allowed":true})),
    }
}

fn legacy_ref(path: &Path) -> Option<String> {
    let connection = Connection::open(path).expect("legacy reader open");
    connection
        .query_row(
            "SELECT value FROM refs WHERE name = ?1",
            params!["refs/heads/main"],
            |row| row.get(0),
        )
        .optional()
        .expect("legacy ref query")
}

fn legacy_project_with_raw_v01_database() -> tempfile::TempDir {
    let project = tempdir().expect("project");
    let repository = Repository::init(project.path()).expect("repository layout");
    let metadata_path = repository.layout().metadata_path().to_path_buf();
    drop(repository);
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut candidate = metadata_path.as_os_str().to_os_string();
        candidate.push(suffix);
        let candidate = std::path::PathBuf::from(candidate);
        match fs::remove_file(candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("remove initialized metadata: {error}"),
        }
    }
    create_v01_database(&metadata_path);
    project
}

#[test]
fn current_marker_fixture_round_trips_with_additive_fields() {
    let fixture = fixture("CT-06-current-repository-marker.json");
    assert_eq!(fixture["repository_format"], REPOSITORY_FORMAT);
    assert_eq!(fixture["expected"]["status"], "supported");

    let project = tempdir().expect("project");
    let repository = Repository::init(project.path()).expect("init");
    let marker_path = repository.layout().marker_path().to_path_buf();
    drop(repository);

    let mut marker: Value =
        serde_json::from_slice(&fs::read(&marker_path).expect("marker")).expect("marker JSON");
    marker["future_optional_field"] = Value::String("preserved-by-reader-boundary".into());
    fs::write(
        &marker_path,
        serde_json::to_vec(&marker).expect("marker encoding"),
    )
    .expect("write additive marker");

    let opened = Repository::open(project.path()).expect("additive marker is compatible");
    assert_eq!(opened.marker(), &RepositoryMarker::default());
}

#[test]
fn unsupported_marker_version_fails_before_repository_is_healthy() {
    let project = tempdir().expect("project");
    let repository = Repository::init(project.path()).expect("init");
    let marker_path = repository.layout().marker_path().to_path_buf();
    drop(repository);

    let mut marker: Value =
        serde_json::from_slice(&fs::read(&marker_path).expect("marker")).expect("marker JSON");
    marker["marker_version"] = Value::Number(99.into());
    fs::write(
        &marker_path,
        serde_json::to_vec(&marker).expect("marker encoding"),
    )
    .expect("write unsupported marker");

    let error = match Repository::open(project.path()) {
        Ok(_) => panic!("unsupported marker must fail closed"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "UNSUPPORTED");
}

#[test]
fn redaction_profile_mismatch_requires_explicit_compatibility_choice() {
    let fixture = fixture("CT-08-redaction-profile.json");
    assert_eq!(fixture["expected"]["status"], "unsupported");

    let project = tempdir().expect("project");
    let redactor = Redactor::new("profile-a", "0.1").expect("profile");
    let repository = Repository::init_with_redactor(project.path(), redactor).expect("init");
    drop(repository);

    let error = match Repository::open(project.path()) {
        Ok(_) => panic!("profile mismatch must fail closed"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "UNSUPPORTED");
}

#[test]
fn previous_event_schema_is_read_without_rewriting_opaque_fields() {
    let fixture = fixture("CT-08-previous-event-schema.json");
    assert_eq!(fixture["expected"]["status"], "supported-as-opaque");

    let mut metadata = MetadataStore::in_memory().expect("metadata");
    let payload = fixture["input"]["payload"].clone();
    let record = metadata
        .append_event(NewEvent {
            event_id: fixture["input"]["event_id"].as_str().unwrap().into(),
            project_id: "project-compatibility".into(),
            stream_id: fixture["input"]["stream_id"].as_str().unwrap().into(),
            schema_version: fixture["input"]["schema_version"].as_str().unwrap().into(),
            actor_id: None,
            request_id: None,
            occurred_at: "2026-08-19T00:00:00Z".into(),
            payload,
        })
        .expect("previous schema event");
    assert_eq!(record.schema_version, "0.0");
    assert!(record.payload_json.contains("future_enum"));
    assert!(record.payload_json.contains("provider_specific"));

    let reopened = metadata
        .list_events("project:compatibility")
        .expect("event projection");
    assert_eq!(reopened, vec![record]);
}

#[test]
fn v01_database_cold_open_keeps_legacy_rows_and_adds_additive_tables() {
    let directory = tempdir().expect("project");
    let path = directory.path().join("metadata.sqlite");
    create_v01_database(&path);

    let before = Connection::open(&path).expect("inspect v0.1 database");
    let before_tables = table_names(&before);
    assert_eq!(
        before_tables,
        V01_TABLES
            .iter()
            .map(|table| (*table).to_owned())
            .collect::<Vec<String>>()
    );
    assert_eq!(
        table_columns(&before, "refs"),
        vec!["name", "value", "updated_at"]
    );
    assert_eq!(
        table_columns(&before, "events"),
        vec![
            "event_id",
            "project_id",
            "stream_id",
            "sequence",
            "schema_version",
            "actor_id",
            "request_id",
            "occurred_at",
            "payload_digest",
            "payload_json"
        ]
    );
    drop(before);

    {
        let mut metadata = MetadataStore::open(&path).expect("cold open v0.1 database");
        assert_eq!(metadata.repository_format().expect("format"), "0.1");
        assert_eq!(
            metadata.get_ref("refs/heads/main").expect("legacy ref"),
            Some("legacy-head".into())
        );
        assert_eq!(
            metadata
                .list_events("legacy-stream")
                .expect("legacy event")
                .len(),
            1
        );

        metadata
            .record_environment(
                "env-legacy-cold-open",
                "legacy-project",
                &json!({"os":"test","schema_version":"0.1"}),
                "m2-time",
            )
            .expect("M2 environment table");
        metadata
            .create_workspace(&WorkspaceRecord {
                workspace_id: "ws-legacy-cold-open".into(),
                project_id: "legacy-project".into(),
                driver: "local".into(),
                locator: "C:/workspace".into(),
                branch_ref: Some("refs/heads/main".into()),
                head: None,
                version_head_id: None,
                environment_id: None,
                status: "created".into(),
                revision: 0,
                created_at: "m2-time".into(),
                updated_at: "m2-time".into(),
            })
            .expect("M2 workspace table");
        let lease = metadata
            .acquire_workspace_lease("ws-legacy-cold-open", "agent", 10, 1_000)
            .expect("M2 lease table");
        metadata
            .update_workspace(WorkspaceUpdate {
                workspace_id: "ws-legacy-cold-open",
                expected_revision: 0,
                lease: &lease,
                branch_ref: Some("refs/heads/main"),
                head: None,
                environment_id: None,
                status: "preparing",
                updated_at: "m2-time-2",
                now_ms: 20,
            })
            .expect("M2 workspace update");
    }

    let after = Connection::open(&path).expect("reopen for schema inspection");
    let after_tables = table_names(&after);
    for table in V01_TABLES {
        assert!(
            after_tables.iter().any(|name| name == table),
            "legacy table disappeared: {table}"
        );
    }
    for table in ADDITIVE_TABLES {
        assert!(
            after_tables.iter().any(|name| name == table),
            "M2 table was not initialized: {table}"
        );
    }
    assert_eq!(
        table_columns(&after, "refs"),
        vec!["name", "value", "updated_at"]
    );
    assert_eq!(
        table_columns(&after, "events"),
        vec![
            "event_id",
            "project_id",
            "stream_id",
            "sequence",
            "schema_version",
            "actor_id",
            "request_id",
            "occurred_at",
            "payload_digest",
            "payload_json"
        ]
    );
    drop(after);

    {
        let metadata = MetadataStore::open(&path).expect("M2 cold reopen");
        assert_eq!(metadata.repository_format().expect("format"), "0.1");
        assert_eq!(
            metadata
                .workspace("ws-legacy-cold-open")
                .expect("workspace")
                .expect("workspace row")
                .revision,
            1
        );
        assert!(metadata
            .environment("env-legacy-cold-open")
            .expect("environment")
            .is_some());
    }

    // This direct SQL read is intentionally limited to the old v0.1 surface.
    // It demonstrates that an old reader's known ref projection still works;
    // it is not an old-reader binary/capability test and makes no M2 claim.
    assert_eq!(legacy_ref(&path), Some("legacy-head".into()));
}

#[test]
fn migration_backs_up_raw_v01_without_mutating_source_schema() {
    let project = legacy_project_with_raw_v01_database();
    let source_path = project.path().join(".pong/metadata.sqlite");
    let before = Connection::open(&source_path).expect("source inspect");
    assert_eq!(
        table_names(&before),
        V01_TABLES
            .iter()
            .map(|table| (*table).to_owned())
            .collect::<Vec<String>>()
    );
    drop(before);

    Repository::migrate(
        project.path(),
        MigrationSpec::new("compat-migration", "compat-generation"),
    )
    .expect("raw v0.1 migration");

    // The legacy source remains byte/schema-compatible and is retained for
    // explicit recovery tooling.  It is not the active authority after the
    // selector commit.
    let source = Connection::open(&source_path).expect("legacy source reopen");
    assert_eq!(
        table_names(&source),
        V01_TABLES
            .iter()
            .map(|table| (*table).to_owned())
            .collect::<Vec<String>>()
    );
    assert_eq!(legacy_ref(&source_path), Some("legacy-head".into()));
    drop(source);

    let mut repository = Repository::open(project.path()).expect("active generation reopen");
    assert_eq!(
        repository.marker().repository_format,
        GENERATION_REPOSITORY_FORMAT
    );
    assert_eq!(repository.active_generation_id(), Some("compat-generation"));
    let target_path = repository.active_metadata_path();
    let target = Connection::open(target_path).expect("target inspect");
    let target_tables = table_names(&target);
    for table in V01_TABLES.iter().chain(ADDITIVE_TABLES.iter()) {
        assert!(
            target_tables.iter().any(|name| name == table),
            "target table missing: {table}"
        );
    }
    assert_eq!(
        table_columns(&target, "workspaces"),
        vec![
            "workspace_id",
            "project_id",
            "driver",
            "locator",
            "branch_ref",
            "head",
            "environment_id",
            "status",
            "revision",
            "created_at",
            "updated_at",
            "version_head_id"
        ]
    );
    assert_eq!(
        table_columns(&target, "workspace_leases"),
        vec![
            "workspace_id",
            "agent_id",
            "epoch",
            "expires_at_ms",
            "acquired_at_ms"
        ]
    );
    assert_eq!(
        table_columns(&target, "environments"),
        vec![
            "environment_id",
            "project_id",
            "fingerprint",
            "facts_json",
            "created_at"
        ]
    );
    assert_eq!(
        table_columns(&target, "operations"),
        vec![
            "operation_id",
            "project_id",
            "request_id",
            "agent_id",
            "session_id",
            "workspace_id",
            "environment_id",
            "parent_operation_id",
            "schema_version",
            "started_at",
            "finished_at",
            "tool",
            "action",
            "input_refs_json",
            "output_refs_json",
            "resource_json",
            "before_state_json",
            "after_state_json",
            "result_json",
            "error_json",
            "reversibility",
            "replayability",
            "side_effect",
            "policy_json",
            "lifecycle_status",
            "recording_status",
            "redaction_profile_id",
            "redaction_profile_version",
            "envelope_digest",
            "updated_at"
        ]
    );
    assert_eq!(
        index_columns(&target, "operations_project_status"),
        vec!["project_id", "lifecycle_status", "updated_at"]
    );
    assert_eq!(
        repository
            .metadata()
            .get_ref("refs/heads/main")
            .expect("migrated ref"),
        Some("legacy-head".into())
    );
    assert_eq!(
        repository
            .metadata()
            .list_events("legacy-stream")
            .expect("migrated event")
            .len(),
        1
    );
    repository
        .metadata_mut()
        .start_operation(compatibility_operation())
        .expect("migrated operations table");
    drop(target);
    drop(repository);

    // A second cold reopen demonstrates that target M2 tables and legacy rows
    // survive restart; it does not widen the old-reader compatibility claim.
    let reopened = Repository::open(project.path()).expect("cold reopen target");
    assert_eq!(reopened.active_generation_id(), Some("compat-generation"));
    assert_eq!(
        reopened
            .metadata()
            .get_ref("refs/heads/main")
            .expect("reopened ref"),
        Some("legacy-head".into())
    );
    assert_eq!(
        reopened
            .metadata()
            .operation_record("op-compatibility")
            .expect("reopened operation")
            .expect("operation row")
            .lifecycle_status,
        "started"
    );
}

#[test]
fn unsupported_metadata_format_is_rejected_before_schema_mutation() {
    let directory = tempdir().expect("project");
    let path = directory.path().join("metadata.sqlite");
    let connection = Connection::open(&path).expect("unsupported database");
    connection
        .execute_batch(
            "CREATE TABLE repository_meta (
                 key TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
             INSERT INTO repository_meta(key, value)
                 VALUES ('repository_format', '9.9');",
        )
        .expect("unsupported schema");
    let before = table_names(&connection);
    drop(connection);

    let error = MetadataStore::open(&path).expect_err("unsupported format");
    assert_eq!(error.code(), "UNSUPPORTED");

    let reopened = Connection::open(&path).expect("inspect rejected database");
    assert_eq!(table_names(&reopened), before);
    assert!(!table_names(&reopened)
        .iter()
        .any(|name| ADDITIVE_TABLES.iter().any(|table| name == table)));
}

#[test]
fn legacy_metadata_without_profile_is_rejected_before_schema_mutation() {
    let directory = tempdir().expect("project");
    let path = directory.path().join("metadata.sqlite");
    let connection = Connection::open(&path).expect("legacy database");
    connection
        .execute_batch(
            "CREATE TABLE repository_meta (
                 key TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
             INSERT INTO repository_meta(key, value)
                 VALUES ('repository_format', '0.1');",
        )
        .expect("legacy metadata");
    let before = table_names(&connection);
    drop(connection);

    let error = MetadataStore::open(&path).expect_err("missing profile metadata");
    assert_eq!(error.code(), "UNSUPPORTED");
    let reopened = Connection::open(&path).expect("inspect rejected legacy database");
    assert_eq!(table_names(&reopened), before);
    assert!(!table_names(&reopened)
        .iter()
        .any(|name| ADDITIVE_TABLES.iter().any(|table| name == table)));
}

#[test]
fn incompatible_operations_object_fails_before_additive_schema_mutation() {
    let directory = tempdir().expect("project");
    let path = directory.path().join("metadata.sqlite");
    let connection = Connection::open(&path).expect("metadata");
    connection
        .execute_batch(
            "CREATE TABLE repository_meta (
                 key TEXT PRIMARY KEY NOT NULL,
                 value TEXT NOT NULL
             );
             INSERT INTO repository_meta(key, value) VALUES
                 ('repository_format', '0.1'),
                 ('redaction_profile_id', 'default'),
                 ('redaction_profile_version', '0.1');
             CREATE TABLE operations (wrong_column TEXT);",
        )
        .expect("malformed operations object");
    let before = table_names(&connection);
    drop(connection);

    let error = MetadataStore::open(&path).expect_err("malformed operations schema");
    assert_eq!(error.code(), "INTEGRITY_ERROR");
    let reopened = Connection::open(&path).expect("inspect malformed schema");
    assert_eq!(table_names(&reopened), before);
    assert_eq!(table_columns(&reopened, "operations"), vec!["wrong_column"]);
}
