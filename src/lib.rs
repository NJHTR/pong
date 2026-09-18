//! Pong Core durable state and control primitives.
//!
//! External Agent Runtimes use the provider-neutral protocol and a single Core
//! owner. Direct [`Repository`] access is the embedded/internal Rust API for
//! library integration, tests, and offline maintenance; it is not a supported
//! multi-process Runtime transport.

pub(crate) mod atomic_replace;
pub mod canonical;
pub mod cas;
pub mod control;
pub mod core_service;
pub mod environment;
pub mod error;
pub mod http_transport;
pub mod metadata;
pub mod protocol;
pub mod redaction;
pub mod remote;
pub mod repository;
pub mod workspace;

pub use control::{
    AcquireWorkspaceRequest, AgentControl, CreateExecutionRequest, CreateTaskRequest,
    CreateWorkspaceRequest, LeaseView, PublishVersionRequest, PublishVersionResult,
    RegisterAgentRequest, ReleaseWorkspaceRequest, RenewWorkspaceRequest, SnapshotView, StateView,
    WorkspaceView,
};
pub use core_service::{AgentProtocolCore, ProtocolDispatch, ProtocolDispatchError};
pub use environment::{EnvironmentFacts, ENVIRONMENT_SCHEMA_VERSION};
pub use error::PongError;
pub use http_transport::{
    CredentialGrant, CredentialVerifier, HttpMetricsSnapshot, HttpRemoteServer,
    HttpRequestCorrelation, HttpServerConfig, HttpServerError, StaticCredentialVerifier,
    DEFAULT_HTTP_MAX_BODY_BYTES, DEFAULT_HTTP_MAX_RESPONSE_BYTES, DEFAULT_HTTP_RATE_LIMIT_REQUESTS,
    DEFAULT_HTTP_RATE_LIMIT_WINDOW_MS, DEFAULT_HTTP_SESSION_TTL_MS, DEFAULT_HTTP_WORKER_THREADS,
    HTTP_PROTOCOL_PATH, MIN_HTTP_MAX_RESPONSE_BYTES,
};
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
pub use remote::{
    AuthenticatedPrincipal, AuthorizedProtocolRequest, RemoteAccessBoundary,
    RemoteAccessCapabilities, RemoteAccessError, RemoteAccessErrorCode, RemoteSession,
    REMOTE_ACCESS_CONTRACT_VERSION,
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
