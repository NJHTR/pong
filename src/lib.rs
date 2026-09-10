//! Pong Core durable primitives.
//!
//! This crate intentionally stops below Workspace, Runtime, SDK, and CLI. The
//! first milestone proves object integrity, transactional metadata, event
//! ordering, and idempotent recovery before higher-level features depend on it.

pub(crate) mod atomic_replace;
pub mod canonical;
pub mod cas;
pub mod environment;
pub mod error;
pub mod metadata;
pub mod redaction;
pub mod repository;
pub mod workspace;

pub use environment::{EnvironmentFacts, ENVIRONMENT_SCHEMA_VERSION};
pub use error::PongError;
pub use metadata::{
    AgentIdentity, CheckpointCreation, CheckpointRecord, EnvironmentRecord, EventEnvelope,
    EventRecord, ExecutionCreation, ExecutionOperationRecord, ExecutionRecord, HandoffCreation,
    HandoffRecord, LeaseRecord, LeaseToken, MetadataFailpoint, MetadataFailpoints, NewEvent,
    NewEventEnvelope, OperationEnvelope, OperationError, OperationOutcome, OperationRecord,
    OperationRef, ProjectionAppliedEvent, ProjectionCursor, ProjectionDefinition,
    ProjectionFailpoint, ProjectionFailpoints, ProjectionHandler, ProjectionRecord, ResumeCreation,
    ResumeRecord, RollbackCreation, RollbackRecord, RollbackTarget, SnapshotPublication,
    SnapshotRecord, TaskCreation, TaskRecord, VersionCreation, VersionPublication, VersionRecord,
    WorkspaceRecord, WorkspaceUpdate, EVENT_ENVELOPE_SCHEMA_VERSION, OPERATION_SCHEMA_VERSION,
    PROJECTION_SCHEMA_VERSION, SNAPSHOT_SCHEMA_VERSION,
};
pub use repository::{
    MigrationFailpoint, MigrationFailpoints, MigrationOutcome, MigrationSpec, Repository,
    RepositoryGenerationManifest, RepositoryLayout, RepositoryMarker, RepositorySelector,
    GENERATION_REPOSITORY_FORMAT, GENERATION_SCHEMA_VERSION, REPOSITORY_FORMAT,
    REPOSITORY_MARKER_VERSION, REPOSITORY_SCHEMA_VERSION, REPOSITORY_SELECTOR_VERSION,
};
pub use workspace::{
    require_workspace_capability, transition_workspace_lifecycle, LocalWorkspace, RestoreOptions,
    RestoreResult, Snapshot, SnapshotChangeType, SnapshotDiff, SnapshotDiffEntry, SnapshotOptions,
    TreeEntry, TreeManifest, WorkspaceCapabilities, WorkspaceCapability, WorkspaceDiffResult,
    WorkspaceEnvironmentStatus, WorkspaceFailPoint, WorkspaceFailpoints, WorkspaceFaultAction,
    WorkspaceIdentity, WorkspaceLeaseStatus, WorkspaceLifecycleAction,
    WorkspaceLifecycleOperationOptions, WorkspaceLifecycleState, WorkspaceManager,
    WorkspaceOperationSummary, WorkspaceProviderContext, WorkspaceStatus,
};
