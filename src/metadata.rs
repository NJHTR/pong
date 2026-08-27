use crate::canonical::canonical_bytes;
use crate::error::PongError;
use crate::redaction::{RedactionProfile, Redactor};
use rusqlite::types::Type;
use rusqlite::{
    params, Connection, DatabaseName, OpenFlags, OptionalExtension, Transaction,
    TransactionBehavior,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Arc;

const REPOSITORY_FORMAT: &str = "0.1";
const GENERATION_REPOSITORY_FORMAT: &str = "0.2";
const REDACTION_PROFILE_ID_KEY: &str = "redaction_profile_id";
const REDACTION_PROFILE_VERSION_KEY: &str = "redaction_profile_version";
const GENERATION_ID_KEY: &str = "generation_id";
const MIGRATION_ID_KEY: &str = "migration_id";
pub const OPERATION_SCHEMA_VERSION: &str = "0.1";
pub const EVENT_ENVELOPE_SCHEMA_VERSION: &str = "0.1";
pub const PROJECTION_SCHEMA_VERSION: &str = "0.1";

/// Deterministic failure boundaries used by the durable-primitive harness.
///
/// A `Before*` point is evaluated after all SQL statements for the operation
/// have run but before `COMMIT`; returning the injected error therefore lets
/// SQLite roll the transaction back.  An `After*` point is evaluated after
/// `COMMIT` returned successfully, so callers must treat the response as
/// unconfirmed and inspect the repository on restart.  The SQLite-wide points
/// apply to every mutating transaction and are useful for exercising the WAL
/// commit boundary independently of a particular journal operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataFailpoint {
    BeforeIntentCommit,
    AfterIntentCommit,
    BeforeOutcomeCommit,
    AfterOutcomeCommit,
    BeforeRecoveryCommit,
    AfterRecoveryCommit,
    BeforeSqliteCommit,
    AfterSqliteCommit,
}

/// Projection-specific interruption boundaries used by the FI-10 harness.
/// Boundaries before the SQLite commit roll back the complete projection
/// state/cursor/ledger transaction; the post-commit boundary reports an
/// unconfirmed result while leaving the committed state durable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionFailpoint {
    BeforeApply,
    AfterValidation,
    AfterIdempotencyCheck,
    AfterStateMutation,
    BeforeCursorUpdate,
    AfterCursorUpdate,
    BeforeLedgerCommit,
    AfterLedgerCommit,
    BeforeTransactionCommit,
    AfterTransactionCommit,
}

impl ProjectionFailpoint {
    fn label(self) -> &'static str {
        match self {
            Self::BeforeApply => "fi10_a_before_projection_apply",
            Self::AfterValidation => "fi10_b_after_event_validation",
            Self::AfterIdempotencyCheck => "fi10_c_after_idempotency_check",
            Self::AfterStateMutation => "fi10_d_after_state_mutation",
            Self::BeforeCursorUpdate => "fi10_e_before_cursor_update",
            Self::AfterCursorUpdate => "fi10_f_after_cursor_update",
            Self::BeforeLedgerCommit => "fi10_g_before_applied_event_ledger_commit",
            Self::AfterLedgerCommit => "fi10_h_after_applied_event_ledger_commit",
            Self::BeforeTransactionCommit => "fi10_i_before_transaction_commit",
            Self::AfterTransactionCommit => "fi10_j_after_transaction_commit",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProjectionFailpoints {
    armed: Option<ProjectionFailpoint>,
}

impl ProjectionFailpoints {
    pub const fn disabled() -> Self {
        Self { armed: None }
    }

    pub const fn once(point: ProjectionFailpoint) -> Self {
        Self { armed: Some(point) }
    }

    pub fn arm(&mut self, point: ProjectionFailpoint) {
        self.armed = Some(point);
    }

    pub fn disarm(&mut self) {
        self.armed = None;
    }

    pub const fn armed(&self) -> Option<ProjectionFailpoint> {
        self.armed
    }

    fn take_if(&mut self, point: ProjectionFailpoint) -> Option<ProjectionFailpoint> {
        if self.armed == Some(point) {
            let armed = self.armed;
            self.armed = None;
            armed
        } else {
            None
        }
    }
}

impl MetadataFailpoint {
    fn label(self) -> &'static str {
        match self {
            Self::BeforeIntentCommit => "before_intent_commit",
            Self::AfterIntentCommit => "after_intent_commit",
            Self::BeforeOutcomeCommit => "before_outcome_commit",
            Self::AfterOutcomeCommit => "after_outcome_commit",
            Self::BeforeRecoveryCommit => "before_recovery_commit",
            Self::AfterRecoveryCommit => "after_recovery_commit",
            Self::BeforeSqliteCommit => "before_sqlite_commit",
            Self::AfterSqliteCommit => "after_sqlite_commit",
        }
    }
}

/// One-shot failpoint configuration for a [`MetadataStore`].
///
/// The default configuration is disabled.  Arming a point consumes it on the
/// first matching boundary, which prevents a test from accidentally turning
/// every subsequent retry into the same synthetic failure.  This type carries
/// no process-global state and is therefore safe for parallel test processes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MetadataFailpoints {
    armed: Option<MetadataFailpoint>,
}

impl MetadataFailpoints {
    pub const fn disabled() -> Self {
        Self { armed: None }
    }

    pub const fn once(point: MetadataFailpoint) -> Self {
        Self { armed: Some(point) }
    }

    pub fn arm(&mut self, point: MetadataFailpoint) {
        self.armed = Some(point);
    }

    pub fn disarm(&mut self) {
        self.armed = None;
    }

    pub const fn armed(&self) -> Option<MetadataFailpoint> {
        self.armed
    }

    fn take_if(
        &mut self,
        specific: MetadataFailpoint,
        generic: MetadataFailpoint,
    ) -> Option<MetadataFailpoint> {
        if matches!(self.armed, Some(point) if point == specific || point == generic) {
            let point = self.armed;
            self.armed = None;
            point
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyKey {
    pub project_id: String,
    pub actor_id: String,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IdempotencyResult {
    NewlyRecorded,
    Existing(Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub event_id: String,
    pub project_id: String,
    pub stream_id: String,
    pub sequence: i64,
    pub schema_version: String,
    pub actor_id: Option<String>,
    pub request_id: Option<String>,
    pub occurred_at: String,
    pub payload_digest: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    pub event_id: String,
    pub project_id: String,
    pub stream_id: String,
    pub schema_version: String,
    pub actor_id: Option<String>,
    pub request_id: Option<String>,
    pub occurred_at: String,
    pub payload: Value,
}

/// Versioned event envelope used by projections and future consumers.  The
/// legacy [`NewEvent`] port remains available for v0.1 compatibility; it is
/// normalized into this envelope on append with conservative defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewEventEnvelope {
    pub event_id: String,
    pub project_id: String,
    pub stream_id: String,
    pub event_type: String,
    pub schema_version: String,
    pub occurred_at: String,
    pub recorded_at: String,
    pub actor_id: Option<String>,
    pub workspace_id: Option<String>,
    pub task_id: Option<String>,
    pub operation_id: Option<String>,
    pub causation_id: Option<String>,
    pub correlation_id: Option<String>,
    pub parent_event_ids: Vec<String>,
    pub capture_confidence: Option<String>,
    pub redaction_status: String,
    /// Optional caller assertion. When present it must match the metadata
    /// generation currently opened by this store.
    #[serde(default)]
    pub generation_id: Option<String>,
    #[serde(default)]
    pub migration_id: Option<String>,
    pub payload: Value,
}

/// Durable envelope returned after append.  `sequence` is stream-local;
/// `project_sequence` is the transactionally allocated cross-stream cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: String,
    pub project_id: String,
    pub stream_id: String,
    pub event_type: String,
    pub sequence: i64,
    pub project_sequence: i64,
    pub schema_version: String,
    pub occurred_at: String,
    pub recorded_at: String,
    pub actor_id: Option<String>,
    pub workspace_id: Option<String>,
    pub task_id: Option<String>,
    pub operation_id: Option<String>,
    pub causation_id: Option<String>,
    pub correlation_id: Option<String>,
    pub parent_event_ids: Vec<String>,
    pub capture_confidence: Option<String>,
    pub redaction_status: String,
    pub payload_digest: String,
    pub payload_json: String,
}

/// A registered projection handler. Handlers must be deterministic and
/// idempotent; duplicate delivery is filtered by `(event_id, payload_digest)`.
pub type ProjectionHandler =
    Arc<dyn Fn(&EventEnvelope, &mut Value) -> Result<(), PongError> + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionDefinition {
    pub projection_id: String,
    pub project_id: String,
    pub schema_version: String,
    /// Optional caller assertion for cross-generation isolation tests and
    /// migration tooling. The durable row is always bound to the opened store.
    pub generation_id: Option<String>,
    pub migration_id: Option<String>,
    pub initial_state: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionCursor {
    pub project_sequence: i64,
    pub stream_id: String,
    pub sequence: i64,
    pub event_id: String,
    pub payload_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionRecord {
    pub projection_id: String,
    pub project_id: String,
    pub schema_version: String,
    pub generation_id: String,
    pub migration_id: String,
    pub redaction_profile_id: String,
    pub redaction_profile_version: String,
    pub status: String,
    pub state_json: String,
    pub state_digest: String,
    pub cursor: Option<ProjectionCursor>,
    pub event_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionAppliedEvent {
    pub event_id: String,
    pub payload_digest: String,
    pub applied_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalOperation {
    pub project_id: String,
    pub actor_id: String,
    pub request_id: String,
    pub operation_id: String,
    pub phase: String,
    pub payload_json: String,
    pub created_at: String,
}

/// Durable logical workspace metadata. The physical materialization is owned
/// by a driver; `locator` is an opaque, redacted driver reference rather than
/// workspace identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRecord {
    pub workspace_id: String,
    pub project_id: String,
    pub driver: String,
    pub locator: String,
    pub branch_ref: Option<String>,
    pub head: Option<String>,
    pub environment_id: Option<String>,
    pub status: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Complete input for one lease-guarded workspace state transition.
#[derive(Debug, Clone, Copy)]
pub struct WorkspaceUpdate<'a> {
    pub workspace_id: &'a str,
    pub expected_revision: i64,
    pub lease: &'a LeaseToken,
    pub branch_ref: Option<&'a str>,
    pub head: Option<&'a str>,
    pub environment_id: Option<&'a str>,
    pub status: &'a str,
    pub updated_at: &'a str,
    pub now_ms: i64,
}

/// A write lease is identified by workspace, owner, and monotonically
/// increasing epoch. Callers must present the complete token for mutations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseRecord {
    pub workspace_id: String,
    pub agent_id: Option<String>,
    pub epoch: i64,
    pub expires_at_ms: i64,
    pub acquired_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseToken {
    pub workspace_id: String,
    pub agent_id: String,
    pub epoch: i64,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentRecord {
    pub environment_id: String,
    pub project_id: String,
    pub fingerprint: String,
    pub facts_json: String,
    pub created_at: String,
}

/// A typed, content-addressed input or output reference carried by an
/// operation envelope. The kind vocabulary is intentionally open so adapters
/// can add artifact, snapshot, resource, or provider-specific references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationRef {
    pub kind: String,
    pub reference: String,
    pub media_type: Option<String>,
}

/// Stable error object persisted with a failed, cancelled, or unknown
/// operation. Details are redacted before they reach SQLite or lifecycle
/// events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub details: Option<Value>,
    pub safe_to_expose: bool,
}

/// Complete immutable operation intent. `start_operation` persists this
/// envelope before returning an acknowledgement to the caller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationEnvelope {
    pub operation_id: String,
    pub project_id: String,
    pub request_id: String,
    pub agent_id: String,
    pub session_id: String,
    pub workspace_id: Option<String>,
    pub environment_id: Option<String>,
    pub parent_operation_id: Option<String>,
    pub schema_version: String,
    pub started_at: String,
    pub tool: String,
    pub action: String,
    pub input_refs: Vec<OperationRef>,
    pub output_refs: Vec<OperationRef>,
    pub resource: Option<Value>,
    pub before_state: Option<Value>,
    pub after_state: Option<Value>,
    pub reversibility: String,
    pub replayability: String,
    pub side_effect: String,
    pub policy_decision: Option<Value>,
}

/// Terminal lifecycle transition for an operation. `output_refs` and
/// `after_state` replace the provisional values from the start envelope when
/// present; a `None` value preserves the original intent fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationOutcome {
    pub status: String,
    pub finished_at: String,
    pub output_refs: Option<Vec<OperationRef>>,
    pub after_state: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<OperationError>,
}

/// Durable operation row, including the current lifecycle and recording
/// quality state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationRecord {
    pub operation_id: String,
    pub project_id: String,
    pub request_id: String,
    pub agent_id: String,
    pub session_id: String,
    pub workspace_id: Option<String>,
    pub environment_id: Option<String>,
    pub parent_operation_id: Option<String>,
    pub schema_version: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub tool: String,
    pub action: String,
    pub input_refs: Vec<OperationRef>,
    pub output_refs: Vec<OperationRef>,
    pub resource: Option<Value>,
    pub before_state: Option<Value>,
    pub after_state: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<OperationError>,
    pub reversibility: String,
    pub replayability: String,
    pub side_effect: String,
    pub policy_decision: Option<Value>,
    pub lifecycle_status: String,
    pub recording_status: String,
    pub redaction_profile_id: String,
    pub redaction_profile_version: String,
    pub envelope_digest: String,
    pub updated_at: String,
}

// Journal rows are intentionally kept as strings at the storage boundary so
// future protocol versions can preserve opaque phases. These sets define the
// phases understood by this core version and the transitions it can safely
// make without guessing about an external effect.
const KNOWN_OUTCOME_PHASES: &[&str] = &[
    "outcome_durable",
    "published",
    "failed",
    "cancelled",
    "unknown",
    "unreconciled",
];

fn phase_transition_allowed(current: &str, next: &str) -> bool {
    if current == next {
        return true;
    }
    match current {
        "reserved" | "intent_durable" => {
            matches!(next, "outcome_durable" | "failed" | "cancelled" | "unknown")
        }
        "executing" => {
            matches!(next, "outcome_durable" | "failed" | "cancelled" | "unknown")
        }
        "outcome_durable" => matches!(next, "published" | "unreconciled" | "unknown"),
        "unreconciled" => matches!(next, "published" | "unknown"),
        // Unknown is deliberately terminal. A later provider/operator
        // decision must append a reconciliation event rather than rewriting
        // the uncertainty record in place.
        "failed" | "cancelled" | "unknown" | "published" => false,
        _ => false,
    }
}

const OPERATION_SELECT: &str = "SELECT operation_id, project_id, request_id, agent_id, session_id,
            workspace_id, environment_id, parent_operation_id, schema_version,
            started_at, finished_at, tool, action, input_refs_json,
            output_refs_json, resource_json, before_state_json, after_state_json,
            result_json, error_json, reversibility, replayability, side_effect,
            policy_json, lifecycle_status, recording_status, redaction_profile_id,
            redaction_profile_version, envelope_digest, updated_at
     FROM operations ";

const OPERATIONS_COLUMNS: &[&str] = &[
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
    "updated_at",
];

const EVENT_ENVELOPE_COLUMNS: &[&str] = &[
    "event_id",
    "project_id",
    "stream_id",
    "sequence",
    "project_sequence",
    "event_type",
    "schema_version",
    "occurred_at",
    "recorded_at",
    "actor_id",
    "workspace_id",
    "task_id",
    "operation_id",
    "causation_id",
    "correlation_id",
    "parent_event_ids_json",
    "capture_confidence",
    "redaction_status",
    "payload_digest",
    "payload_json",
];
const PROJECTION_COLUMNS: &[&str] = &[
    "projection_id",
    "project_id",
    "schema_version",
    "generation_id",
    "migration_id",
    "redaction_profile_id",
    "redaction_profile_version",
    "status",
    "initial_state_json",
    "state_json",
    "state_digest",
    "cursor_project_sequence",
    "cursor_stream_id",
    "cursor_sequence",
    "cursor_event_id",
    "cursor_payload_digest",
    "event_count",
    "updated_at",
];
const PROJECTION_EVENT_COLUMNS: &[&str] =
    &["projection_id", "event_id", "payload_digest", "applied_at"];

fn inject_before_commit(
    failpoints: &mut MetadataFailpoints,
    specific: MetadataFailpoint,
) -> Result<(), PongError> {
    if let Some(point) = failpoints.take_if(specific, MetadataFailpoint::BeforeSqliteCommit) {
        return Err(PongError::FaultInjected(point.label().into()));
    }
    Ok(())
}

fn inject_after_commit(
    failpoints: &mut MetadataFailpoints,
    specific: MetadataFailpoint,
) -> Result<(), PongError> {
    if let Some(point) = failpoints.take_if(specific, MetadataFailpoint::AfterSqliteCommit) {
        return Err(PongError::FaultInjected(point.label().into()));
    }
    Ok(())
}

fn inject_projection(
    failpoints: &mut ProjectionFailpoints,
    point: ProjectionFailpoint,
) -> Result<(), PongError> {
    if let Some(fired) = failpoints.take_if(point) {
        return Err(PongError::FaultInjected(fired.label().into()));
    }
    Ok(())
}

/// Local-first transactional metadata and event store.
///
/// The connection is intentionally owned by one repository handle. Callers that
/// need concurrency must serialize commands at the repository boundary and use
/// ref/lease compare-and-swap rather than sharing mutable state implicitly.
pub struct MetadataStore {
    connection: Connection,
    redactor: Redactor,
    failpoints: MetadataFailpoints,
    projection_failpoints: ProjectionFailpoints,
    projection_handlers: HashMap<String, ProjectionHandler>,
}

impl fmt::Debug for MetadataStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MetadataStore")
            .field("redaction_profile", &self.redactor.profile())
            .field("failpoints", &self.failpoints)
            .field("projection_failpoints", &self.projection_failpoints)
            .field(
                "projection_handlers",
                &self.projection_handlers.keys().collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl MetadataStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PongError> {
        Self::open_with_redactor(path, Redactor::default())
    }

    /// Open a metadata database with an explicit persistence redaction policy.
    ///
    /// The policy identity is persisted in `repository_meta`; reopening an
    /// existing repository with a different profile fails closed rather than
    /// silently mixing incompatible records.
    pub fn open_with_redactor(
        path: impl AsRef<Path>,
        redactor: Redactor,
    ) -> Result<Self, PongError> {
        Self::open_with_redactor_and_failpoints(path, redactor, MetadataFailpoints::disabled())
    }

    /// Open a metadata database with an explicit, one-shot fault schedule.
    ///
    /// This is intended for deterministic crash/recovery tests.  Normal
    /// callers should use [`MetadataStore::open`] or
    /// [`MetadataStore::open_with_redactor`], both of which leave injection
    /// disabled.
    pub fn open_with_failpoints(
        path: impl AsRef<Path>,
        failpoints: MetadataFailpoints,
    ) -> Result<Self, PongError> {
        Self::open_with_redactor_and_failpoints(path, Redactor::default(), failpoints)
    }

    pub fn open_with_redactor_and_failpoints(
        path: impl AsRef<Path>,
        redactor: Redactor,
        failpoints: MetadataFailpoints,
    ) -> Result<Self, PongError> {
        let path = path.as_ref().to_path_buf();
        scan_database_bytes(&path, &redactor)?;
        let connection = Connection::open(&path)?;
        let store = Self::from_connection(connection, redactor.clone(), failpoints)?;
        // The pre-open scan catches legacy bytes. A second scan also covers
        // SQLite sidecars created while the schema/profile was initialized.
        scan_database_bytes(&path, &redactor)?;
        Ok(store)
    }

    /// Open an existing metadata file without running schema initialization.
    ///
    /// Repository-generation migration uses this read-only entrance for the
    /// legacy source.  Calling the ordinary open path there would create any
    /// newly introduced additive tables before the source digest and backup
    /// were captured, silently changing the legacy generation.  This method
    /// validates only identities already present in the database and never
    /// creates tables, metadata rows, or WAL checkpoints. SQLite may attach
    /// or create a read-only WAL shared-memory sidecar as part of opening the
    /// file; the migration scan covers such SQLite-owned bytes.
    pub(crate) fn open_for_backup(
        path: impl AsRef<Path>,
        redactor: Redactor,
    ) -> Result<Self, PongError> {
        let path = path.as_ref().to_path_buf();
        scan_database_bytes(&path, &redactor)?;
        let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let store = Self {
            connection,
            redactor,
            failpoints: MetadataFailpoints::disabled(),
            projection_failpoints: ProjectionFailpoints::disabled(),
            projection_handlers: HashMap::new(),
        };
        store.configure_read_only()?;
        store.validate_existing_schema()?;
        // Opening a read-only WAL database may create or attach a shared-memory
        // sidecar on some SQLite/platform combinations.  Scan again so the
        // migration's redaction boundary covers any SQLite-owned bytes that
        // became visible during open.
        scan_database_bytes(&path, &store.redactor)?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, PongError> {
        Self::in_memory_with_redactor(Redactor::default())
    }

    pub fn in_memory_with_redactor(redactor: Redactor) -> Result<Self, PongError> {
        Self::in_memory_with_redactor_and_failpoints(redactor, MetadataFailpoints::disabled())
    }

    pub fn in_memory_with_failpoints(failpoints: MetadataFailpoints) -> Result<Self, PongError> {
        Self::in_memory_with_redactor_and_failpoints(Redactor::default(), failpoints)
    }

    pub fn in_memory_with_redactor_and_failpoints(
        redactor: Redactor,
        failpoints: MetadataFailpoints,
    ) -> Result<Self, PongError> {
        let connection = Connection::open_in_memory()?;
        Self::from_connection(connection, redactor, failpoints)
    }

    fn from_connection(
        connection: Connection,
        redactor: Redactor,
        failpoints: MetadataFailpoints,
    ) -> Result<Self, PongError> {
        let mut store = Self {
            connection,
            redactor,
            failpoints,
            projection_failpoints: ProjectionFailpoints::disabled(),
            projection_handlers: HashMap::new(),
        };
        store.configure()?;
        store.initialize_schema()?;
        Ok(store)
    }

    /// Return the non-secret identity of the policy used at the persistence
    /// boundary. Registered secret values are intentionally not exposed.
    pub fn redaction_profile(&self) -> RedactionProfile {
        self.redactor.profile()
    }

    pub fn redactor(&self) -> &Redactor {
        &self.redactor
    }

    /// Replace the one-shot fault schedule.  Supplying the default value
    /// disables all injection without affecting any persisted state.
    pub fn set_failpoints(&mut self, failpoints: MetadataFailpoints) {
        self.failpoints = failpoints;
    }

    pub fn failpoints(&self) -> MetadataFailpoints {
        self.failpoints
    }

    pub fn set_projection_failpoints(&mut self, failpoints: ProjectionFailpoints) {
        self.projection_failpoints = failpoints;
    }

    pub fn projection_failpoints(&self) -> ProjectionFailpoints {
        self.projection_failpoints
    }

    fn configure(&mut self) -> Result<(), PongError> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA busy_timeout = 5000;",
        )?;
        Ok(())
    }

    fn configure_read_only(&self) -> Result<(), PongError> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA query_only = ON;
             PRAGMA busy_timeout = 5000;",
        )?;
        Ok(())
    }

    fn validate_existing_schema(&self) -> Result<(), PongError> {
        let has_repository_meta: bool = self.connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'repository_meta'
            )",
            [],
            |row| row.get::<_, i64>(0),
        )? == 1;
        if !has_repository_meta {
            return Err(PongError::Integrity(
                "metadata database lacks repository metadata".into(),
            ));
        }
        let format: String = self
            .connection
            .query_row(
                "SELECT value FROM repository_meta WHERE key = 'repository_format'",
                [],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                PongError::Integrity("metadata database lacks repository format".into())
            })?;
        if format != REPOSITORY_FORMAT && format != GENERATION_REPOSITORY_FORMAT {
            return Err(PongError::Unsupported(format!(
                "repository format {format} is not supported"
            )));
        }

        let profile = self.redactor.profile();
        let profile_id = self.redactor.redact_text(&profile.id);
        let profile_version = self.redactor.redact_text(&profile.version);
        let stored_profile_id: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM repository_meta WHERE key = ?1",
                [REDACTION_PROFILE_ID_KEY],
                |row| row.get(0),
            )
            .optional()?;
        let stored_profile_version: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM repository_meta WHERE key = ?1",
                [REDACTION_PROFILE_VERSION_KEY],
                |row| row.get(0),
            )
            .optional()?;
        if stored_profile_id.is_none() || stored_profile_version.is_none() {
            return Err(PongError::Unsupported(
                "repository lacks redaction profile metadata; explicit migration is required"
                    .into(),
            ));
        }
        if stored_profile_id.as_deref() != Some(&profile_id)
            || stored_profile_version.as_deref() != Some(&profile_version)
        {
            return Err(PongError::Unsupported(format!(
                "redaction profile mismatch: stored {:?}@{:?}, requested {profile_id}@{profile_version}",
                stored_profile_id, stored_profile_version
            )));
        }
        Ok(())
    }

    fn initialize_schema(&mut self) -> Result<(), PongError> {
        let has_repository_meta = self.connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'repository_meta'
            )",
            [],
            |row| row.get::<_, i64>(0),
        )? == 1;
        let existing_format: Option<String> = if has_repository_meta {
            self.connection
                .query_row(
                    "SELECT value FROM repository_meta WHERE key = 'repository_format'",
                    [],
                    |row| row.get(0),
                )
                .optional()?
        } else {
            None
        };
        if has_repository_meta {
            match existing_format.as_deref() {
                Some(REPOSITORY_FORMAT) | Some(GENERATION_REPOSITORY_FORMAT) => {}
                Some(format) => {
                    // Reject an unknown format before any CREATE TABLE or
                    // INSERT can mutate the database.  This is the startup
                    // compatibility boundary, not an in-place migration.
                    return Err(PongError::Unsupported(format!(
                        "repository format {format} is not supported"
                    )));
                }
                None => {
                    return Err(PongError::Unsupported(
                        "repository lacks repository format metadata; explicit migration is required"
                            .into(),
                    ));
                }
            }
        }
        let profile = self.redactor.profile();
        let profile_id = self.redactor.redact_text(&profile.id);
        let profile_version = self.redactor.redact_text(&profile.version);
        if has_repository_meta {
            let stored_profile_id: Option<String> = self
                .connection
                .query_row(
                    "SELECT value FROM repository_meta WHERE key = ?1",
                    [REDACTION_PROFILE_ID_KEY],
                    |row| row.get(0),
                )
                .optional()?;
            let stored_profile_version: Option<String> = self
                .connection
                .query_row(
                    "SELECT value FROM repository_meta WHERE key = ?1",
                    [REDACTION_PROFILE_VERSION_KEY],
                    |row| row.get(0),
                )
                .optional()?;
            if stored_profile_id.is_none() || stored_profile_version.is_none() {
                return Err(PongError::Unsupported(
                    "repository lacks redaction profile metadata; explicit migration is required"
                        .into(),
                ));
            }
            if stored_profile_id.as_deref() != Some(&profile_id)
                || stored_profile_version.as_deref() != Some(&profile_version)
            {
                return Err(PongError::Unsupported(format!(
                    "redaction profile mismatch: stored {:?}@{:?}, requested {profile_id}@{profile_version}",
                    stored_profile_id, stored_profile_version
                )));
            }
        }
        // A same-named view or incompatible table must never be silently
        // accepted by the additive CREATE TABLE IF NOT EXISTS below.
        validate_operations_schema(&self.connection, true)?;
        validate_additive_table_schema(
            &self.connection,
            "event_envelopes",
            EVENT_ENVELOPE_COLUMNS,
            true,
        )?;
        validate_additive_table_schema(&self.connection, "projections", PROJECTION_COLUMNS, true)?;
        validate_additive_table_schema(
            &self.connection,
            "projection_events",
            PROJECTION_EVENT_COLUMNS,
            true,
        )?;
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS repository_meta (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS refs (
                name TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS idempotency (
                project_id TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                request_id TEXT NOT NULL,
                command_digest TEXT NOT NULL,
                result_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (project_id, actor_id, request_id)
            );
             CREATE TABLE IF NOT EXISTS events (
                event_id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL,
                stream_id TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                schema_version TEXT NOT NULL,
                actor_id TEXT,
                request_id TEXT,
                occurred_at TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                UNIQUE (stream_id, sequence)
            );
             CREATE TABLE IF NOT EXISTS event_envelopes (
                event_id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL,
                stream_id TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                project_sequence INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                schema_version TEXT NOT NULL,
                occurred_at TEXT NOT NULL,
                recorded_at TEXT NOT NULL,
                actor_id TEXT,
                workspace_id TEXT,
                task_id TEXT,
                operation_id TEXT,
                causation_id TEXT,
                correlation_id TEXT,
                parent_event_ids_json TEXT NOT NULL,
                capture_confidence TEXT,
                redaction_status TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                UNIQUE (project_id, project_sequence)
             );
             CREATE INDEX IF NOT EXISTS event_envelopes_project_cursor
                 ON event_envelopes(project_id, project_sequence);
             CREATE TABLE IF NOT EXISTS projections (
                projection_id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL,
                schema_version TEXT NOT NULL,
                generation_id TEXT NOT NULL,
                migration_id TEXT NOT NULL,
                redaction_profile_id TEXT NOT NULL,
                redaction_profile_version TEXT NOT NULL,
                status TEXT NOT NULL,
                initial_state_json TEXT NOT NULL,
                state_json TEXT NOT NULL,
                state_digest TEXT NOT NULL,
                cursor_project_sequence INTEGER,
                cursor_stream_id TEXT,
                cursor_sequence INTEGER,
                cursor_event_id TEXT,
                cursor_payload_digest TEXT,
                event_count INTEGER NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS projection_events (
                projection_id TEXT NOT NULL,
                event_id TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                PRIMARY KEY (projection_id, event_id),
                FOREIGN KEY (projection_id) REFERENCES projections(projection_id)
             );
             CREATE TABLE IF NOT EXISTS operation_journal (
                 project_id TEXT NOT NULL,
                 actor_id TEXT NOT NULL,
                 request_id TEXT NOT NULL,
                 operation_id TEXT NOT NULL,
                phase TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 PRIMARY KEY (project_id, actor_id, request_id)
             );
             CREATE TABLE IF NOT EXISTS operations (
                 operation_id TEXT PRIMARY KEY NOT NULL,
                 project_id TEXT NOT NULL,
                 request_id TEXT NOT NULL,
                 agent_id TEXT NOT NULL,
                 session_id TEXT NOT NULL,
                 workspace_id TEXT,
                 environment_id TEXT,
                 parent_operation_id TEXT,
                 schema_version TEXT NOT NULL,
                 started_at TEXT NOT NULL,
                 finished_at TEXT,
                 tool TEXT NOT NULL,
                 action TEXT NOT NULL,
                 input_refs_json TEXT NOT NULL,
                 output_refs_json TEXT NOT NULL,
                 resource_json TEXT,
                 before_state_json TEXT,
                 after_state_json TEXT,
                 result_json TEXT,
                 error_json TEXT,
                 reversibility TEXT NOT NULL,
                 replayability TEXT NOT NULL,
                 side_effect TEXT NOT NULL,
                 policy_json TEXT,
                 lifecycle_status TEXT NOT NULL,
                 recording_status TEXT NOT NULL,
                 redaction_profile_id TEXT NOT NULL,
                 redaction_profile_version TEXT NOT NULL,
                 envelope_digest TEXT NOT NULL,
                 updated_at TEXT NOT NULL,
                 UNIQUE (project_id, agent_id, request_id)
             );
             CREATE INDEX IF NOT EXISTS operations_project_status
                 ON operations(project_id, lifecycle_status, updated_at);
             CREATE TABLE IF NOT EXISTS workspaces (
                workspace_id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL,
                driver TEXT NOT NULL,
                locator TEXT NOT NULL,
                branch_ref TEXT,
                head TEXT,
                environment_id TEXT,
                status TEXT NOT NULL,
                revision INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS workspace_leases (
                workspace_id TEXT PRIMARY KEY NOT NULL,
                agent_id TEXT,
                epoch INTEGER NOT NULL,
                expires_at_ms INTEGER NOT NULL,
                acquired_at_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS environments (
                environment_id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                facts_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            INSERT OR IGNORE INTO repository_meta(key, value)
                VALUES ('repository_format', '0.1');",
        )?;
        self.backfill_legacy_event_envelopes()?;
        validate_operations_schema(&self.connection, false)?;
        validate_additive_table_schema(
            &self.connection,
            "event_envelopes",
            EVENT_ENVELOPE_COLUMNS,
            false,
        )?;
        validate_additive_table_schema(&self.connection, "projections", PROJECTION_COLUMNS, false)?;
        validate_additive_table_schema(
            &self.connection,
            "projection_events",
            PROJECTION_EVENT_COLUMNS,
            false,
        )?;
        let format: String = self.connection.query_row(
            "SELECT value FROM repository_meta WHERE key = 'repository_format'",
            [],
            |row| row.get(0),
        )?;
        if format != REPOSITORY_FORMAT && format != GENERATION_REPOSITORY_FORMAT {
            return Err(PongError::Unsupported(format!(
                "repository format {format} is not supported"
            )));
        }
        // Persist only policy identity. Registered secret values remain in the
        // in-memory redactor and never become repository metadata.
        let stored_profile_id: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM repository_meta WHERE key = ?1",
                [REDACTION_PROFILE_ID_KEY],
                |row| row.get(0),
            )
            .optional()?;
        let stored_profile_version: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM repository_meta WHERE key = ?1",
                [REDACTION_PROFILE_VERSION_KEY],
                |row| row.get(0),
            )
            .optional()?;
        if has_repository_meta && (stored_profile_id.is_none() || stored_profile_version.is_none())
        {
            return Err(PongError::Unsupported(
                "repository lacks redaction profile metadata; explicit migration is required"
                    .into(),
            ));
        }
        if stored_profile_id.is_none() {
            self.connection.execute(
                "INSERT INTO repository_meta(key, value) VALUES (?1, ?2)",
                params![REDACTION_PROFILE_ID_KEY, profile_id],
            )?;
            self.connection.execute(
                "INSERT INTO repository_meta(key, value) VALUES (?1, ?2)",
                params![REDACTION_PROFILE_VERSION_KEY, profile_version],
            )?;
        } else if stored_profile_id.as_deref() != Some(&profile_id)
            || stored_profile_version.as_deref() != Some(&profile_version)
        {
            return Err(PongError::Unsupported(format!(
                "redaction profile mismatch: stored {:?}@{:?}, requested {profile_id}@{profile_version}",
                stored_profile_id, stored_profile_version
            )));
        }
        Ok(())
    }

    fn backfill_legacy_event_envelopes(&mut self) -> Result<(), PongError> {
        let mut statement = self.connection.prepare(
            "SELECT event_id, project_id, stream_id, sequence, schema_version,
                    actor_id, request_id, occurred_at, payload_digest, payload_json
             FROM events ORDER BY rowid ASC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(9)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (
            event_id,
            project_id,
            stream_id,
            sequence,
            schema_version,
            actor_id,
            request_id,
            occurred_at,
            payload_json,
        ) in rows
        {
            if transaction
                .query_row(
                    "SELECT 1 FROM event_envelopes WHERE event_id = ?1",
                    [&event_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some()
            {
                continue;
            }
            let payload: Value = serde_json::from_str(&payload_json)
                .map_err(|error| PongError::Serialization(error.to_string()))?;
            let project_sequence: i64 = transaction.query_row(
                "SELECT COALESCE(MAX(project_sequence), 0) + 1 FROM event_envelopes WHERE project_id = ?1",
                [&project_id],
                |row| row.get(0),
            )?;
            let event_type = payload
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("legacy.event");
            transaction.execute(
                "INSERT INTO event_envelopes
                 (event_id, project_id, stream_id, sequence, project_sequence, event_type,
                  schema_version, occurred_at, recorded_at, actor_id, workspace_id, task_id,
                  operation_id, causation_id, correlation_id, parent_event_ids_json,
                  capture_confidence, redaction_status, payload_digest, payload_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9, NULL, NULL,
                         NULL, NULL, ?10, '[]', 'observed', 'redacted',
                         (SELECT payload_digest FROM events WHERE event_id = ?1), ?11)",
                params![
                    event_id,
                    project_id,
                    stream_id,
                    sequence,
                    project_sequence,
                    event_type,
                    schema_version,
                    occurred_at,
                    actor_id,
                    request_id,
                    payload_json,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn repository_format(&self) -> Result<String, PongError> {
        self.connection
            .query_row(
                "SELECT value FROM repository_meta WHERE key = 'repository_format'",
                [],
                |row| row.get(0),
            )
            .map_err(PongError::from)
    }

    /// Change the storage format identity as part of an offline generation
    /// migration.  The event, ref, idempotency, and journal rows remain
    /// untouched; only the repository format declaration moves with the new
    /// SQLite file.
    pub(crate) fn set_repository_format(&mut self, format: &str) -> Result<(), PongError> {
        if format != REPOSITORY_FORMAT && format != GENERATION_REPOSITORY_FORMAT {
            return Err(PongError::Unsupported(format!(
                "repository format {format} is not supported"
            )));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "UPDATE repository_meta SET value = ?1 WHERE key = 'repository_format'",
            params![format],
        )?;
        if transaction.changes() != 1 {
            return Err(PongError::Integrity(
                "repository format metadata is missing".into(),
            ));
        }
        transaction.commit()?;
        Ok(())
    }

    /// Bind a copied metadata file to the generation and migration that will
    /// be named by its manifest. The identity lives inside SQLite as well as
    /// in JSON, so replacing the active file with another valid database is
    /// detected without comparing mutable live bytes to a historical digest.
    pub(crate) fn set_generation_identity(
        &mut self,
        generation_id: &str,
        migration_id: &str,
    ) -> Result<(), PongError> {
        if generation_id.is_empty() || migration_id.is_empty() {
            return Err(PongError::InvalidInput(
                "generation identity cannot be empty".into(),
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (key, value) in [
            (GENERATION_ID_KEY, generation_id),
            (MIGRATION_ID_KEY, migration_id),
        ] {
            transaction.execute(
                "INSERT INTO repository_meta(key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn generation_identity(&self) -> Result<Option<(String, String)>, PongError> {
        let generation_id: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM repository_meta WHERE key = ?1",
                [GENERATION_ID_KEY],
                |row| row.get(0),
            )
            .optional()?;
        let migration_id: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM repository_meta WHERE key = ?1",
                [MIGRATION_ID_KEY],
                |row| row.get(0),
            )
            .optional()?;
        match (generation_id, migration_id) {
            (Some(generation_id), Some(migration_id)) => Ok(Some((generation_id, migration_id))),
            (None, None) => Ok(None),
            _ => Err(PongError::Integrity(
                "generation identity metadata is incomplete".into(),
            )),
        }
    }

    pub(crate) fn rebind_projections(
        &mut self,
        generation_id: &str,
        migration_id: &str,
    ) -> Result<(), PongError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "UPDATE projections SET generation_id = ?1, migration_id = ?2",
            params![generation_id, migration_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn validate_projection_bindings(
        &self,
        generation_id: &str,
        migration_id: &str,
    ) -> Result<(), PongError> {
        let profile = self.redactor.profile();
        let invalid: Option<String> = self
            .connection
            .query_row(
                "SELECT projection_id FROM projections
                 WHERE generation_id != ?1 OR migration_id != ?2
                    OR redaction_profile_id != ?3 OR redaction_profile_version != ?4
                 LIMIT 1",
                params![generation_id, migration_id, profile.id, profile.version],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(projection_id) = invalid {
            return Err(PongError::Integrity(format!(
                "projection {projection_id} is not bound to the active generation"
            )));
        }
        let mut statement = self
            .connection
            .prepare("SELECT projection_id FROM projections ORDER BY projection_id")?;
        let projection_ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for projection_id in projection_ids {
            let record = self.projection_record(&projection_id)?.ok_or_else(|| {
                PongError::Integrity("projection disappeared during validation".into())
            })?;
            validate_projection_record_integrity(&self.connection, &record)?;
        }
        Ok(())
    }

    /// Verify SQLite's own page/index integrity and foreign-key invariants.
    /// This is intentionally a separate operation so migration can verify a
    /// closed, fully backed-up target before exposing it through the selector.
    pub(crate) fn verify_integrity(&self, expected_format: &str) -> Result<(), PongError> {
        let format = self.repository_format()?;
        if format != expected_format {
            return Err(PongError::Integrity(
                "target metadata format does not match its generation".into(),
            ));
        }
        self.integrity_check()?;
        self.foreign_key_check()?;
        Ok(())
    }

    pub(crate) fn checkpoint_truncate(&self) -> Result<(), PongError> {
        let (busy, _, _): (i64, i64, i64) =
            self.connection
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
        if busy != 0 {
            return Err(PongError::Conflict("SQLite WAL checkpoint is busy".into()));
        }
        Ok(())
    }

    pub(crate) fn backup_to(&self, path: &Path) -> Result<(), PongError> {
        self.connection
            .backup(DatabaseName::Main, path, None)
            .map_err(PongError::from)
    }

    pub(crate) fn integrity_check(&self) -> Result<(), PongError> {
        let result: String = self
            .connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if result != "ok" {
            return Err(PongError::Integrity(format!(
                "SQLite integrity_check failed: {result}"
            )));
        }
        Ok(())
    }

    pub(crate) fn foreign_key_check(&self) -> Result<(), PongError> {
        let mut statement = self.connection.prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        if rows.next()?.is_some() {
            return Err(PongError::Integrity(
                "SQLite foreign_key_check reported violations".into(),
            ));
        }
        Ok(())
    }

    /// Create a logical workspace at revision zero. The driver locator is
    /// metadata only; callers must not use it as the workspace identity.
    pub fn create_workspace(
        &mut self,
        record: &WorkspaceRecord,
    ) -> Result<WorkspaceRecord, PongError> {
        validate_workspace_record(record)?;
        let workspace_id = self.redactor.redact_text(&record.workspace_id);
        let project_id = self.redactor.redact_text(&record.project_id);
        let driver = self.redactor.redact_text(&record.driver);
        let locator = self.redactor.redact_text(&record.locator);
        let branch_ref = record
            .branch_ref
            .as_deref()
            .map(|value| self.redactor.redact_text(value));
        let head = record
            .head
            .as_deref()
            .map(|value| self.redactor.redact_text(value));
        let environment_id = record
            .environment_id
            .as_deref()
            .map(|value| self.redactor.redact_text(value));
        let status = self.redactor.redact_text(&record.status);
        let created_at = self.redactor.redact_text(&record.created_at);
        let updated_at = self.redactor.redact_text(&record.updated_at);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(environment_id) = &environment_id {
            let environment_project: Option<String> = transaction
                .query_row(
                    "SELECT project_id FROM environments WHERE environment_id = ?1",
                    [environment_id],
                    |row| row.get(0),
                )
                .optional()?;
            if environment_project.as_deref() != Some(project_id.as_str()) {
                return Err(PongError::Conflict(
                    "workspace environment is missing or belongs to another project".into(),
                ));
            }
        }
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO workspaces
             (workspace_id, project_id, driver, locator, branch_ref, head,
              environment_id, status, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10)",
            params![
                workspace_id,
                project_id,
                driver,
                locator,
                branch_ref,
                head,
                environment_id,
                status,
                created_at,
                updated_at,
            ],
        )?;
        if inserted != 1 {
            return Err(PongError::Conflict(
                "workspace identity already exists".into(),
            ));
        }
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        self.workspace(&record.workspace_id)?
            .ok_or_else(|| PongError::Integrity("workspace disappeared after creation".into()))
    }

    pub fn workspace(&self, workspace_id: &str) -> Result<Option<WorkspaceRecord>, PongError> {
        let workspace_id = self.redactor.redact_text(workspace_id);
        self.connection
            .query_row(
                "SELECT workspace_id, project_id, driver, locator, branch_ref, head,
                        environment_id, status, revision, created_at, updated_at
                 FROM workspaces WHERE workspace_id = ?1",
                [&workspace_id],
                workspace_from_row,
            )
            .optional()
            .map_err(PongError::from)
    }

    /// Read the current lease row for a workspace. A released lease remains
    /// as an epoch tombstone so a stale token can never become valid again.
    pub fn workspace_lease(&self, workspace_id: &str) -> Result<Option<LeaseRecord>, PongError> {
        let workspace_id = self.redactor.redact_text(workspace_id);
        self.connection
            .query_row(
                "SELECT workspace_id, agent_id, epoch, expires_at_ms, acquired_at_ms
                 FROM workspace_leases WHERE workspace_id = ?1",
                [&workspace_id],
                lease_from_row,
            )
            .optional()
            .map_err(PongError::from)
    }

    pub fn acquire_workspace_lease(
        &mut self,
        workspace_id: &str,
        agent_id: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<LeaseToken, PongError> {
        validate_non_empty(workspace_id, "workspace id")?;
        validate_non_empty(agent_id, "agent id")?;
        if ttl_ms <= 0 {
            return Err(PongError::InvalidInput("lease ttl must be positive".into()));
        }
        let workspace_id = self.redactor.redact_text(workspace_id);
        let agent_id = self.redactor.redact_text(agent_id);
        let expires_at_ms = now_ms
            .checked_add(ttl_ms)
            .ok_or_else(|| PongError::InvalidInput("lease expiry overflow".into()))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let workspace_exists: Option<String> = transaction
            .query_row(
                "SELECT workspace_id FROM workspaces WHERE workspace_id = ?1",
                [&workspace_id],
                |row| row.get(0),
            )
            .optional()?;
        if workspace_exists.is_none() {
            return Err(PongError::NotFound("workspace does not exist".into()));
        }
        let current: Option<LeaseRecord> = transaction
            .query_row(
                "SELECT workspace_id, agent_id, epoch, expires_at_ms, acquired_at_ms
                 FROM workspace_leases WHERE workspace_id = ?1",
                [&workspace_id],
                lease_from_row,
            )
            .optional()?;
        let epoch = match current {
            Some(lease) if lease.agent_id.is_some() && lease.expires_at_ms > now_ms => {
                return Err(PongError::Conflict("workspace lease is held".into()));
            }
            Some(lease) => lease
                .epoch
                .checked_add(1)
                .ok_or_else(|| PongError::Integrity("workspace lease epoch overflow".into()))?,
            None => 1,
        };
        transaction.execute(
            "INSERT INTO workspace_leases
             (workspace_id, agent_id, epoch, expires_at_ms, acquired_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workspace_id) DO UPDATE SET
               agent_id = excluded.agent_id,
               epoch = excluded.epoch,
               expires_at_ms = excluded.expires_at_ms,
               acquired_at_ms = excluded.acquired_at_ms",
            params![workspace_id, agent_id, epoch, expires_at_ms, now_ms],
        )?;
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        Ok(LeaseToken {
            workspace_id,
            agent_id,
            epoch,
            expires_at_ms,
        })
    }

    pub fn renew_workspace_lease(
        &mut self,
        token: &LeaseToken,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<LeaseToken, PongError> {
        if ttl_ms <= 0 {
            return Err(PongError::InvalidInput("lease ttl must be positive".into()));
        }
        let workspace_id = self.redactor.redact_text(&token.workspace_id);
        let agent_id = self.redactor.redact_text(&token.agent_id);
        let expires_at_ms = now_ms
            .checked_add(ttl_ms)
            .ok_or_else(|| PongError::InvalidInput("lease expiry overflow".into()))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE workspace_leases SET expires_at_ms = ?4, acquired_at_ms = ?5
             WHERE workspace_id = ?1 AND agent_id = ?2 AND epoch = ?3
               AND expires_at_ms > ?5",
            params![workspace_id, agent_id, token.epoch, expires_at_ms, now_ms],
        )?;
        if changed != 1 {
            return Err(PongError::Conflict(
                "workspace lease is stale or expired".into(),
            ));
        }
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        Ok(LeaseToken {
            workspace_id,
            agent_id,
            epoch: token.epoch,
            expires_at_ms,
        })
    }

    pub fn release_workspace_lease(
        &mut self,
        token: &LeaseToken,
        now_ms: i64,
    ) -> Result<(), PongError> {
        let workspace_id = self.redactor.redact_text(&token.workspace_id);
        let agent_id = self.redactor.redact_text(&token.agent_id);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE workspace_leases SET agent_id = NULL, expires_at_ms = 0,
                    acquired_at_ms = ?4
             WHERE workspace_id = ?1 AND agent_id = ?2 AND epoch = ?3",
            params![workspace_id, agent_id, token.epoch, now_ms],
        )?;
        if changed != 1 {
            return Err(PongError::Conflict("workspace lease is stale".into()));
        }
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        Ok(())
    }

    pub fn update_workspace(
        &mut self,
        update: WorkspaceUpdate<'_>,
    ) -> Result<WorkspaceRecord, PongError> {
        let WorkspaceUpdate {
            workspace_id,
            expected_revision,
            lease,
            branch_ref,
            head,
            environment_id,
            status,
            updated_at,
            now_ms,
        } = update;
        validate_non_empty(workspace_id, "workspace id")?;
        if expected_revision < 0 {
            return Err(PongError::InvalidInput(
                "workspace revision must not be negative".into(),
            ));
        }
        validate_workspace_status(status)?;
        validate_workspace_ready_state(status, head, environment_id)?;
        let workspace_id = self.redactor.redact_text(workspace_id);
        let branch_ref = branch_ref.map(|value| self.redactor.redact_text(value));
        let head = head.map(|value| self.redactor.redact_text(value));
        let environment_id = environment_id.map(|value| self.redactor.redact_text(value));
        let status = self.redactor.redact_text(status);
        let updated_at = self.redactor.redact_text(updated_at);
        let lease_workspace = self.redactor.redact_text(&lease.workspace_id);
        let lease_agent = self.redactor.redact_text(&lease.agent_id);
        if lease_workspace != workspace_id {
            return Err(PongError::Conflict(
                "lease belongs to another workspace".into(),
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(environment_id) = &environment_id {
            let environment_project: Option<String> = transaction
                .query_row(
                    "SELECT project_id FROM environments WHERE environment_id = ?1",
                    [environment_id],
                    |row| row.get(0),
                )
                .optional()?;
            let workspace_project: Option<String> = transaction
                .query_row(
                    "SELECT project_id FROM workspaces WHERE workspace_id = ?1",
                    [&workspace_id],
                    |row| row.get(0),
                )
                .optional()?;
            if environment_project.is_none() || environment_project != workspace_project {
                return Err(PongError::Conflict(
                    "workspace environment is missing or belongs to another project".into(),
                ));
            }
        }
        let lease_valid: Option<i64> = transaction
            .query_row(
                "SELECT epoch FROM workspace_leases
                 WHERE workspace_id = ?1 AND agent_id = ?2 AND epoch = ?3
                   AND expires_at_ms > ?4",
                params![lease_workspace, lease_agent, lease.epoch, now_ms],
                |row| row.get(0),
            )
            .optional()?;
        if lease_valid.is_none() {
            return Err(PongError::Conflict(
                "workspace lease is stale or expired".into(),
            ));
        }
        let changed = transaction.execute(
            "UPDATE workspaces SET branch_ref = ?2, head = ?3, environment_id = ?4,
                    status = ?5, revision = revision + 1, updated_at = ?6
             WHERE workspace_id = ?1 AND revision = ?7",
            params![
                workspace_id,
                branch_ref,
                head,
                environment_id,
                status,
                updated_at,
                expected_revision,
            ],
        )?;
        if changed != 1 {
            return Err(PongError::Conflict("workspace revision is stale".into()));
        }
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        self.workspace(&workspace_id)?
            .ok_or_else(|| PongError::Integrity("workspace disappeared after update".into()))
    }

    pub fn record_environment(
        &mut self,
        environment_id: &str,
        project_id: &str,
        facts: &Value,
        created_at: &str,
    ) -> Result<EnvironmentRecord, PongError> {
        validate_non_empty(environment_id, "environment id")?;
        validate_non_empty(project_id, "project id")?;
        let environment_id = self.redactor.redact_text(environment_id);
        let project_id = self.redactor.redact_text(project_id);
        let facts = self.redactor.redact_value(facts);
        let facts_json = String::from_utf8(canonical_bytes(&facts)?).map_err(|error| {
            PongError::Serialization(format!(
                "canonical environment facts are not UTF-8: {error}"
            ))
        })?;
        let fingerprint = format!(
            "sha256:{}",
            crate::cas::digest_hex("environment/v1", facts_json.as_bytes())
        );
        let created_at = self.redactor.redact_text(created_at);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(String, String, String)> = transaction
            .query_row(
                "SELECT project_id, fingerprint, facts_json FROM environments
                 WHERE environment_id = ?1",
                [&environment_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if let Some((existing_project, existing_fingerprint, existing_facts)) = existing {
            if existing_project != project_id
                || existing_fingerprint != fingerprint
                || existing_facts != facts_json
            {
                return Err(PongError::Integrity(
                    "environment identity was reused with different facts".into(),
                ));
            }
            transaction.commit()?;
            return self.environment(&environment_id)?.ok_or_else(|| {
                PongError::Integrity("environment disappeared during idempotent read".into())
            });
        }
        transaction.execute(
            "INSERT INTO environments
             (environment_id, project_id, fingerprint, facts_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                environment_id,
                project_id,
                fingerprint,
                facts_json,
                created_at
            ],
        )?;
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        self.environment(&environment_id)?
            .ok_or_else(|| PongError::Integrity("environment disappeared after creation".into()))
    }

    pub fn environment(
        &self,
        environment_id: &str,
    ) -> Result<Option<EnvironmentRecord>, PongError> {
        let environment_id = self.redactor.redact_text(environment_id);
        self.connection
            .query_row(
                "SELECT environment_id, project_id, fingerprint, facts_json, created_at
                 FROM environments WHERE environment_id = ?1",
                [&environment_id],
                environment_from_row,
            )
            .optional()
            .map_err(PongError::from)
    }

    pub fn get_ref(&self, name: &str) -> Result<Option<String>, PongError> {
        let name = self.redactor.redact_text(name);
        self.connection
            .query_row("SELECT value FROM refs WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .optional()
            .map_err(PongError::from)
    }

    pub fn compare_and_swap_ref(
        &mut self,
        name: &str,
        expected: Option<&str>,
        new_value: &str,
        updated_at: &str,
    ) -> Result<(), PongError> {
        let name = self.redactor.redact_text(name);
        let expected = expected.map(|value| self.redactor.redact_text(value));
        let new_value = self.redactor.redact_text(new_value);
        let updated_at = self.redactor.redact_text(updated_at);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: Option<String> = transaction
            .query_row("SELECT value FROM refs WHERE name = ?1", [&name], |row| {
                row.get(0)
            })
            .optional()?;
        if current.as_deref() != expected.as_deref() {
            return Err(PongError::Conflict(format!(
                "STALE_HEAD for ref {name}: expected {:?}, found {:?}",
                expected, current
            )));
        }
        match current {
            Some(_) => {
                transaction.execute(
                    "UPDATE refs SET value = ?2, updated_at = ?3 WHERE name = ?1",
                    params![name, new_value, updated_at],
                )?;
            }
            None => {
                transaction.execute(
                    "INSERT INTO refs(name, value, updated_at) VALUES (?1, ?2, ?3)",
                    params![name, new_value, updated_at],
                )?;
            }
        }
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        Ok(())
    }

    pub fn record_idempotency(
        &mut self,
        key: &IdempotencyKey,
        command_digest: &str,
        result: &Value,
        created_at: &str,
    ) -> Result<IdempotencyResult, PongError> {
        let project_id = self.redactor.redact_text(&key.project_id);
        let actor_id = self.redactor.redact_text(&key.actor_id);
        let request_id = self.redactor.redact_text(&key.request_id);
        let command_digest = self.redactor.redact_text(command_digest);
        let created_at = self.redactor.redact_text(created_at);
        let redacted_result = self.redactor.redact_value(result);
        let result_json =
            String::from_utf8(canonical_bytes(&redacted_result)?).map_err(|error| {
                PongError::Serialization(format!("canonical result is not UTF-8: {error}"))
            })?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO idempotency
             (project_id, actor_id, request_id, command_digest, result_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                project_id,
                actor_id,
                request_id,
                command_digest,
                result_json,
                created_at
            ],
        )?;
        let existing: (String, String) = transaction.query_row(
            "SELECT command_digest, result_json FROM idempotency
             WHERE project_id = ?1 AND actor_id = ?2 AND request_id = ?3",
            params![project_id, actor_id, request_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let result = if inserted == 1 {
            IdempotencyResult::NewlyRecorded
        } else {
            let (existing_digest, existing_json) = existing;
            if existing_digest != command_digest {
                return Err(PongError::IdempotencyKeyReuse(request_id));
            }
            let value = serde_json::from_str(&existing_json)
                .map_err(|error| PongError::Serialization(error.to_string()))?;
            IdempotencyResult::Existing(value)
        };
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        Ok(result)
    }

    /// Append a fully described event envelope. The legacy `events` table is
    /// populated as a compatibility projection; authoritative envelope data
    /// lives in `event_envelopes`.
    pub fn append_event_envelope(
        &mut self,
        event: NewEventEnvelope,
    ) -> Result<EventEnvelope, PongError> {
        let event = redact_event_envelope(&self.redactor, event);
        validate_event_envelope(&event)?;
        let expected_identity = self.projection_identity()?;
        validate_identity_assertion(
            event.generation_id.as_deref(),
            event.migration_id.as_deref(),
            &expected_identity,
            "event envelope",
        )?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let record = append_event_envelope_tx(&transaction, event)?;
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        Ok(record)
    }

    /// Append the v0.1 event shape. It is normalized into the envelope layer
    /// with `event_type` taken from payload `type` (or `legacy.event`) and
    /// `recorded_at` equal to `occurred_at`.
    pub fn append_event(&mut self, event: NewEvent) -> Result<EventRecord, PongError> {
        let event = NewEvent {
            event_id: self.redactor.redact_text(&event.event_id),
            project_id: self.redactor.redact_text(&event.project_id),
            stream_id: self.redactor.redact_text(&event.stream_id),
            schema_version: self.redactor.redact_text(&event.schema_version),
            actor_id: event
                .actor_id
                .as_deref()
                .map(|value| self.redactor.redact_text(value)),
            request_id: event
                .request_id
                .as_deref()
                .map(|value| self.redactor.redact_text(value)),
            occurred_at: self.redactor.redact_text(&event.occurred_at),
            payload: self.redactor.redact_value(&event.payload),
        };
        let payload_json =
            String::from_utf8(canonical_bytes(&event.payload)?).map_err(|error| {
                PongError::Serialization(format!("canonical payload is not UTF-8: {error}"))
            })?;
        let payload_digest = hex::encode(Sha256::digest(payload_json.as_bytes()));
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = transaction
            .query_row(
                "SELECT event_id, project_id, stream_id, sequence, schema_version,
                        actor_id, request_id, occurred_at, payload_digest, payload_json
                 FROM events WHERE event_id = ?1",
                [&event.event_id],
                event_from_row,
            )
            .optional()?
        {
            if existing.payload_digest != payload_digest
                || existing.project_id != event.project_id
                || existing.stream_id != event.stream_id
                || existing.schema_version != event.schema_version
                || existing.actor_id != event.actor_id
                || existing.request_id != event.request_id
                || existing.occurred_at != event.occurred_at
            {
                return Err(PongError::Integrity(format!(
                    "event ID {} was reused with different content",
                    event.event_id
                )));
            }
            inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
            transaction.commit()?;
            inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
            return Ok(existing);
        }
        let next_sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM events WHERE stream_id = ?1",
            [&event.stream_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO events
             (event_id, project_id, stream_id, sequence, schema_version, actor_id,
              request_id, occurred_at, payload_digest, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                event.event_id,
                event.project_id,
                event.stream_id,
                next_sequence,
                event.schema_version,
                event.actor_id,
                event.request_id,
                event.occurred_at,
                payload_digest,
                payload_json
            ],
        )?;
        append_event_envelope_tx(
            &transaction,
            NewEventEnvelope {
                event_id: event.event_id.clone(),
                project_id: event.project_id.clone(),
                stream_id: event.stream_id.clone(),
                event_type: event
                    .payload
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("legacy.event")
                    .to_owned(),
                schema_version: event.schema_version.clone(),
                occurred_at: event.occurred_at.clone(),
                recorded_at: event.occurred_at.clone(),
                actor_id: event.actor_id.clone(),
                workspace_id: None,
                task_id: None,
                operation_id: None,
                causation_id: None,
                correlation_id: event.request_id.clone(),
                parent_event_ids: Vec::new(),
                capture_confidence: Some("observed".into()),
                redaction_status: "redacted".into(),
                generation_id: None,
                migration_id: None,
                payload: event.payload.clone(),
            },
        )?;
        let record = EventRecord {
            event_id: event.event_id,
            project_id: event.project_id,
            stream_id: event.stream_id,
            sequence: next_sequence,
            schema_version: event.schema_version,
            actor_id: event.actor_id,
            request_id: event.request_id,
            occurred_at: event.occurred_at,
            payload_digest,
            payload_json,
        };
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        Ok(record)
    }

    pub fn list_events(&self, stream_id: &str) -> Result<Vec<EventRecord>, PongError> {
        let stream_id = self.redactor.redact_text(stream_id);
        let mut statement = self.connection.prepare(
            "SELECT event_id, project_id, stream_id, sequence, schema_version,
                    actor_id, request_id, occurred_at, payload_digest, payload_json
             FROM events WHERE stream_id = ?1 ORDER BY sequence ASC",
        )?;
        let rows = statement.query_map([stream_id], event_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(PongError::from)
    }

    /// Register or replace the in-process handler for a projection event
    /// type. Handler code is deliberately not persisted; the projection row
    /// records only schema and state identity.
    pub fn register_projection_handler<F>(&mut self, event_type: impl Into<String>, handler: F)
    where
        F: Fn(&EventEnvelope, &mut Value) -> Result<(), PongError> + Send + Sync + 'static,
    {
        self.projection_handlers
            .insert(event_type.into(), Arc::new(handler));
    }

    pub fn create_projection(
        &mut self,
        definition: ProjectionDefinition,
        updated_at: impl Into<String>,
    ) -> Result<ProjectionRecord, PongError> {
        validate_projection_definition(&definition)?;
        let initial_state_json = canonical_json_value(&definition.initial_state)?;
        let state_digest = digest_text(&initial_state_json);
        let (generation_id, migration_id) = self.projection_identity()?;
        validate_identity_assertion(
            definition.generation_id.as_deref(),
            definition.migration_id.as_deref(),
            &(generation_id.clone(), migration_id.clone()),
            "projection definition",
        )?;
        let profile = self.redactor.profile();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO projections
             (projection_id, project_id, schema_version, generation_id, migration_id,
              redaction_profile_id, redaction_profile_version, status,
              initial_state_json, state_json, state_digest, event_count, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ready', ?8, ?8, ?9, 0, ?10)",
            params![
                definition.projection_id,
                definition.project_id,
                definition.schema_version,
                generation_id,
                migration_id,
                profile.id,
                profile.version,
                initial_state_json,
                state_digest,
                updated_at.into(),
            ],
        )?;
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        self.projection_record(&definition.projection_id)?
            .ok_or_else(|| PongError::Integrity("projection disappeared after creation".into()))
    }

    pub fn projection_record(
        &self,
        projection_id: &str,
    ) -> Result<Option<ProjectionRecord>, PongError> {
        self.connection
            .query_row(
                "SELECT projection_id, project_id, schema_version, generation_id,
                        migration_id, redaction_profile_id, redaction_profile_version,
                        status, state_json, state_digest, cursor_project_sequence,
                        cursor_stream_id, cursor_sequence, cursor_event_id,
                        cursor_payload_digest, event_count
                 FROM projections WHERE projection_id = ?1",
                [projection_id],
                projection_from_row,
            )
            .optional()
            .map_err(PongError::from)
    }

    pub fn projection_applied_events(
        &self,
        projection_id: &str,
    ) -> Result<Vec<ProjectionAppliedEvent>, PongError> {
        let mut statement = self.connection.prepare(
            "SELECT event_id, payload_digest, applied_at FROM projection_events
             WHERE projection_id = ?1 ORDER BY rowid ASC",
        )?;
        let rows = statement.query_map([projection_id], |row| {
            Ok(ProjectionAppliedEvent {
                event_id: row.get(0)?,
                payload_digest: row.get(1)?,
                applied_at: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(PongError::from)
    }

    /// Apply all source envelopes after the durable cursor in one metadata
    /// transaction. Unknown event types are preserved and mark the projection
    /// `degraded`; they are never silently reported as `ready`.
    pub fn apply_projection(
        &mut self,
        projection_id: &str,
        updated_at: impl Into<String>,
    ) -> Result<ProjectionRecord, PongError> {
        self.apply_projection_internal(projection_id, updated_at.into(), false)
    }

    /// Rebuild from the immutable event source. Reset, replay, cursor and
    /// digest publication are atomic, so a failed rebuild leaves the prior
    /// ready state intact and retrying converges deterministically.
    pub fn rebuild_projection(
        &mut self,
        projection_id: &str,
        updated_at: impl Into<String>,
    ) -> Result<ProjectionRecord, PongError> {
        self.apply_projection_internal(projection_id, updated_at.into(), true)
    }

    fn apply_projection_internal(
        &mut self,
        projection_id: &str,
        updated_at: String,
        rebuild: bool,
    ) -> Result<ProjectionRecord, PongError> {
        inject_projection(
            &mut self.projection_failpoints,
            ProjectionFailpoint::BeforeApply,
        )?;
        let existing = self
            .projection_record(projection_id)?
            .ok_or_else(|| PongError::NotFound(format!("projection {projection_id}")))?;
        let (generation_id, migration_id) = self.projection_identity()?;
        if existing.generation_id != generation_id || existing.migration_id != migration_id {
            return Err(PongError::Integrity(
                "projection generation identity does not match metadata".into(),
            ));
        }
        let profile = self.redactor.profile();
        if existing.redaction_profile_id != profile.id
            || existing.redaction_profile_version != profile.version
        {
            return Err(PongError::Integrity(
                "projection redaction profile does not match metadata".into(),
            ));
        }
        validate_projection_record_integrity(&self.connection, &existing)?;
        let initial_state = if rebuild {
            self.connection.query_row(
                "SELECT initial_state_json FROM projections WHERE projection_id = ?1",
                [projection_id],
                |row| row.get::<_, String>(0),
            )?
        } else {
            existing.state_json.clone()
        };
        let mut state: Value = serde_json::from_str(&initial_state)
            .map_err(|error| PongError::Serialization(error.to_string()))?;
        let start_sequence = if rebuild {
            0
        } else {
            existing
                .cursor
                .as_ref()
                .map_or(0, |cursor| cursor.project_sequence)
        };
        let envelopes = self.list_event_envelopes(&existing.project_id, start_sequence)?;
        inject_projection(
            &mut self.projection_failpoints,
            ProjectionFailpoint::AfterValidation,
        )?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if rebuild {
            transaction.execute(
                "DELETE FROM projection_events WHERE projection_id = ?1",
                [projection_id],
            )?;
        }
        let mut cursor = if rebuild {
            None
        } else {
            existing.cursor.clone()
        };
        let mut count = if rebuild { 0 } else { existing.event_count };
        let mut degraded = if rebuild {
            false
        } else {
            existing.status == "degraded"
        };
        for event in envelopes {
            let applied_digest = transaction
                .query_row(
                    "SELECT payload_digest FROM projection_events
                     WHERE projection_id = ?1 AND event_id = ?2",
                    params![projection_id, event.event_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            inject_projection(
                &mut self.projection_failpoints,
                ProjectionFailpoint::AfterIdempotencyCheck,
            )?;
            if let Some(applied_digest) = applied_digest {
                if applied_digest != event.payload_digest {
                    return Err(PongError::Integrity(format!(
                        "projection event {} was reused with a different payload digest",
                        event.event_id
                    )));
                }
                cursor = Some(event_cursor(&event));
                continue;
            }
            if let Some(handler) = self.projection_handlers.get(&event.event_type).cloned() {
                handler(&event, &mut state)?;
            } else {
                degraded = true;
            }
            inject_projection(
                &mut self.projection_failpoints,
                ProjectionFailpoint::AfterStateMutation,
            )?;
            inject_projection(
                &mut self.projection_failpoints,
                ProjectionFailpoint::BeforeCursorUpdate,
            )?;
            cursor = Some(event_cursor(&event));
            inject_projection(
                &mut self.projection_failpoints,
                ProjectionFailpoint::AfterCursorUpdate,
            )?;
            inject_projection(
                &mut self.projection_failpoints,
                ProjectionFailpoint::BeforeLedgerCommit,
            )?;
            transaction.execute(
                "INSERT INTO projection_events(projection_id, event_id, payload_digest, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    projection_id,
                    event.event_id,
                    event.payload_digest,
                    updated_at
                ],
            )?;
            inject_projection(
                &mut self.projection_failpoints,
                ProjectionFailpoint::AfterLedgerCommit,
            )?;
            count += 1;
        }
        let state_json = canonical_json_value(&state)?;
        let state_digest = digest_text(&state_json);
        let status = if degraded { "degraded" } else { "ready" };
        transaction.execute(
            "UPDATE projections SET status = ?1, state_json = ?2, state_digest = ?3,
                cursor_project_sequence = ?4, cursor_stream_id = ?5,
                cursor_sequence = ?6, cursor_event_id = ?7,
                cursor_payload_digest = ?8, event_count = ?9, updated_at = ?10
             WHERE projection_id = ?11",
            params![
                status,
                state_json,
                state_digest,
                cursor.as_ref().map(|c| c.project_sequence),
                cursor.as_ref().map(|c| c.stream_id.as_str()),
                cursor.as_ref().map(|c| c.sequence),
                cursor.as_ref().map(|c| c.event_id.as_str()),
                cursor.as_ref().map(|c| c.payload_digest.as_str()),
                count,
                updated_at,
                projection_id,
            ],
        )?;
        inject_projection(
            &mut self.projection_failpoints,
            ProjectionFailpoint::BeforeTransactionCommit,
        )?;
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_projection(
            &mut self.projection_failpoints,
            ProjectionFailpoint::AfterTransactionCommit,
        )?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        self.projection_record(projection_id)?
            .ok_or_else(|| PongError::Integrity("projection disappeared after apply".into()))
    }

    fn projection_identity(&self) -> Result<(String, String), PongError> {
        Ok(self
            .generation_identity()?
            .unwrap_or_else(|| ("legacy-v0.1".into(), "legacy".into())))
    }

    pub fn list_event_envelopes(
        &self,
        project_id: &str,
        after_project_sequence: i64,
    ) -> Result<Vec<EventEnvelope>, PongError> {
        let mut statement = self.connection.prepare(
            "SELECT event_id, project_id, stream_id, event_type, sequence,
                    project_sequence, schema_version, occurred_at, recorded_at,
                    actor_id, workspace_id, task_id, operation_id, causation_id,
                    correlation_id, parent_event_ids_json, capture_confidence,
                    redaction_status, payload_digest, payload_json
             FROM event_envelopes WHERE project_id = ?1 AND project_sequence > ?2
             ORDER BY project_sequence ASC",
        )?;
        let rows = statement.query_map(
            params![project_id, after_project_sequence],
            envelope_from_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(PongError::from)
    }

    /// Persist an operation intent and its `started` lifecycle event in one
    /// SQLite transaction. A retry with the same project/agent/request and
    /// canonical envelope returns the existing row; changing any envelope
    /// field under that identity is rejected as idempotency-key reuse.
    pub fn start_operation(
        &mut self,
        envelope: OperationEnvelope,
    ) -> Result<OperationRecord, PongError> {
        let envelope = redact_operation_envelope(&self.redactor, envelope);
        validate_operation_envelope(&envelope)?;
        let envelope_digest = operation_envelope_digest(&envelope)?;
        let input_refs_json = canonical_json(&envelope.input_refs)?;
        let output_refs_json = canonical_json(&envelope.output_refs)?;
        let resource_json = optional_canonical_json(envelope.resource.as_ref())?;
        let before_state_json = optional_canonical_json(envelope.before_state.as_ref())?;
        let after_state_json = optional_canonical_json(envelope.after_state.as_ref())?;
        let policy_json = optional_canonical_json(envelope.policy_decision.as_ref())?;
        let envelope_json = canonical_json(&envelope)?;
        let profile = self.redactor.profile();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        let by_request: Option<OperationRecord> = transaction
            .query_row(
                &format!(
                    "{OPERATION_SELECT} WHERE project_id = ?1 AND agent_id = ?2 AND request_id = ?3"
                ),
                params![envelope.project_id, envelope.agent_id, envelope.request_id],
                operation_from_row,
            )
            .optional()?;
        if let Some(existing) = by_request {
            if existing.envelope_digest != envelope_digest {
                return Err(PongError::IdempotencyKeyReuse(envelope.request_id));
            }
            transaction.commit()?;
            return Ok(existing);
        }

        let by_id: Option<OperationRecord> = transaction
            .query_row(
                &format!("{OPERATION_SELECT} WHERE operation_id = ?1"),
                [&envelope.operation_id],
                operation_from_row,
            )
            .optional()?;
        if by_id.is_some() {
            return Err(PongError::Conflict(
                "operation identity is already used by another request".into(),
            ));
        }
        validate_operation_bindings(
            &transaction,
            &envelope.project_id,
            envelope.workspace_id.as_deref(),
            envelope.environment_id.as_deref(),
            envelope.parent_operation_id.as_deref(),
        )?;
        let journal_inserted = transaction.execute(
            "INSERT INTO operation_journal
             (project_id, actor_id, request_id, operation_id, phase, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, 'intent_durable', ?5, ?6)
             ON CONFLICT(project_id, actor_id, request_id) DO NOTHING",
            params![
                envelope.project_id,
                envelope.agent_id,
                envelope.request_id,
                envelope.operation_id,
                envelope_json,
                envelope.started_at,
            ],
        )?;
        if journal_inserted == 0 {
            let existing: (String, String) = transaction.query_row(
                "SELECT operation_id, payload_json FROM operation_journal
                 WHERE project_id = ?1 AND actor_id = ?2 AND request_id = ?3",
                params![envelope.project_id, envelope.agent_id, envelope.request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if existing.0 != envelope.operation_id || existing.1 != envelope_json {
                return Err(PongError::IdempotencyKeyReuse(envelope.request_id));
            }
        }
        transaction.execute(
            "INSERT INTO operations
             (operation_id, project_id, request_id, agent_id, session_id,
              workspace_id, environment_id, parent_operation_id, schema_version,
              started_at, finished_at, tool, action, input_refs_json,
              output_refs_json, resource_json, before_state_json, after_state_json,
              result_json, error_json, reversibility, replayability, side_effect,
              policy_json, lifecycle_status, recording_status, redaction_profile_id,
              redaction_profile_version, envelope_digest, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL,
                     ?11, ?12, ?13, ?14, ?15, ?16, ?17, NULL, NULL,
                     ?18, ?19, ?20, ?21, 'started', 'durable', ?22, ?23, ?24, ?25)",
            params![
                envelope.operation_id,
                envelope.project_id,
                envelope.request_id,
                envelope.agent_id,
                envelope.session_id,
                envelope.workspace_id,
                envelope.environment_id,
                envelope.parent_operation_id,
                envelope.schema_version,
                envelope.started_at,
                envelope.tool,
                envelope.action,
                input_refs_json,
                output_refs_json,
                resource_json,
                before_state_json,
                after_state_json,
                envelope.reversibility,
                envelope.replayability,
                envelope.side_effect,
                policy_json,
                profile.id,
                profile.version,
                envelope_digest,
                envelope.started_at,
            ],
        )?;
        let payload = json!({
            "operation_id": envelope.operation_id,
            "lifecycle_status": "started",
            "recording_status": "durable",
            "envelope_digest": envelope_digest,
        });
        let sequence = next_operation_event_sequence(&transaction, &envelope.operation_id)?;
        append_operation_event(
            &transaction,
            &envelope.operation_id,
            &envelope.project_id,
            &envelope.agent_id,
            &envelope.request_id,
            &envelope.schema_version,
            &envelope.started_at,
            sequence,
            &payload,
        )?;
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        self.operation_record(&envelope.operation_id)?
            .ok_or_else(|| PongError::Integrity("operation disappeared after start".into()))
    }

    /// Persist one terminal operation outcome and its immutable lifecycle
    /// event. Retries of an identical outcome are no-ops; a different outcome
    /// for one operation identity is rejected.
    pub fn finish_operation(
        &mut self,
        operation_id: &str,
        outcome: OperationOutcome,
    ) -> Result<OperationRecord, PongError> {
        let operation_id = self.redactor.redact_text(operation_id);
        validate_non_empty(&operation_id, "operation id")?;
        let outcome = redact_operation_outcome(&self.redactor, outcome);
        validate_operation_outcome(&outcome)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: OperationRecord = transaction
            .query_row(
                &format!("{OPERATION_SELECT} WHERE operation_id = ?1"),
                [&operation_id],
                operation_from_row,
            )
            .optional()?
            .ok_or_else(|| PongError::NotFound(format!("operation {operation_id}")))?;
        let output_refs = outcome
            .output_refs
            .clone()
            .unwrap_or_else(|| current.output_refs.clone());
        let after_state = outcome
            .after_state
            .clone()
            .or_else(|| current.after_state.clone());
        let result_json = optional_canonical_json(outcome.result.as_ref())?;
        let error_json = match outcome.error.as_ref() {
            Some(error) => Some(canonical_json_value(
                &serde_json::to_value(error).map_err(|serialization| {
                    PongError::Serialization(format!(
                        "cannot encode operation error: {serialization}"
                    ))
                })?,
            )?),
            None => None,
        };
        let output_refs_json = canonical_json(&output_refs)?;
        let after_state_json = optional_canonical_json(after_state.as_ref())?;
        if current.lifecycle_status == outcome.status {
            if current.finished_at.as_deref() == Some(outcome.finished_at.as_str())
                && current.output_refs == output_refs
                && current.after_state == after_state
                && current.result.as_ref() == outcome.result.as_ref()
                && current.error.as_ref() == outcome.error.as_ref()
            {
                transaction.commit()?;
                return Ok(current);
            }
            return Err(PongError::Integrity(
                "operation outcome was reused with different content".into(),
            ));
        }
        if current.lifecycle_status != "started" {
            return Err(PongError::Conflict(format!(
                "invalid operation lifecycle transition {} -> {}",
                current.lifecycle_status, outcome.status
            )));
        }
        let outcome_value = serde_json::to_value(&outcome).map_err(|error| {
            PongError::Serialization(format!("cannot encode operation outcome: {error}"))
        })?;
        let outcome_digest = operation_value_digest(&outcome_value)?;
        let journal_phase = match outcome.status.as_str() {
            "completed" => "outcome_durable",
            "failed" => "failed",
            "cancelled" => "cancelled",
            "unknown" => "unknown",
            _ => unreachable!("validated operation outcome status"),
        };
        let journal_payload = json!({
            "operation_id": operation_id,
            "lifecycle_status": outcome.status,
            "outcome_digest": outcome_digest,
            "outcome": outcome,
        });
        let journal_payload_json = canonical_json_value(&journal_payload)?;
        transaction.execute(
            "UPDATE operations SET output_refs_json = ?2, after_state_json = ?3,
                    finished_at = ?4, result_json = ?5, error_json = ?6,
                    lifecycle_status = ?7, updated_at = ?8
             WHERE operation_id = ?1 AND lifecycle_status = 'started'",
            params![
                operation_id,
                output_refs_json,
                after_state_json,
                outcome.finished_at,
                result_json,
                error_json,
                outcome.status,
                outcome.finished_at,
            ],
        )?;
        let journal_changed = transaction.execute(
            "UPDATE operation_journal SET phase = ?4, payload_json = ?5
             WHERE project_id = ?1 AND actor_id = ?2 AND request_id = ?3",
            params![
                current.project_id,
                current.agent_id,
                current.request_id,
                journal_phase,
                journal_payload_json,
            ],
        )?;
        if journal_changed != 1 {
            return Err(PongError::Integrity(
                "operation journal row disappeared during finish".into(),
            ));
        }
        let payload = json!({
            "operation_id": operation_id,
            "lifecycle_status": outcome.status,
            "recording_status": current.recording_status,
            "outcome_digest": outcome_digest,
        });
        let sequence = next_operation_event_sequence(&transaction, &operation_id)?;
        append_operation_event(
            &transaction,
            &operation_id,
            &current.project_id,
            &current.agent_id,
            &current.request_id,
            &current.schema_version,
            &outcome.finished_at,
            sequence,
            &payload,
        )?;
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        self.operation_record(&operation_id)?
            .ok_or_else(|| PongError::Integrity("operation disappeared after finish".into()))
    }

    /// Change the recording-quality state without rewriting lifecycle or
    /// outcome data. The transition is itself represented by an immutable
    /// operation event.
    pub fn set_operation_recording_status(
        &mut self,
        operation_id: &str,
        recording_status: &str,
        updated_at: &str,
    ) -> Result<OperationRecord, PongError> {
        let operation_id = self.redactor.redact_text(operation_id);
        let recording_status = self.redactor.redact_text(recording_status);
        let updated_at = self.redactor.redact_text(updated_at);
        validate_non_empty(&operation_id, "operation id")?;
        validate_non_empty(&updated_at, "operation updated_at")?;
        if !matches!(recording_status.as_str(), "durable" | "unreconciled") {
            return Err(PongError::InvalidInput(
                "operation recording status is unsupported".into(),
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: OperationRecord = transaction
            .query_row(
                &format!("{OPERATION_SELECT} WHERE operation_id = ?1"),
                [&operation_id],
                operation_from_row,
            )
            .optional()?
            .ok_or_else(|| PongError::NotFound(format!("operation {operation_id}")))?;
        if current.recording_status == recording_status {
            transaction.commit()?;
            return Ok(current);
        }
        transaction.execute(
            "UPDATE operations SET recording_status = ?2, updated_at = ?3
             WHERE operation_id = ?1",
            params![operation_id, recording_status, updated_at],
        )?;
        let payload = json!({
            "operation_id": operation_id,
            "lifecycle_status": current.lifecycle_status,
            "recording_status": recording_status,
        });
        let sequence = next_operation_event_sequence(&transaction, &operation_id)?;
        append_operation_event(
            &transaction,
            &operation_id,
            &current.project_id,
            &current.agent_id,
            &current.request_id,
            &current.schema_version,
            &updated_at,
            sequence,
            &payload,
        )?;
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeSqliteCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterSqliteCommit)?;
        self.operation_record(&operation_id)?.ok_or_else(|| {
            PongError::Integrity("operation disappeared after recording update".into())
        })
    }

    pub fn operation_record(
        &self,
        operation_id: &str,
    ) -> Result<Option<OperationRecord>, PongError> {
        let operation_id = self.redactor.redact_text(operation_id);
        self.connection
            .query_row(
                &format!("{OPERATION_SELECT} WHERE operation_id = ?1"),
                [&operation_id],
                operation_from_row,
            )
            .optional()
            .map_err(PongError::from)
    }

    pub fn operation_record_for_request(
        &self,
        project_id: &str,
        agent_id: &str,
        request_id: &str,
    ) -> Result<Option<OperationRecord>, PongError> {
        let project_id = self.redactor.redact_text(project_id);
        let agent_id = self.redactor.redact_text(agent_id);
        let request_id = self.redactor.redact_text(request_id);
        self.connection
            .query_row(
                &format!(
                    "{OPERATION_SELECT} WHERE project_id = ?1 AND agent_id = ?2 AND request_id = ?3"
                ),
                params![project_id, agent_id, request_id],
                operation_from_row,
            )
            .optional()
            .map_err(PongError::from)
    }

    pub fn record_intent(
        &mut self,
        project_id: &str,
        actor_id: &str,
        request_id: &str,
        operation_id: &str,
        payload: &Value,
        created_at: &str,
    ) -> Result<(), PongError> {
        let project_id = self.redactor.redact_text(project_id);
        let actor_id = self.redactor.redact_text(actor_id);
        let request_id = self.redactor.redact_text(request_id);
        let operation_id = self.redactor.redact_text(operation_id);
        let created_at = self.redactor.redact_text(created_at);
        let redacted_payload = self.redactor.redact_value(payload);
        let payload_json =
            String::from_utf8(canonical_bytes(&redacted_payload)?).map_err(|error| {
                PongError::Serialization(format!("canonical intent is not UTF-8: {error}"))
            })?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = transaction.execute(
            "INSERT INTO operation_journal
             (project_id, actor_id, request_id, operation_id, phase, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, 'intent_durable', ?5, ?6)
             ON CONFLICT(project_id, actor_id, request_id) DO NOTHING",
            params![
                project_id,
                actor_id,
                request_id,
                operation_id,
                payload_json,
                created_at
            ],
        )?;
        if inserted == 0 {
            let existing: (String, String, String) = transaction.query_row(
                "SELECT operation_id, phase, payload_json FROM operation_journal
                 WHERE project_id = ?1 AND actor_id = ?2 AND request_id = ?3",
                params![project_id, actor_id, request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            if existing.0 != operation_id || existing.2 != payload_json {
                return Err(PongError::IdempotencyKeyReuse(request_id));
            }
        }
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeIntentCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterIntentCommit)?;
        Ok(())
    }

    pub fn record_outcome(
        &mut self,
        key: &IdempotencyKey,
        phase: &str,
        payload: &Value,
    ) -> Result<(), PongError> {
        let project_id = self.redactor.redact_text(&key.project_id);
        let actor_id = self.redactor.redact_text(&key.actor_id);
        let request_id = self.redactor.redact_text(&key.request_id);
        let phase = self.redactor.redact_text(phase);
        let redacted_payload = self.redactor.redact_value(payload);
        let payload_json =
            String::from_utf8(canonical_bytes(&redacted_payload)?).map_err(|error| {
                PongError::Serialization(format!("canonical outcome is not UTF-8: {error}"))
            })?;
        if !KNOWN_OUTCOME_PHASES.contains(&phase.as_str()) {
            return Err(PongError::InvalidInput(format!(
                "unsupported journal outcome phase {phase}"
            )));
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<(String, String)> = transaction
            .query_row(
                "SELECT phase, payload_json FROM operation_journal
                 WHERE project_id = ?1 AND actor_id = ?2 AND request_id = ?3",
                params![project_id, actor_id, request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((current_phase, current_payload)) = existing else {
            return Err(PongError::NotFound(format!(
                "operation intent {}",
                request_id
            )));
        };

        // Retries of the same phase and payload are safe no-ops. A changed
        // payload under one request identity is an integrity failure rather
        // than a second outcome.
        if current_phase == phase {
            if current_payload != payload_json {
                return Err(PongError::Integrity(format!(
                    "operation {} reused phase {phase} with different payload",
                    request_id
                )));
            }
            inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeOutcomeCommit)?;
            transaction.commit()?;
            inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterOutcomeCommit)?;
            return Ok(());
        }
        if !phase_transition_allowed(&current_phase, &phase) {
            return Err(PongError::Conflict(format!(
                "invalid operation phase transition {current_phase} -> {phase} for {}",
                request_id
            )));
        }

        transaction.execute(
            "UPDATE operation_journal SET phase = ?4, payload_json = ?5
             WHERE project_id = ?1 AND actor_id = ?2 AND request_id = ?3",
            params![project_id, actor_id, request_id, phase, payload_json],
        )?;
        inject_before_commit(&mut self.failpoints, MetadataFailpoint::BeforeOutcomeCommit)?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterOutcomeCommit)?;
        Ok(())
    }

    pub fn unfinished_operations(&self) -> Result<Vec<JournalOperation>, PongError> {
        let mut statement = self.connection.prepare(
            "SELECT project_id, actor_id, request_id, operation_id, phase, payload_json, created_at
             FROM operation_journal
             WHERE phase NOT IN ('failed', 'cancelled', 'unknown', 'published')
             ORDER BY created_at, request_id",
        )?;
        let rows = statement.query_map([], journal_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(PongError::from)
    }

    /// Mark intent records that have no durable outcome as `unknown`.
    ///
    /// This is an explicit, repeatable recovery action: all rows are updated
    /// in one SQLite transaction, and a second invocation returns an empty set.
    /// Outcome-durable rows are intentionally left untouched so a caller can
    /// republish their event/ref transaction instead of losing that evidence.
    pub fn recover_unfinished_operations(&mut self) -> Result<Vec<JournalOperation>, PongError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut statement = transaction.prepare(
            "SELECT project_id, actor_id, request_id, operation_id, phase, payload_json, created_at
             FROM operation_journal
             WHERE phase IN ('reserved', 'intent_durable', 'executing')
             ORDER BY created_at, request_id",
        )?;
        let rows = statement.query_map([], journal_from_row)?;
        let mut recovered = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for operation in &mut recovered {
            transaction.execute(
                "UPDATE operation_journal SET phase = 'unknown'
                 WHERE project_id = ?1 AND actor_id = ?2 AND request_id = ?3",
                params![
                    operation.project_id,
                    operation.actor_id,
                    operation.request_id
                ],
            )?;
            operation.phase = "unknown".into();
        }
        inject_before_commit(
            &mut self.failpoints,
            MetadataFailpoint::BeforeRecoveryCommit,
        )?;
        transaction.commit()?;
        inject_after_commit(&mut self.failpoints, MetadataFailpoint::AfterRecoveryCommit)?;
        Ok(recovered)
    }

    /// Return operations whose effects remain explicitly unknown after
    /// recovery. The original intent payload is retained for reconciliation.
    pub fn unknown_operations(&self) -> Result<Vec<JournalOperation>, PongError> {
        let mut statement = self.connection.prepare(
            "SELECT project_id, actor_id, request_id, operation_id, phase, payload_json, created_at
             FROM operation_journal WHERE phase = 'unknown'
             ORDER BY created_at, request_id",
        )?;
        let rows = statement.query_map([], journal_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(PongError::from)
    }

    /// Return an operation by its request identity, including terminal states.
    pub fn operation(&self, key: &IdempotencyKey) -> Result<Option<JournalOperation>, PongError> {
        let project_id = self.redactor.redact_text(&key.project_id);
        let actor_id = self.redactor.redact_text(&key.actor_id);
        let request_id = self.redactor.redact_text(&key.request_id);
        self.connection
            .query_row(
                "SELECT project_id, actor_id, request_id, operation_id, phase, payload_json, created_at
                 FROM operation_journal
                 WHERE project_id = ?1 AND actor_id = ?2 AND request_id = ?3",
                params![project_id, actor_id, request_id],
                journal_from_row,
            )
            .optional()
            .map_err(PongError::from)
    }
}

fn scan_database_bytes(path: &Path, redactor: &Redactor) -> Result<(), PongError> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut candidate = path.as_os_str().to_os_string();
        candidate.push(suffix);
        let candidate = std::path::PathBuf::from(candidate);
        match fs::read(&candidate) {
            Ok(bytes) => redactor.assert_clean_bytes(&bytes, "SQLite-owned bytes")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(PongError::from(error)),
        }
    }
    Ok(())
}

fn journal_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JournalOperation> {
    Ok(JournalOperation {
        project_id: row.get(0)?,
        actor_id: row.get(1)?,
        request_id: row.get(2)?,
        operation_id: row.get(3)?,
        phase: row.get(4)?,
        payload_json: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn canonical_json<T: Serialize>(value: &T) -> Result<String, PongError> {
    let value = serde_json::to_value(value).map_err(|error| {
        PongError::Serialization(format!("cannot encode operation JSON: {error}"))
    })?;
    canonical_json_value(&value)
}

fn canonical_json_value(value: &Value) -> Result<String, PongError> {
    String::from_utf8(crate::canonical::canonical_bytes(value)?).map_err(|error| {
        PongError::Serialization(format!("canonical operation JSON is not UTF-8: {error}"))
    })
}

fn optional_canonical_json(value: Option<&Value>) -> Result<Option<String>, PongError> {
    value.map(canonical_json_value).transpose()
}

/// Validate the additive operations table at the schema boundary. The
/// read-only migration entrance deliberately does not call this function:
/// legacy source files are allowed to predate the table entirely.
fn validate_operations_schema(
    connection: &Connection,
    allow_missing: bool,
) -> Result<(), PongError> {
    let object_type: Option<String> = connection
        .query_row(
            "SELECT type FROM sqlite_master WHERE name = 'operations'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(object_type) = object_type else {
        return if allow_missing {
            Ok(())
        } else {
            Err(PongError::Integrity(
                "operations table is missing after schema initialization".into(),
            ))
        };
    };
    if object_type != "table" {
        return Err(PongError::Integrity(
            "operations schema object is not a table".into(),
        ));
    }
    let mut statement = connection.prepare("PRAGMA table_info(operations)")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if columns
        .iter()
        .map(String::as_str)
        .ne(OPERATIONS_COLUMNS.iter().copied())
    {
        return Err(PongError::Integrity(
            "operations table columns are incompatible".into(),
        ));
    }
    Ok(())
}

fn validate_additive_table_schema(
    connection: &Connection,
    table: &str,
    expected: &[&str],
    allow_missing: bool,
) -> Result<(), PongError> {
    let object_type: Option<String> = connection
        .query_row(
            "SELECT type FROM sqlite_master WHERE name = ?1",
            [table],
            |row| row.get(0),
        )
        .optional()?;
    let Some(object_type) = object_type else {
        return if allow_missing {
            Ok(())
        } else {
            Err(PongError::Integrity(format!(
                "{table} table is missing after schema initialization"
            )))
        };
    };
    if object_type != "table" {
        return Err(PongError::Integrity(format!(
            "{table} schema object is not a table"
        )));
    }
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if columns
        .iter()
        .map(String::as_str)
        .ne(expected.iter().copied())
    {
        return Err(PongError::Integrity(format!(
            "{table} table columns are incompatible"
        )));
    }
    Ok(())
}

fn operation_value_digest(value: &Value) -> Result<String, PongError> {
    Ok(format!(
        "sha256:{}",
        crate::canonical::canonical_digest("operation/value/v1", value)?
    ))
}

fn operation_envelope_digest(envelope: &OperationEnvelope) -> Result<String, PongError> {
    let value = serde_json::to_value(envelope).map_err(|error| {
        PongError::Serialization(format!("cannot encode operation envelope: {error}"))
    })?;
    Ok(format!(
        "sha256:{}",
        crate::canonical::canonical_digest("operation/envelope/v1", &value)?
    ))
}

fn redact_operation_ref(redactor: &Redactor, reference: OperationRef) -> OperationRef {
    OperationRef {
        kind: redactor.redact_text(&reference.kind),
        reference: redactor.redact_text(&reference.reference),
        media_type: reference
            .media_type
            .as_deref()
            .map(|value| redactor.redact_text(value)),
    }
}

fn redact_operation_envelope(
    redactor: &Redactor,
    envelope: OperationEnvelope,
) -> OperationEnvelope {
    OperationEnvelope {
        operation_id: redactor.redact_text(&envelope.operation_id),
        project_id: redactor.redact_text(&envelope.project_id),
        request_id: redactor.redact_text(&envelope.request_id),
        agent_id: redactor.redact_text(&envelope.agent_id),
        session_id: redactor.redact_text(&envelope.session_id),
        workspace_id: envelope
            .workspace_id
            .as_deref()
            .map(|value| redactor.redact_text(value)),
        environment_id: envelope
            .environment_id
            .as_deref()
            .map(|value| redactor.redact_text(value)),
        parent_operation_id: envelope
            .parent_operation_id
            .as_deref()
            .map(|value| redactor.redact_text(value)),
        schema_version: redactor.redact_text(&envelope.schema_version),
        started_at: redactor.redact_text(&envelope.started_at),
        tool: redactor.redact_text(&envelope.tool),
        action: redactor.redact_text(&envelope.action),
        input_refs: envelope
            .input_refs
            .into_iter()
            .map(|reference| redact_operation_ref(redactor, reference))
            .collect(),
        output_refs: envelope
            .output_refs
            .into_iter()
            .map(|reference| redact_operation_ref(redactor, reference))
            .collect(),
        resource: envelope
            .resource
            .as_ref()
            .map(|value| redactor.redact_value(value)),
        before_state: envelope
            .before_state
            .as_ref()
            .map(|value| redactor.redact_value(value)),
        after_state: envelope
            .after_state
            .as_ref()
            .map(|value| redactor.redact_value(value)),
        reversibility: redactor.redact_text(&envelope.reversibility),
        replayability: redactor.redact_text(&envelope.replayability),
        side_effect: redactor.redact_text(&envelope.side_effect),
        policy_decision: envelope
            .policy_decision
            .as_ref()
            .map(|value| redactor.redact_value(value)),
    }
}

fn redact_operation_outcome(redactor: &Redactor, outcome: OperationOutcome) -> OperationOutcome {
    OperationOutcome {
        status: redactor.redact_text(&outcome.status),
        finished_at: redactor.redact_text(&outcome.finished_at),
        output_refs: outcome.output_refs.map(|references| {
            references
                .into_iter()
                .map(|reference| redact_operation_ref(redactor, reference))
                .collect()
        }),
        after_state: outcome
            .after_state
            .as_ref()
            .map(|value| redactor.redact_value(value)),
        result: outcome
            .result
            .as_ref()
            .map(|value| redactor.redact_value(value)),
        error: outcome.error.map(|error| OperationError {
            code: redactor.redact_text(&error.code),
            message: redactor.redact_text(&error.message),
            retryable: error.retryable,
            details: error
                .details
                .as_ref()
                .map(|value| redactor.redact_value(value)),
            safe_to_expose: error.safe_to_expose,
        }),
    }
}

fn validate_operation_ref(reference: &OperationRef) -> Result<(), PongError> {
    validate_non_empty(&reference.kind, "operation reference kind")?;
    validate_non_empty(&reference.reference, "operation reference")?;
    if let Some(media_type) = reference.media_type.as_deref() {
        validate_non_empty(media_type, "operation reference media type")?;
    }
    Ok(())
}

fn validate_operation_envelope(envelope: &OperationEnvelope) -> Result<(), PongError> {
    validate_non_empty(&envelope.operation_id, "operation id")?;
    validate_non_empty(&envelope.project_id, "operation project id")?;
    validate_non_empty(&envelope.request_id, "operation request id")?;
    validate_non_empty(&envelope.agent_id, "operation agent id")?;
    validate_non_empty(&envelope.session_id, "operation session id")?;
    validate_non_empty(&envelope.schema_version, "operation schema version")?;
    validate_non_empty(&envelope.started_at, "operation started_at")?;
    validate_non_empty(&envelope.tool, "operation tool")?;
    validate_non_empty(&envelope.action, "operation action")?;
    validate_non_empty(&envelope.reversibility, "operation reversibility")?;
    validate_non_empty(&envelope.replayability, "operation replayability")?;
    validate_non_empty(&envelope.side_effect, "operation side effect")?;
    for reference in envelope
        .input_refs
        .iter()
        .chain(envelope.output_refs.iter())
    {
        validate_operation_ref(reference)?;
    }
    for value in [
        envelope.workspace_id.as_deref(),
        envelope.environment_id.as_deref(),
        envelope.parent_operation_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_non_empty(value, "operation association")?;
    }
    Ok(())
}

fn validate_operation_outcome(outcome: &OperationOutcome) -> Result<(), PongError> {
    validate_non_empty(&outcome.status, "operation status")?;
    validate_non_empty(&outcome.finished_at, "operation finished_at")?;
    if !matches!(
        outcome.status.as_str(),
        "completed" | "failed" | "cancelled" | "unknown"
    ) {
        return Err(PongError::InvalidInput(
            "operation outcome status is unsupported".into(),
        ));
    }
    match outcome.status.as_str() {
        "completed" if outcome.result.is_some() && outcome.error.is_none() => {}
        "failed" | "cancelled" | "unknown"
            if outcome.result.is_none() && outcome.error.is_some() => {}
        _ => {
            return Err(PongError::InvalidInput(
                "operation outcome must contain exactly one result or error".into(),
            ))
        }
    }
    if let Some(error) = outcome.error.as_ref() {
        validate_non_empty(&error.code, "operation error code")?;
        validate_non_empty(&error.message, "operation error message")?;
    }
    if let Some(references) = outcome.output_refs.as_ref() {
        for reference in references {
            validate_operation_ref(reference)?;
        }
    }
    Ok(())
}

fn validate_operation_bindings(
    transaction: &Transaction<'_>,
    project_id: &str,
    workspace_id: Option<&str>,
    environment_id: Option<&str>,
    parent_operation_id: Option<&str>,
) -> Result<(), PongError> {
    if let Some(workspace_id) = workspace_id {
        let workspace_project: Option<String> = transaction
            .query_row(
                "SELECT project_id FROM workspaces WHERE workspace_id = ?1",
                [workspace_id],
                |row| row.get(0),
            )
            .optional()?;
        if workspace_project.as_deref() != Some(project_id) {
            return Err(PongError::Conflict(
                "operation workspace is missing or belongs to another project".into(),
            ));
        }
    }
    if let Some(environment_id) = environment_id {
        let environment_project: Option<String> = transaction
            .query_row(
                "SELECT project_id FROM environments WHERE environment_id = ?1",
                [environment_id],
                |row| row.get(0),
            )
            .optional()?;
        if environment_project.as_deref() != Some(project_id) {
            return Err(PongError::Conflict(
                "operation environment is missing or belongs to another project".into(),
            ));
        }
    }
    if let Some(parent_operation_id) = parent_operation_id {
        let parent_project: Option<String> = transaction
            .query_row(
                "SELECT project_id FROM operations WHERE operation_id = ?1",
                [parent_operation_id],
                |row| row.get(0),
            )
            .optional()?;
        if parent_project.as_deref() != Some(project_id) {
            return Err(PongError::Conflict(
                "operation parent is missing or belongs to another project".into(),
            ));
        }
    }
    Ok(())
}

fn next_operation_event_sequence(
    transaction: &Transaction<'_>,
    operation_id: &str,
) -> Result<i64, PongError> {
    let stream_id = format!("operation:{operation_id}");
    transaction
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM events WHERE stream_id = ?1",
            [&stream_id],
            |row| row.get(0),
        )
        .map_err(PongError::from)
}

#[allow(clippy::too_many_arguments)]
fn append_operation_event(
    transaction: &Transaction<'_>,
    operation_id: &str,
    project_id: &str,
    agent_id: &str,
    request_id: &str,
    schema_version: &str,
    occurred_at: &str,
    sequence: i64,
    payload: &Value,
) -> Result<(), PongError> {
    let payload_json = canonical_json_value(payload)?;
    let payload_digest = hex::encode(Sha256::digest(payload_json.as_bytes()));
    let stream_id = format!("operation:{operation_id}");
    let event_id = format!("operation:{operation_id}:{sequence}");
    transaction.execute(
        "INSERT INTO events
         (event_id, project_id, stream_id, sequence, schema_version, actor_id,
          request_id, occurred_at, payload_digest, payload_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            event_id,
            project_id,
            stream_id,
            sequence,
            schema_version,
            agent_id,
            request_id,
            occurred_at,
            payload_digest,
            payload_json,
        ],
    )?;
    append_event_envelope_tx(
        transaction,
        NewEventEnvelope {
            event_id,
            project_id: project_id.to_owned(),
            stream_id,
            event_type: payload
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("operation.lifecycle")
                .to_owned(),
            schema_version: schema_version.to_owned(),
            occurred_at: occurred_at.to_owned(),
            recorded_at: occurred_at.to_owned(),
            actor_id: Some(agent_id.to_owned()),
            workspace_id: None,
            task_id: None,
            operation_id: Some(operation_id.to_owned()),
            causation_id: None,
            correlation_id: Some(request_id.to_owned()),
            parent_event_ids: Vec::new(),
            capture_confidence: Some("observed".into()),
            redaction_status: "redacted".into(),
            generation_id: None,
            migration_id: None,
            payload: payload.clone(),
        },
    )?;
    Ok(())
}

fn parse_operation_json<T: DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<T> {
    let value: String = row.get(index)?;
    serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}

fn parse_operation_optional_json<T: DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<T>> {
    let value: Option<String> = row.get(index)?;
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
            })
        })
        .transpose()
}

fn operation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OperationRecord> {
    Ok(OperationRecord {
        operation_id: row.get(0)?,
        project_id: row.get(1)?,
        request_id: row.get(2)?,
        agent_id: row.get(3)?,
        session_id: row.get(4)?,
        workspace_id: row.get(5)?,
        environment_id: row.get(6)?,
        parent_operation_id: row.get(7)?,
        schema_version: row.get(8)?,
        started_at: row.get(9)?,
        finished_at: row.get(10)?,
        tool: row.get(11)?,
        action: row.get(12)?,
        input_refs: parse_operation_json(row, 13)?,
        output_refs: parse_operation_json(row, 14)?,
        resource: parse_operation_optional_json(row, 15)?,
        before_state: parse_operation_optional_json(row, 16)?,
        after_state: parse_operation_optional_json(row, 17)?,
        result: parse_operation_optional_json(row, 18)?,
        error: parse_operation_optional_json(row, 19)?,
        reversibility: row.get(20)?,
        replayability: row.get(21)?,
        side_effect: row.get(22)?,
        policy_decision: parse_operation_optional_json(row, 23)?,
        lifecycle_status: row.get(24)?,
        recording_status: row.get(25)?,
        redaction_profile_id: row.get(26)?,
        redaction_profile_version: row.get(27)?,
        envelope_digest: row.get(28)?,
        updated_at: row.get(29)?,
    })
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRecord> {
    Ok(EventRecord {
        event_id: row.get(0)?,
        project_id: row.get(1)?,
        stream_id: row.get(2)?,
        sequence: row.get(3)?,
        schema_version: row.get(4)?,
        actor_id: row.get(5)?,
        request_id: row.get(6)?,
        occurred_at: row.get(7)?,
        payload_digest: row.get(8)?,
        payload_json: row.get(9)?,
    })
}

fn redact_event_envelope(redactor: &Redactor, event: NewEventEnvelope) -> NewEventEnvelope {
    NewEventEnvelope {
        event_id: redactor.redact_text(&event.event_id),
        project_id: redactor.redact_text(&event.project_id),
        stream_id: redactor.redact_text(&event.stream_id),
        event_type: redactor.redact_text(&event.event_type),
        schema_version: redactor.redact_text(&event.schema_version),
        occurred_at: redactor.redact_text(&event.occurred_at),
        recorded_at: redactor.redact_text(&event.recorded_at),
        actor_id: event.actor_id.as_deref().map(|v| redactor.redact_text(v)),
        workspace_id: event
            .workspace_id
            .as_deref()
            .map(|v| redactor.redact_text(v)),
        task_id: event.task_id.as_deref().map(|v| redactor.redact_text(v)),
        operation_id: event
            .operation_id
            .as_deref()
            .map(|v| redactor.redact_text(v)),
        causation_id: event
            .causation_id
            .as_deref()
            .map(|v| redactor.redact_text(v)),
        correlation_id: event
            .correlation_id
            .as_deref()
            .map(|v| redactor.redact_text(v)),
        parent_event_ids: event
            .parent_event_ids
            .iter()
            .map(|v| redactor.redact_text(v))
            .collect(),
        capture_confidence: event
            .capture_confidence
            .as_deref()
            .map(|v| redactor.redact_text(v)),
        redaction_status: redactor.redact_text(&event.redaction_status),
        generation_id: event
            .generation_id
            .as_deref()
            .map(|v| redactor.redact_text(v)),
        migration_id: event
            .migration_id
            .as_deref()
            .map(|v| redactor.redact_text(v)),
        payload: redactor.redact_value(&event.payload),
    }
}

fn validate_identity_assertion(
    generation_id: Option<&str>,
    migration_id: Option<&str>,
    expected: &(String, String),
    subject: &str,
) -> Result<(), PongError> {
    if let Some(provided) = generation_id {
        if provided != expected.0 {
            return Err(PongError::Integrity(format!(
                "{subject} generation identity {provided:?} does not match metadata identity {:?}",
                expected.0
            )));
        }
    }
    if let Some(provided) = migration_id {
        if provided != expected.1 {
            return Err(PongError::Integrity(format!(
                "{subject} migration identity {provided:?} does not match metadata identity {:?}",
                expected.1
            )));
        }
    }
    Ok(())
}

fn validate_event_envelope(event: &NewEventEnvelope) -> Result<(), PongError> {
    for (value, label) in [
        (&event.event_id, "event_id"),
        (&event.project_id, "project_id"),
        (&event.stream_id, "stream_id"),
        (&event.event_type, "event_type"),
        (&event.schema_version, "schema_version"),
        (&event.occurred_at, "occurred_at"),
        (&event.recorded_at, "recorded_at"),
        (&event.redaction_status, "redaction_status"),
    ] {
        validate_non_empty(value, label)?;
    }
    if event.parent_event_ids.iter().any(|id| id.trim().is_empty()) {
        return Err(PongError::InvalidInput(
            "parent_event_ids cannot contain empty values".into(),
        ));
    }
    Ok(())
}

fn append_event_envelope_tx(
    transaction: &Transaction<'_>,
    event: NewEventEnvelope,
) -> Result<EventEnvelope, PongError> {
    validate_event_envelope(&event)?;
    let payload_json = canonical_json_value(&event.payload)?;
    let payload_digest = digest_text(&payload_json);
    let parents_json = canonical_json(&event.parent_event_ids)?;
    if let Some(existing) = transaction
        .query_row(
            "SELECT event_id, project_id, stream_id, event_type, sequence,
                    project_sequence, schema_version, occurred_at, recorded_at,
                    actor_id, workspace_id, task_id, operation_id, causation_id,
                    correlation_id, parent_event_ids_json, capture_confidence,
                    redaction_status, payload_digest, payload_json
             FROM event_envelopes WHERE event_id = ?1",
            [&event.event_id],
            envelope_from_row,
        )
        .optional()?
    {
        if envelope_matches_input(&existing, &event, &payload_digest, &parents_json) {
            return Ok(existing);
        }
        return Err(PongError::Integrity(format!(
            "event ID {} was reused with different envelope content",
            event.event_id
        )));
    }
    let existing_sequence: Option<i64> = transaction
        .query_row(
            "SELECT sequence FROM events WHERE event_id = ?1",
            [&event.event_id],
            |row| row.get(0),
        )
        .optional()?;
    if existing_sequence.is_some() {
        let legacy: (String, String, String, String, String) = transaction.query_row(
            "SELECT project_id, stream_id, schema_version, occurred_at, payload_digest
             FROM events WHERE event_id = ?1",
            [&event.event_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        if legacy.0 != event.project_id
            || legacy.1 != event.stream_id
            || legacy.2 != event.schema_version
            || legacy.3 != event.occurred_at
            || legacy.4 != payload_digest
        {
            return Err(PongError::Integrity(format!(
                "legacy event {} conflicts with envelope identity",
                event.event_id
            )));
        }
    }
    let sequence = if let Some(sequence) = existing_sequence {
        sequence
    } else {
        transaction.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM events WHERE stream_id = ?1",
            [&event.stream_id],
            |row| row.get(0),
        )?
    };
    if existing_sequence.is_none() {
        transaction.execute(
            "INSERT INTO events
             (event_id, project_id, stream_id, sequence, schema_version, actor_id,
              request_id, occurred_at, payload_digest, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                event.event_id.clone(),
                event.project_id.clone(),
                event.stream_id.clone(),
                sequence,
                event.schema_version.clone(),
                event.actor_id.clone(),
                event.correlation_id.clone(),
                event.occurred_at.clone(),
                payload_digest.clone(),
                payload_json.clone(),
            ],
        )?;
    }
    let project_sequence: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(project_sequence), 0) + 1 FROM event_envelopes WHERE project_id = ?1",
        [&event.project_id],
        |row| row.get(0),
    )?;
    transaction.execute(
        "INSERT INTO event_envelopes
         (event_id, project_id, stream_id, sequence, project_sequence, event_type,
          schema_version, occurred_at, recorded_at, actor_id, workspace_id, task_id,
          operation_id, causation_id, correlation_id, parent_event_ids_json,
          capture_confidence, redaction_status, payload_digest, payload_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                 ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            event.event_id,
            event.project_id,
            event.stream_id,
            sequence,
            project_sequence,
            event.event_type,
            event.schema_version,
            event.occurred_at,
            event.recorded_at,
            event.actor_id,
            event.workspace_id,
            event.task_id,
            event.operation_id,
            event.causation_id,
            event.correlation_id,
            parents_json,
            event.capture_confidence,
            event.redaction_status,
            payload_digest,
            payload_json,
        ],
    )?;
    Ok(EventEnvelope {
        event_id: event.event_id,
        project_id: event.project_id,
        stream_id: event.stream_id,
        event_type: event.event_type,
        sequence,
        project_sequence,
        schema_version: event.schema_version,
        occurred_at: event.occurred_at,
        recorded_at: event.recorded_at,
        actor_id: event.actor_id,
        workspace_id: event.workspace_id,
        task_id: event.task_id,
        operation_id: event.operation_id,
        causation_id: event.causation_id,
        correlation_id: event.correlation_id,
        parent_event_ids: event.parent_event_ids,
        capture_confidence: event.capture_confidence,
        redaction_status: event.redaction_status,
        payload_digest,
        payload_json,
    })
}

fn envelope_matches_input(
    existing: &EventEnvelope,
    event: &NewEventEnvelope,
    payload_digest: &str,
    parents_json: &str,
) -> bool {
    existing.project_id == event.project_id
        && existing.stream_id == event.stream_id
        && existing.event_type == event.event_type
        && existing.schema_version == event.schema_version
        && existing.occurred_at == event.occurred_at
        && existing.recorded_at == event.recorded_at
        && existing.actor_id == event.actor_id
        && existing.workspace_id == event.workspace_id
        && existing.task_id == event.task_id
        && existing.operation_id == event.operation_id
        && existing.causation_id == event.causation_id
        && existing.correlation_id == event.correlation_id
        && existing.parent_event_ids == event.parent_event_ids
        && existing.capture_confidence == event.capture_confidence
        && existing.redaction_status == event.redaction_status
        && existing.payload_digest == payload_digest
        && existing.payload_json == canonical_json_value(&event.payload).unwrap_or_default()
        && canonical_json(&existing.parent_event_ids).ok().as_deref() == Some(parents_json)
}

fn envelope_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventEnvelope> {
    let parents: String = row.get(15)?;
    Ok(EventEnvelope {
        event_id: row.get(0)?,
        project_id: row.get(1)?,
        stream_id: row.get(2)?,
        event_type: row.get(3)?,
        sequence: row.get(4)?,
        project_sequence: row.get(5)?,
        schema_version: row.get(6)?,
        occurred_at: row.get(7)?,
        recorded_at: row.get(8)?,
        actor_id: row.get(9)?,
        workspace_id: row.get(10)?,
        task_id: row.get(11)?,
        operation_id: row.get(12)?,
        causation_id: row.get(13)?,
        correlation_id: row.get(14)?,
        parent_event_ids: serde_json::from_str(&parents).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(15, Type::Text, Box::new(error))
        })?,
        capture_confidence: row.get(16)?,
        redaction_status: row.get(17)?,
        payload_digest: row.get(18)?,
        payload_json: row.get(19)?,
    })
}

fn event_cursor(event: &EventEnvelope) -> ProjectionCursor {
    ProjectionCursor {
        project_sequence: event.project_sequence,
        stream_id: event.stream_id.clone(),
        sequence: event.sequence,
        event_id: event.event_id.clone(),
        payload_digest: event.payload_digest.clone(),
    }
}

fn projection_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectionRecord> {
    let cursor_project_sequence: Option<i64> = row.get(10)?;
    let cursor = cursor_project_sequence.map(|project_sequence| ProjectionCursor {
        project_sequence,
        stream_id: row.get(11).unwrap_or_default(),
        sequence: row.get(12).unwrap_or_default(),
        event_id: row.get(13).unwrap_or_default(),
        payload_digest: row.get(14).unwrap_or_default(),
    });
    Ok(ProjectionRecord {
        projection_id: row.get(0)?,
        project_id: row.get(1)?,
        schema_version: row.get(2)?,
        generation_id: row.get(3)?,
        migration_id: row.get(4)?,
        redaction_profile_id: row.get(5)?,
        redaction_profile_version: row.get(6)?,
        status: row.get(7)?,
        state_json: row.get(8)?,
        state_digest: row.get(9)?,
        cursor,
        event_count: row.get(15)?,
    })
}

fn validate_projection_definition(definition: &ProjectionDefinition) -> Result<(), PongError> {
    validate_non_empty(&definition.projection_id, "projection_id")?;
    validate_non_empty(&definition.project_id, "project_id")?;
    validate_non_empty(&definition.schema_version, "projection schema_version")
}

fn validate_projection_record_integrity(
    connection: &Connection,
    record: &ProjectionRecord,
) -> Result<(), PongError> {
    let state: Value = serde_json::from_str(&record.state_json).map_err(|error| {
        PongError::Integrity(format!(
            "projection {} state is not valid JSON: {error}",
            record.projection_id
        ))
    })?;
    let canonical_state = canonical_json_value(&state)?;
    if canonical_state != record.state_json
        || digest_text(&record.state_json) != record.state_digest
    {
        return Err(PongError::Integrity(format!(
            "projection {} state digest does not match canonical state",
            record.projection_id
        )));
    }

    let ledger_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM projection_events WHERE projection_id = ?1",
        [&record.projection_id],
        |row| row.get(0),
    )?;
    if ledger_count != record.event_count {
        return Err(PongError::Integrity(format!(
            "projection {} event count does not match applied-event ledger",
            record.projection_id
        )));
    }

    let invalid_ledger_event: Option<String> = connection
        .query_row(
            "SELECT pe.event_id
             FROM projection_events pe
             LEFT JOIN event_envelopes ee ON ee.event_id = pe.event_id
             WHERE pe.projection_id = ?1
               AND (ee.event_id IS NULL OR ee.project_id != ?2 OR ee.payload_digest != pe.payload_digest)
             LIMIT 1",
            params![record.projection_id, record.project_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(event_id) = invalid_ledger_event {
        return Err(PongError::Integrity(format!(
            "projection {} applied-event ledger conflicts with source event {}",
            record.projection_id, event_id
        )));
    }

    let max_applied_sequence: Option<i64> = connection.query_row(
        "SELECT MAX(ee.project_sequence)
             FROM projection_events pe
             JOIN event_envelopes ee ON ee.event_id = pe.event_id
            WHERE pe.projection_id = ?1",
        [&record.projection_id],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    match (&record.cursor, max_applied_sequence) {
        (None, None) => {}
        (Some(cursor), Some(max_sequence)) if cursor.project_sequence == max_sequence => {
            let cursor_matches_source: i64 = connection.query_row(
                "SELECT COUNT(*) FROM event_envelopes
                 WHERE event_id = ?1 AND project_id = ?2 AND project_sequence = ?3
                   AND stream_id = ?4 AND sequence = ?5 AND payload_digest = ?6",
                params![
                    cursor.event_id,
                    record.project_id,
                    cursor.project_sequence,
                    cursor.stream_id,
                    cursor.sequence,
                    cursor.payload_digest,
                ],
                |row| row.get(0),
            )?;
            if cursor_matches_source != 1 {
                return Err(PongError::Integrity(format!(
                    "projection {} cursor does not match its source event",
                    record.projection_id
                )));
            }
        }
        _ => {
            return Err(PongError::Integrity(format!(
                "projection {} cursor is inconsistent with its applied-event ledger",
                record.projection_id
            )));
        }
    }
    Ok(())
}

fn digest_text(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn workspace_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkspaceRecord> {
    Ok(WorkspaceRecord {
        workspace_id: row.get(0)?,
        project_id: row.get(1)?,
        driver: row.get(2)?,
        locator: row.get(3)?,
        branch_ref: row.get(4)?,
        head: row.get(5)?,
        environment_id: row.get(6)?,
        status: row.get(7)?,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn lease_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LeaseRecord> {
    Ok(LeaseRecord {
        workspace_id: row.get(0)?,
        agent_id: row.get(1)?,
        epoch: row.get(2)?,
        expires_at_ms: row.get(3)?,
        acquired_at_ms: row.get(4)?,
    })
}

fn environment_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EnvironmentRecord> {
    Ok(EnvironmentRecord {
        environment_id: row.get(0)?,
        project_id: row.get(1)?,
        fingerprint: row.get(2)?,
        facts_json: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn validate_non_empty(value: &str, label: &str) -> Result<(), PongError> {
    if value.trim().is_empty() || value.as_bytes().contains(&0) {
        return Err(PongError::InvalidInput(format!("{label} is invalid")));
    }
    Ok(())
}

fn validate_workspace_status(status: &str) -> Result<(), PongError> {
    if matches!(
        status,
        "created" | "preparing" | "ready" | "active" | "paused" | "reconciling" | "archived"
    ) {
        Ok(())
    } else {
        Err(PongError::InvalidInput(
            "workspace status is unsupported".into(),
        ))
    }
}

fn validate_workspace_record(record: &WorkspaceRecord) -> Result<(), PongError> {
    validate_non_empty(&record.workspace_id, "workspace id")?;
    validate_non_empty(&record.project_id, "project id")?;
    validate_non_empty(&record.driver, "workspace driver")?;
    validate_non_empty(&record.locator, "workspace locator")?;
    validate_workspace_status(&record.status)?;
    validate_workspace_ready_state(
        &record.status,
        record.head.as_deref(),
        record.environment_id.as_deref(),
    )?;
    if record.revision != 0 {
        return Err(PongError::InvalidInput(
            "new workspace revision must be zero".into(),
        ));
    }
    validate_non_empty(&record.created_at, "workspace created_at")?;
    validate_non_empty(&record.updated_at, "workspace updated_at")?;
    Ok(())
}

fn validate_workspace_ready_state(
    status: &str,
    head: Option<&str>,
    environment_id: Option<&str>,
) -> Result<(), PongError> {
    if matches!(status, "ready" | "active" | "paused")
        && (head.is_none() || environment_id.is_none())
    {
        return Err(PongError::InvalidInput(
            "ready workspace requires a head and environment revision".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    fn key() -> IdempotencyKey {
        IdempotencyKey {
            project_id: "prj_test".into(),
            actor_id: "agt_test".into(),
            request_id: "req_test".into(),
        }
    }

    fn redactor_with_secret(secret: &str) -> Redactor {
        let mut redactor = Redactor::new("test-profile", "0.1").expect("profile");
        redactor.register_secret(secret).expect("secret");
        redactor
    }

    fn assert_no_plaintext_in_database(directory: &Path, secret: &str) {
        let secret_bytes = secret.as_bytes();
        for entry in fs::read_dir(directory).expect("database directory") {
            let path = entry.expect("database entry").path();
            if !path.is_file() {
                continue;
            }
            let bytes = fs::read(&path).expect("database bytes");
            assert!(
                !bytes
                    .windows(secret_bytes.len())
                    .any(|window| window == secret_bytes),
                "secret found in {}",
                path.display()
            );
        }
    }

    #[test]
    fn repository_format_is_pinned() {
        let store = MetadataStore::in_memory().expect("store");
        assert_eq!(
            store.repository_format().expect("format"),
            REPOSITORY_FORMAT
        );
    }

    #[test]
    fn ref_compare_and_swap_rejects_stale_writer() {
        let mut store = MetadataStore::in_memory().expect("store");
        store
            .compare_and_swap_ref("refs/heads/main", None, "c1", "t1")
            .expect("initial ref");
        let error = store
            .compare_and_swap_ref("refs/heads/main", Some("stale"), "c2", "t2")
            .expect_err("stale ref must fail");
        assert_eq!(error.code(), "CONFLICT");
        assert_eq!(
            store.get_ref("refs/heads/main").expect("read"),
            Some("c1".into())
        );
    }

    #[test]
    fn idempotency_returns_original_result_and_rejects_changed_digest() {
        let mut store = MetadataStore::in_memory().expect("store");
        let result = json!({"status":"accepted","n":1});
        assert_eq!(
            store
                .record_idempotency(&key(), "sha256:a", &result, "t1")
                .expect("record"),
            IdempotencyResult::NewlyRecorded
        );
        assert_eq!(
            store
                .record_idempotency(
                    &key(),
                    "sha256:a",
                    &json!({"n":1,"status":"accepted"}),
                    "t2"
                )
                .expect("retry"),
            IdempotencyResult::Existing(result)
        );
        let error = store
            .record_idempotency(&key(), "sha256:b", &json!({"status":"changed"}), "t3")
            .expect_err("changed request must fail");
        assert_eq!(error.code(), "IDEMPOTENCY_KEY_REUSE");
    }

    #[test]
    fn events_have_monotonic_stream_sequences_and_deduplicate() {
        let mut store = MetadataStore::in_memory().expect("store");
        let first = NewEvent {
            event_id: "evt_1".into(),
            project_id: "prj_test".into(),
            stream_id: "project:prj_test".into(),
            schema_version: "0.1".into(),
            actor_id: Some("agt_test".into()),
            request_id: Some("req_1".into()),
            occurred_at: "2026-08-19T00:00:00Z".into(),
            payload: json!({"type":"one"}),
        };
        let second = NewEvent {
            event_id: "evt_2".into(),
            payload: json!({"type":"two"}),
            ..first.clone()
        };
        assert_eq!(
            store.append_event(first.clone()).expect("first").sequence,
            1
        );
        assert_eq!(store.append_event(second).expect("second").sequence, 2);
        assert_eq!(store.append_event(first).expect("duplicate").sequence, 1);
        assert_eq!(
            store.list_events("project:prj_test").expect("events").len(),
            2
        );
    }

    #[test]
    fn duplicate_event_id_with_changed_metadata_is_rejected() {
        let mut store = MetadataStore::in_memory().expect("store");
        let event = NewEvent {
            event_id: "evt_immutable".into(),
            project_id: "prj_test".into(),
            stream_id: "project:prj_test".into(),
            schema_version: "0.1".into(),
            actor_id: Some("agt_test".into()),
            request_id: Some("req_1".into()),
            occurred_at: "2026-08-19T00:00:00Z".into(),
            payload: json!({"type":"one"}),
        };
        store.append_event(event.clone()).expect("event");
        let changed = NewEvent {
            occurred_at: "2026-08-19T00:00:01Z".into(),
            ..event
        };
        let error = store
            .append_event(changed)
            .expect_err("changed duplicate must fail");
        assert_eq!(error.code(), "INTEGRITY_ERROR");
    }

    #[test]
    fn unfinished_intent_is_visible_for_recovery() {
        let mut store = MetadataStore::in_memory().expect("store");
        store
            .record_intent(
                "prj_test",
                "agt_test",
                "req_unknown",
                "op_unknown",
                &json!({"tool":"filesystem.write"}),
                "2026-08-19T00:00:00Z",
            )
            .expect("intent");
        let unfinished = store.unfinished_operations().expect("unfinished");
        assert_eq!(unfinished.len(), 1);
        assert_eq!(unfinished[0].phase, "intent_durable");
    }

    #[test]
    fn redaction_profile_is_pinned_across_reopen() {
        let directory = tempdir().expect("temporary repository");
        let path = directory.path().join("metadata.sqlite");
        let redactor = redactor_with_secret("profile-secret");
        let profile = redactor.profile();
        let store = MetadataStore::open_with_redactor(&path, redactor.clone()).expect("open");
        assert_eq!(store.redaction_profile(), profile);
        drop(store);

        let reopened = MetadataStore::open_with_redactor(&path, redactor).expect("reopen");
        assert_eq!(reopened.redaction_profile(), profile);
    }

    #[test]
    fn legacy_repository_without_profile_metadata_fails_closed() {
        let directory = tempdir().expect("temporary repository");
        let path = directory.path().join("metadata.sqlite");
        let connection = rusqlite::Connection::open(&path).expect("legacy database");
        connection
            .execute_batch(
                "CREATE TABLE repository_meta (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
                 INSERT INTO repository_meta(key, value) VALUES ('repository_format', '0.1');",
            )
            .expect("legacy metadata");
        drop(connection);

        let error = match MetadataStore::open(&path) {
            Ok(_) => panic!("legacy profile must be explicit"),
            Err(error) => error,
        };
        assert_eq!(error.code(), "UNSUPPORTED");
    }

    #[test]
    fn registered_secret_never_reaches_sqlite_or_wal_bytes() {
        let directory = tempdir().expect("temporary repository");
        let path = directory.path().join("metadata.sqlite");
        let secret = "sk_live_pong_123456";
        let redactor = redactor_with_secret(secret);
        let mut store = MetadataStore::open_with_redactor(path, redactor).expect("open");
        let key = IdempotencyKey {
            project_id: format!("project-{secret}"),
            actor_id: format!("actor-{secret}"),
            request_id: format!("request-{secret}"),
        };
        store
            .record_idempotency(
                &key,
                &format!("sha256:{secret}"),
                &json!({"result": secret, "password": "also hidden"}),
                &format!("created-{secret}"),
            )
            .expect("idempotency");
        store
            .append_event(NewEvent {
                event_id: format!("event-{secret}"),
                project_id: format!("project-{secret}"),
                stream_id: format!("stream-{secret}"),
                schema_version: format!("schema-{secret}"),
                actor_id: Some(format!("actor-{secret}")),
                request_id: Some(format!("request-{secret}")),
                occurred_at: format!("time-{secret}"),
                payload: json!({"body": secret, "api_key": secret}),
            })
            .expect("event");
        store
            .record_intent(
                &key.project_id,
                &key.actor_id,
                &key.request_id,
                &format!("operation-{secret}"),
                &json!({"input": secret}),
                &format!("intent-{secret}"),
            )
            .expect("intent");
        store
            .record_outcome(&key, "outcome_durable", &json!({"output": secret}))
            .expect("outcome");
        store
            .compare_and_swap_ref(
                &format!("refs/{secret}"),
                None,
                &format!("commit-{secret}"),
                &format!("updated-{secret}"),
            )
            .expect("ref");
        drop(store);

        assert_no_plaintext_in_database(directory.path(), secret);
    }

    #[test]
    fn duplicate_intent_with_changed_content_is_rejected() {
        let mut store = MetadataStore::in_memory().expect("store");
        store
            .record_intent(
                "project",
                "agent",
                "request",
                "operation-a",
                &json!({"path":"a.txt"}),
                "t1",
            )
            .expect("first intent");
        let error = store
            .record_intent(
                "project",
                "agent",
                "request",
                "operation-b",
                &json!({"path":"b.txt"}),
                "t2",
            )
            .expect_err("changed duplicate intent must fail");
        assert_eq!(error.code(), "IDEMPOTENCY_KEY_REUSE");
        let operation = store
            .operation(&IdempotencyKey {
                project_id: "project".into(),
                actor_id: "agent".into(),
                request_id: "request".into(),
            })
            .expect("operation lookup")
            .expect("original intent");
        assert_eq!(operation.operation_id, "operation-a");
        assert_eq!(operation.payload_json, r#"{"path":"a.txt"}"#);
    }

    #[test]
    fn opening_database_with_existing_secret_fails_closed() {
        let directory = tempdir().expect("temporary repository");
        let path = directory.path().join("metadata.sqlite");
        let secret = "legacy-secret-value";
        let connection = rusqlite::Connection::open(&path).expect("sqlite");
        connection
            .execute_batch(&format!(
                "CREATE TABLE leaked (value TEXT); INSERT INTO leaked(value) VALUES ('{secret}');"
            ))
            .expect("legacy secret");
        drop(connection);
        let mut redactor = Redactor::default();
        redactor.register_secret(secret).expect("secret");
        let error = match MetadataStore::open_with_redactor(&path, redactor) {
            Ok(_) => panic!("legacy secret must block startup"),
            Err(error) => error,
        };
        assert_eq!(error.code(), "INTEGRITY_ERROR");
        assert!(!error.to_string().contains(secret));
    }
}
