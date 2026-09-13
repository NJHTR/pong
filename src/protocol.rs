//! Transport-independent external Agent protocol.
//!
//! The protocol owns wire-safe DTOs and command/query semantics. It does not
//! own persistence, transport sessions, provider behavior, or authentication.

use crate::control::{
    AcquireWorkspaceRequest, AgentControl, CreateExecutionRequest, CreateTaskRequest,
    CreateWorkspaceRequest, LeaseView, PublishVersionRequest, RenewWorkspaceRequest, SnapshotView,
    StateView, WorkspaceView,
};
use crate::metadata::{
    AgentIdentity, CheckpointCreation, CheckpointRecord, ExecutionRecord, HandoffCreation,
    HandoffRecord, LeaseToken, OperationEnvelope, OperationError, OperationOutcome,
    OperationRecord, ResumeCreation, ResumeRecord, TaskRecord, VersionRecord,
    OPERATION_SCHEMA_VERSION,
};
use crate::workspace::{SnapshotChangeType, SnapshotDiff};
use crate::{PongError, Repository};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const EXTERNAL_AGENT_PROTOCOL_VERSION: &str = "1.0";

const COMMAND_CAPABILITIES: &[&str] = &[
    "register_agent",
    "create_task",
    "create_workspace",
    "create_execution",
    "start_execution",
    "pause_execution",
    "complete_execution",
    "fail_execution",
    "interrupt_execution",
    "acquire_workspace_lease",
    "renew_workspace_lease",
    "release_workspace_lease",
    "publish_version",
    "set_execution_current_version",
    "create_checkpoint",
    "resume_from_checkpoint",
    "create_handoff",
    "materialize_version",
    "start_operation",
    "finish_operation",
];

const QUERY_CAPABILITIES: &[&str] = &[
    "hello",
    "get_agent",
    "get_task",
    "get_execution",
    "get_workspace",
    "get_version",
    "get_checkpoint",
    "get_handoff",
    "get_operation",
    "resolve_operation",
    "inspect_execution",
    "list_checkpoints",
    "list_handoffs",
    "diff_workspace_version",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolRequest {
    pub protocol_version: String,
    pub request_id: String,
    pub caller_agent_id: Option<String>,
    pub issued_at: String,
    pub operation_id: Option<String>,
    #[serde(flatten)]
    pub call: ProtocolCall,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "payload", rename_all = "snake_case")]
pub enum ProtocolCall {
    Hello,
    RegisterAgent(RegisterAgentCommand),
    CreateTask(CreateTaskCommand),
    CreateWorkspace(CreateWorkspaceCommand),
    CreateExecution(CreateExecutionCommand),
    StartExecution(ExecutionRevisionCommand),
    PauseExecution(ExecutionOutcomeCommand),
    CompleteExecution(ExecutionOutcomeCommand),
    FailExecution(ExecutionOutcomeCommand),
    InterruptExecution(ExecutionOutcomeCommand),
    AcquireWorkspaceLease(AcquireLeaseCommand),
    RenewWorkspaceLease(RenewLeaseCommand),
    ReleaseWorkspaceLease(ReleaseLeaseCommand),
    PublishVersion(PublishVersionCommand),
    SetExecutionCurrentVersion(SetExecutionVersionCommand),
    CreateCheckpoint(CreateCheckpointCommand),
    ResumeFromCheckpoint(ResumeCheckpointCommand),
    CreateHandoff(CreateHandoffCommand),
    MaterializeVersion(MaterializeVersionCommand),
    StartOperation(StartOperationCommand),
    FinishOperation(FinishOperationCommand),
    GetAgent(EntityQuery),
    GetTask(EntityQuery),
    GetExecution(EntityQuery),
    GetWorkspace(EntityQuery),
    GetVersion(EntityQuery),
    GetCheckpoint(EntityQuery),
    GetHandoff(EntityQuery),
    GetOperation(EntityQuery),
    ResolveOperation(OperationRequestQuery),
    InspectExecution(EntityQuery),
    ListCheckpoints(TaskQuery),
    ListHandoffs(TaskQuery),
    DiffWorkspaceVersion(DiffWorkspaceVersionQuery),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterAgentCommand {
    pub agent_id: String,
    pub provider_metadata: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTaskCommand {
    pub task_id: String,
    pub project_id: String,
    pub goal_ref: String,
    pub context_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWorkspaceCommand {
    pub workspace_id: String,
    pub project_id: String,
    pub binding_ref: String,
    pub branch_ref: Option<String>,
    pub environment_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateExecutionCommand {
    pub execution_id: String,
    pub task_id: String,
    pub parent_execution_id: Option<String>,
    pub workspace_id: Option<String>,
    pub base_version_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRevisionCommand {
    pub execution_id: String,
    pub expected_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionOutcomeCommand {
    pub execution_id: String,
    pub expected_revision: i64,
    pub outcome_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartOperationCommand {
    pub execution_id: String,
    pub action: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperationTerminalStatus {
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

impl OperationTerminalStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationFailureResource {
    pub code: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishOperationCommand {
    pub execution_id: String,
    pub status: OperationTerminalStatus,
    pub failure: Option<OperationFailureResource>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquireLeaseCommand {
    pub execution_id: String,
    pub workspace_id: String,
    pub ttl_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenewLeaseCommand {
    pub lease: LeaseAuthority,
    pub ttl_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseLeaseCommand {
    pub lease: LeaseAuthority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishVersionCommand {
    pub execution_id: String,
    pub workspace_id: String,
    pub lease: LeaseAuthority,
    pub expected_workspace_revision: i64,
    pub parent_version_id: Option<String>,
    pub update_version_head: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetExecutionVersionCommand {
    pub execution_id: String,
    pub version_id: String,
    pub lease: LeaseAuthority,
    pub expected_execution_revision: i64,
    pub expected_workspace_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCheckpointCommand {
    pub checkpoint_id: String,
    pub task_id: String,
    pub execution_id: String,
    pub workspace_id: String,
    pub version_id: String,
    pub source_operation_id: Option<String>,
    pub reason_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeCheckpointCommand {
    pub execution_id: String,
    pub task_id: String,
    pub parent_execution_id: Option<String>,
    pub workspace_id: String,
    pub checkpoint_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateHandoffCommand {
    pub handoff_id: String,
    pub task_id: String,
    pub from_execution_id: String,
    pub to_execution_id: String,
    pub source_version_id: Option<String>,
    pub checkpoint_id: Option<String>,
    pub reason_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializeVersionCommand {
    pub execution_id: String,
    pub workspace_id: String,
    pub source_version_id: String,
    pub lease: LeaseAuthority,
    pub expected_workspace_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityQuery {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationRequestQuery {
    pub project_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskQuery {
    pub task_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffWorkspaceVersionQuery {
    pub workspace_id: String,
    pub source_version_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseAuthority {
    pub workspace_id: String,
    pub agent_id: String,
    pub epoch: i64,
    pub expires_at_ms: i64,
}

impl From<LeaseToken> for LeaseAuthority {
    fn from(value: LeaseToken) -> Self {
        Self {
            workspace_id: value.workspace_id,
            agent_id: value.agent_id,
            epoch: value.epoch,
            expires_at_ms: value.expires_at_ms,
        }
    }
}

impl From<&LeaseAuthority> for LeaseToken {
    fn from(value: &LeaseAuthority) -> Self {
        Self {
            workspace_id: value.workspace_id.clone(),
            agent_id: value.agent_id.clone(),
            epoch: value.epoch,
            expires_at_ms: value.expires_at_ms,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolStatus {
    Ok,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolResponse {
    pub protocol_version: String,
    pub request_id: String,
    pub operation_id: Option<String>,
    pub status: ProtocolStatus,
    pub result: Option<ProtocolResult>,
    pub error: Option<ProtocolError>,
}

impl ProtocolResponse {
    pub fn invalid_envelope(request_id: Option<String>) -> Self {
        Self::failure(
            request_id.unwrap_or_else(|| "unknown".into()),
            None,
            ProtocolFailure::validation("request envelope is invalid"),
        )
    }

    fn success(request_id: String, operation_id: Option<String>, result: ProtocolResult) -> Self {
        Self {
            protocol_version: EXTERNAL_AGENT_PROTOCOL_VERSION.into(),
            request_id,
            operation_id,
            status: ProtocolStatus::Ok,
            result: Some(result),
            error: None,
        }
    }

    fn failure(request_id: String, operation_id: Option<String>, failure: ProtocolFailure) -> Self {
        Self {
            protocol_version: EXTERNAL_AGENT_PROTOCOL_VERSION.into(),
            request_id,
            operation_id,
            status: ProtocolStatus::Error,
            result: None,
            error: Some(failure.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ProtocolResult {
    Hello(HelloResult),
    Agent(AgentResource),
    Task(TaskResource),
    Execution(ExecutionResource),
    Workspace(WorkspaceResource),
    WorkspaceInspection(WorkspaceInspection),
    Lease(LeaseResource),
    Released(ReleasedResource),
    Version(VersionResource),
    VersionPublication(Box<VersionPublicationResource>),
    Snapshot(SnapshotResource),
    Checkpoint(CheckpointResource),
    Checkpoints(ResourceList<CheckpointResource>),
    Handoff(HandoffResource),
    Handoffs(ResourceList<HandoffResource>),
    Resume(ResumeResource),
    Operation(OperationResource),
    ExecutionInspection(Box<ExecutionInspection>),
    Diff(SnapshotDiffResource),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelloResult {
    pub protocol_versions: Vec<String>,
    pub commands: Vec<String>,
    pub queries: Vec<String>,
    pub mutation_acknowledgement: String,
    pub provider_neutral: bool,
    pub transport_neutral: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceList<T> {
    pub items: Vec<T>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentResource {
    pub agent_id: String,
    pub provider_metadata: Option<String>,
    pub display_name: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskResource {
    pub task_id: String,
    pub project_id: String,
    pub goal_ref: String,
    pub context_ref: Option<String>,
    pub state: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionResource {
    pub execution_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub project_id: String,
    pub parent_execution_id: Option<String>,
    pub workspace_id: Option<String>,
    pub base_version_id: Option<String>,
    pub current_version_id: Option<String>,
    pub state: String,
    pub outcome_code: Option<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceResource {
    pub workspace_id: String,
    pub project_id: String,
    pub branch_ref: Option<String>,
    pub head: Option<String>,
    pub version_head_id: Option<String>,
    pub environment_id: Option<String>,
    pub state: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceInspection {
    pub workspace: WorkspaceResource,
    pub lease: Option<LeaseResource>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseResource {
    pub authority: LeaseAuthority,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleasedResource {
    pub entity_kind: String,
    pub entity_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionResource {
    pub version_id: String,
    pub workspace_id: String,
    pub project_id: String,
    pub snapshot_id: String,
    pub creation_operation_id: String,
    pub environment_id: Option<String>,
    pub parent_version_id: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotResource {
    pub snapshot_id: String,
    pub root_digest: String,
    pub workspace_id: String,
    pub project_id: String,
    pub environment_id: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionPublicationResource {
    pub snapshot: SnapshotResource,
    pub version: VersionResource,
    pub workspace: WorkspaceResource,
    pub execution_id: String,
    pub operation_attached: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointResource {
    pub checkpoint_id: String,
    pub task_id: String,
    pub execution_id: String,
    pub workspace_id: String,
    pub version_id: String,
    pub source_operation_id: Option<String>,
    pub reason_code: String,
    pub actor_agent_id: String,
    pub request_id: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffResource {
    pub handoff_id: String,
    pub task_id: String,
    pub from_execution_id: String,
    pub to_execution_id: String,
    pub source_version_id: Option<String>,
    pub checkpoint_id: Option<String>,
    pub reason_code: String,
    pub actor_agent_id: String,
    pub request_id: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeResource {
    pub execution_id: String,
    pub task_id: String,
    pub agent_id: String,
    pub source_version_id: String,
    pub checkpoint_id: Option<String>,
    pub request_id: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationResource {
    pub operation_id: String,
    pub execution_id: Option<String>,
    pub request_id: String,
    pub agent_id: String,
    pub workspace_id: Option<String>,
    pub action: String,
    pub state: String,
    pub recording_state: String,
    pub failure_code: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionInspection {
    pub agent: AgentResource,
    pub task: TaskResource,
    pub execution: ExecutionResource,
    pub workspace: Option<WorkspaceInspection>,
    pub base_version: Option<VersionResource>,
    pub current_version: Option<VersionResource>,
    pub checkpoints: Vec<CheckpointResource>,
    pub handoffs: Vec<HandoffResource>,
    pub resume: Option<ResumeResource>,
    pub operations: Vec<OperationResource>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDiffResource {
    pub source_snapshot_id: String,
    pub current_tree_id: String,
    pub entries: Vec<SnapshotDiffEntryResource>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDiffEntryResource {
    pub path: String,
    pub change: String,
    pub old_digest: Option<String>,
    pub new_digest: Option<String>,
    pub old_size: Option<u64>,
    pub new_size: Option<u64>,
    pub old_type: Option<String>,
    pub new_type: Option<String>,
}

impl From<AgentIdentity> for AgentResource {
    fn from(value: AgentIdentity) -> Self {
        Self {
            agent_id: value.agent_id,
            provider_metadata: Some(value.provider),
            display_name: value.display_name,
            created_at: value.created_at,
        }
    }
}

impl From<TaskRecord> for TaskResource {
    fn from(value: TaskRecord) -> Self {
        Self {
            task_id: value.task_id,
            project_id: value.project_id,
            goal_ref: value.goal,
            context_ref: value.context_ref,
            state: value.state,
            revision: value.revision,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<ExecutionRecord> for ExecutionResource {
    fn from(value: ExecutionRecord) -> Self {
        Self {
            execution_id: value.execution_id,
            task_id: value.task_id,
            agent_id: value.agent_id,
            project_id: value.project_id,
            parent_execution_id: value.parent_execution_id,
            workspace_id: value.workspace_id,
            base_version_id: value.base_version_id,
            current_version_id: value.current_version_id,
            state: value.state,
            outcome_code: value.outcome,
            revision: value.revision,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<WorkspaceView> for WorkspaceResource {
    fn from(value: WorkspaceView) -> Self {
        Self {
            workspace_id: value.workspace_id,
            project_id: value.project_id,
            branch_ref: value.branch_ref,
            head: value.head,
            version_head_id: value.version_head_id,
            environment_id: value.environment_id,
            state: value.status,
            revision: value.revision,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<VersionRecord> for VersionResource {
    fn from(value: VersionRecord) -> Self {
        Self {
            version_id: value.version_id,
            workspace_id: value.workspace_id,
            project_id: value.project_id,
            snapshot_id: value.snapshot_id,
            creation_operation_id: value.creation_operation_id,
            environment_id: value.environment_id,
            parent_version_id: value.parent_version_id,
            created_at: value.created_at,
        }
    }
}

impl From<SnapshotView> for SnapshotResource {
    fn from(value: SnapshotView) -> Self {
        Self {
            snapshot_id: value.snapshot_id,
            root_digest: value.root_digest,
            workspace_id: value.workspace_id,
            project_id: value.project_id,
            environment_id: value.environment_id,
            file_count: value.file_count,
            total_bytes: value.total_bytes,
            created_at: value.created_at,
        }
    }
}

impl From<CheckpointRecord> for CheckpointResource {
    fn from(value: CheckpointRecord) -> Self {
        Self {
            checkpoint_id: value.checkpoint_id,
            task_id: value.task_id,
            execution_id: value.execution_id,
            workspace_id: value.workspace_id,
            version_id: value.version_id,
            source_operation_id: value.operation_id,
            reason_code: value.reason,
            actor_agent_id: value.actor_agent_id,
            request_id: value.request_id,
            created_at: value.created_at,
        }
    }
}

impl From<HandoffRecord> for HandoffResource {
    fn from(value: HandoffRecord) -> Self {
        Self {
            handoff_id: value.handoff_id,
            task_id: value.task_id,
            from_execution_id: value.from_execution_id,
            to_execution_id: value.to_execution_id,
            source_version_id: value.source_version_id,
            checkpoint_id: value.checkpoint_id,
            reason_code: value.reason,
            actor_agent_id: value.actor_agent_id,
            request_id: value.request_id,
            state: value.status,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<ResumeRecord> for ResumeResource {
    fn from(value: ResumeRecord) -> Self {
        Self {
            execution_id: value.execution_id,
            task_id: value.task_id,
            agent_id: value.agent_id,
            source_version_id: value.source_version_id,
            checkpoint_id: value.checkpoint_id,
            request_id: value.request_id,
            created_at: value.created_at,
        }
    }
}

impl OperationResource {
    fn from_record(value: OperationRecord, execution_id: Option<String>) -> Self {
        Self {
            operation_id: value.operation_id,
            execution_id,
            request_id: value.request_id,
            agent_id: value.agent_id,
            workspace_id: value.workspace_id,
            action: value.action,
            state: value.lifecycle_status,
            recording_state: value.recording_status,
            failure_code: value.error.map(|error| error.code),
            started_at: value.started_at,
            finished_at: value.finished_at,
        }
    }
}

impl From<SnapshotDiff> for SnapshotDiffResource {
    fn from(value: SnapshotDiff) -> Self {
        Self {
            source_snapshot_id: value.old_snapshot_id,
            current_tree_id: value.new_snapshot_id,
            entries: value
                .entries
                .into_iter()
                .map(|entry| SnapshotDiffEntryResource {
                    path: entry.path,
                    change: match entry.change_type {
                        SnapshotChangeType::Added => "ADDED",
                        SnapshotChangeType::Removed => "REMOVED",
                        SnapshotChangeType::Modified => "MODIFIED",
                        SnapshotChangeType::TypeChanged => "TYPE_CHANGED",
                    }
                    .into(),
                    old_digest: entry.old_digest,
                    new_digest: entry.new_digest,
                    old_size: entry.old_size,
                    new_size: entry.new_size,
                    old_type: entry.old_type,
                    new_type: entry.new_type,
                })
                .collect(),
        }
    }
}

impl ExecutionInspection {
    fn from_state(state: StateView, operations: Vec<OperationRecord>) -> Self {
        let execution_id = state.execution.execution_id.clone();
        let workspace = state.workspace.map(|workspace| WorkspaceInspection {
            workspace: workspace.into(),
            lease: state.lease.and_then(lease_resource_from_view),
        });
        Self {
            agent: state.agent.into(),
            task: state.task.into(),
            execution: state.execution.into(),
            workspace,
            base_version: state.base_version.map(Into::into),
            current_version: state.current_version.map(Into::into),
            checkpoints: state
                .checkpoints
                .into_iter()
                .filter(|checkpoint| checkpoint.execution_id == execution_id)
                .map(Into::into)
                .collect(),
            handoffs: state
                .handoffs
                .into_iter()
                .filter(|handoff| {
                    handoff.from_execution_id == execution_id
                        || handoff.to_execution_id == execution_id
                })
                .map(Into::into)
                .collect(),
            resume: state.resume.map(Into::into),
            operations: operations
                .into_iter()
                .map(|operation| {
                    OperationResource::from_record(operation, Some(execution_id.clone()))
                })
                .collect(),
        }
    }
}

fn lease_resource_from_view(value: LeaseView) -> Option<LeaseResource> {
    let agent_id = value.agent_id?;
    Some(LeaseResource {
        authority: LeaseAuthority {
            workspace_id: value.workspace_id,
            agent_id,
            epoch: value.epoch,
            expires_at_ms: value.expires_at_ms,
        },
        active: value.active,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProtocolErrorCode {
    UnsupportedVersion,
    ValidationError,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    IntegrityError,
    LeaseConflict,
    RevisionConflict,
    InvalidState,
    IdempotencyConflict,
    NotSupported,
    RecoveryRequired,
    ResourceExhausted,
    InternalError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolError {
    pub code: ProtocolErrorCode,
    pub message: String,
    pub retryable: bool,
    pub details: Option<ProtocolErrorDetails>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolErrorDetails {
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub expected_revision: Option<i64>,
    pub actual_revision: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceBindingError {
    Invalid,
    NotFound,
    Forbidden,
    Unavailable,
}

pub trait WorkspaceBindingResolver {
    fn resolve(&self, binding_ref: &str) -> Result<PathBuf, WorkspaceBindingError>;
}

pub struct ExternalAgentProtocol<'repository, 'bindings> {
    repository: &'repository mut Repository,
    workspace_bindings: &'bindings dyn WorkspaceBindingResolver,
}

impl<'repository, 'bindings> ExternalAgentProtocol<'repository, 'bindings> {
    pub fn new(
        repository: &'repository mut Repository,
        workspace_bindings: &'bindings dyn WorkspaceBindingResolver,
    ) -> Self {
        Self {
            repository,
            workspace_bindings,
        }
    }

    /// Handle one decoded protocol request. `now_ms` is supplied by the host,
    /// never by the external caller, so lease expiry cannot be bypassed by a
    /// forged request timestamp.
    pub fn handle(&mut self, request: ProtocolRequest, now_ms: i64) -> ProtocolResponse {
        let request_id = request.request_id.clone();
        let operation_id = request.operation_id.clone();
        match self.dispatch(request, now_ms) {
            Ok(result) => ProtocolResponse::success(request_id, operation_id, result),
            Err(error) => ProtocolResponse::failure(request_id, operation_id, error),
        }
    }

    fn dispatch(
        &mut self,
        request: ProtocolRequest,
        now_ms: i64,
    ) -> Result<ProtocolResult, ProtocolFailure> {
        validate_request_header(&request)?;
        if request.protocol_version != EXTERNAL_AGENT_PROTOCOL_VERSION {
            return Err(ProtocolFailure::new(
                ProtocolErrorCode::UnsupportedVersion,
                "protocol version is not supported",
                false,
            ));
        }

        let caller = request.caller_agent_id.as_deref();
        match request.call {
            ProtocolCall::Hello => Ok(ProtocolResult::Hello(HelloResult {
                protocol_versions: vec![EXTERNAL_AGENT_PROTOCOL_VERSION.into()],
                commands: COMMAND_CAPABILITIES
                    .iter()
                    .map(|value| (*value).into())
                    .collect(),
                queries: QUERY_CAPABILITIES
                    .iter()
                    .map(|value| (*value).into())
                    .collect(),
                mutation_acknowledgement: "durable_or_error".into(),
                provider_neutral: true,
                transport_neutral: true,
            })),
            ProtocolCall::RegisterAgent(command) => {
                let caller = required_caller(caller)?;
                if caller != command.agent_id {
                    return Err(ProtocolFailure::forbidden(
                        "an Agent may register only its own asserted identity",
                    ));
                }
                let agent = AgentControl::new(self.repository)
                    .register_agent(crate::control::RegisterAgentRequest {
                        agent_id: command.agent_id,
                        provider: command
                            .provider_metadata
                            .unwrap_or_else(|| "unspecified".into()),
                        display_name: command.display_name,
                        created_at: request.issued_at,
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Agent(agent.into()))
            }
            call => {
                let caller = required_caller(caller)?.to_owned();
                self.require_registered_agent(&caller)?;
                self.dispatch_authenticated(
                    &caller,
                    &request.request_id,
                    request.operation_id.as_deref(),
                    &request.issued_at,
                    call,
                    now_ms,
                )
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dispatch_authenticated(
        &mut self,
        caller: &str,
        request_id: &str,
        operation_id: Option<&str>,
        issued_at: &str,
        call: ProtocolCall,
        now_ms: i64,
    ) -> Result<ProtocolResult, ProtocolFailure> {
        match call {
            ProtocolCall::CreateTask(command) => {
                let task = AgentControl::new(self.repository)
                    .create_task(CreateTaskRequest {
                        task_id: command.task_id,
                        project_id: command.project_id,
                        goal: command.goal_ref,
                        context_ref: command.context_ref,
                        created_at: issued_at.into(),
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Task(task.into()))
            }
            ProtocolCall::CreateWorkspace(command) => {
                let path = self
                    .workspace_bindings
                    .resolve(&command.binding_ref)
                    .map_err(ProtocolFailure::from_workspace_binding)?;
                let workspace = AgentControl::new(self.repository)
                    .create_workspace(CreateWorkspaceRequest {
                        workspace_id: command.workspace_id,
                        project_id: command.project_id,
                        path,
                        branch_ref: command.branch_ref,
                        environment_id: command.environment_id,
                        now: issued_at.into(),
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Workspace(workspace.into()))
            }
            ProtocolCall::CreateExecution(command) => {
                let execution = AgentControl::new(self.repository)
                    .create_execution(CreateExecutionRequest {
                        execution_id: command.execution_id,
                        task_id: command.task_id,
                        agent_id: caller.into(),
                        parent_execution_id: command.parent_execution_id,
                        workspace_id: command.workspace_id,
                        base_version_id: command.base_version_id,
                        current_version_id: None,
                        created_at: issued_at.into(),
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Execution(execution.into()))
            }
            ProtocolCall::StartExecution(command) => {
                self.require_execution_transition(
                    caller,
                    &command.execution_id,
                    command.expected_revision,
                    "running",
                    None,
                )?;
                let execution = AgentControl::new(self.repository)
                    .start_execution(&command.execution_id, command.expected_revision, issued_at)
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Execution(execution.into()))
            }
            ProtocolCall::PauseExecution(command) => {
                self.execution_transition(caller, command, "paused", issued_at)
            }
            ProtocolCall::CompleteExecution(command) => {
                self.execution_transition(caller, command, "completed", issued_at)
            }
            ProtocolCall::FailExecution(command) => {
                self.execution_transition(caller, command, "failed", issued_at)
            }
            ProtocolCall::InterruptExecution(command) => {
                self.execution_transition(caller, command, "interrupted", issued_at)
            }
            ProtocolCall::AcquireWorkspaceLease(command) => {
                self.require_execution_workspace(
                    caller,
                    &command.execution_id,
                    &command.workspace_id,
                )?;
                if command.ttl_ms <= 0 {
                    return Err(ProtocolFailure::validation("lease ttl_ms must be positive"));
                }
                if let Some(lease) = AgentControl::new(self.repository)
                    .lease(&command.workspace_id, now_ms)
                    .map_err(ProtocolFailure::from_pong)?
                    .filter(|lease| lease.active)
                {
                    if lease.agent_id.as_deref() == Some(caller) {
                        return Ok(ProtocolResult::Lease(
                            lease_resource_from_view(lease).ok_or_else(|| {
                                ProtocolFailure::new(
                                    ProtocolErrorCode::IntegrityError,
                                    "active lease has no Agent owner",
                                    false,
                                )
                            })?,
                        ));
                    }
                    return Err(ProtocolFailure::lease(&command.workspace_id));
                }
                let lease = AgentControl::new(self.repository)
                    .acquire_workspace(AcquireWorkspaceRequest {
                        workspace_id: command.workspace_id,
                        agent_id: caller.into(),
                        now_ms,
                        ttl_ms: command.ttl_ms,
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Lease(LeaseResource {
                    authority: lease.into(),
                    active: true,
                }))
            }
            ProtocolCall::RenewWorkspaceLease(command) => {
                self.require_lease(caller, &command.lease, now_ms)?;
                if command.ttl_ms <= 0 {
                    return Err(ProtocolFailure::validation("lease ttl_ms must be positive"));
                }
                let lease = AgentControl::new(self.repository)
                    .renew_workspace(RenewWorkspaceRequest {
                        lease: (&command.lease).into(),
                        now_ms,
                        ttl_ms: command.ttl_ms,
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Lease(LeaseResource {
                    authority: lease.into(),
                    active: true,
                }))
            }
            ProtocolCall::ReleaseWorkspaceLease(command) => {
                let existing = AgentControl::new(self.repository)
                    .lease(&command.lease.workspace_id, now_ms)
                    .map_err(ProtocolFailure::from_pong)?;
                if existing.as_ref().is_some_and(|lease| {
                    !lease.active && lease.agent_id.is_none() && lease.epoch == command.lease.epoch
                }) {
                    return Ok(ProtocolResult::Released(ReleasedResource {
                        entity_kind: "workspace_lease".into(),
                        entity_id: command.lease.workspace_id,
                    }));
                }
                self.require_lease(caller, &command.lease, now_ms)?;
                let workspace_id = command.lease.workspace_id.clone();
                AgentControl::new(self.repository)
                    .release_workspace(crate::control::ReleaseWorkspaceRequest {
                        lease: (&command.lease).into(),
                        now_ms,
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Released(ReleasedResource {
                    entity_kind: "workspace_lease".into(),
                    entity_id: workspace_id,
                }))
            }
            ProtocolCall::PublishVersion(command) => {
                let operation_id = required_operation_id(operation_id)?;
                self.require_execution_workspace(
                    caller,
                    &command.execution_id,
                    &command.workspace_id,
                )?;
                if self
                    .repository
                    .metadata()
                    .operation_record(operation_id)
                    .map_err(ProtocolFailure::from_pong)?
                    .is_none()
                {
                    self.require_workspace_revision(
                        &command.workspace_id,
                        command.expected_workspace_revision,
                    )?;
                }
                self.require_lease(caller, &command.lease, now_ms)?;
                let published = AgentControl::new(self.repository)
                    .publish_version(PublishVersionRequest {
                        workspace_id: command.workspace_id,
                        lease: (&command.lease).into(),
                        expected_workspace_revision: command.expected_workspace_revision,
                        now_ms,
                        created_at: issued_at.into(),
                        operation_id: operation_id.into(),
                        request_id: request_id.into(),
                        session_id: Some(format!("protocol-request:{request_id}")),
                        tool: Some("external-agent-protocol".into()),
                        parent_version_id: command.parent_version_id,
                        update_version_head: command.update_version_head,
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                AgentControl::new(self.repository)
                    .attach_operation_to_execution(&command.execution_id, operation_id, issued_at)
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::VersionPublication(Box::new(
                    VersionPublicationResource {
                        snapshot: published.snapshot.into(),
                        version: published.version.into(),
                        workspace: published.workspace.into(),
                        execution_id: command.execution_id,
                        operation_attached: true,
                    },
                )))
            }
            ProtocolCall::SetExecutionCurrentVersion(command) => {
                self.require_execution_version_revisions(
                    caller,
                    &command.execution_id,
                    &command.lease.workspace_id,
                    &command.version_id,
                    command.expected_execution_revision,
                    command.expected_workspace_revision,
                )?;
                self.require_lease(caller, &command.lease, now_ms)?;
                let execution = AgentControl::new(self.repository)
                    .set_execution_current_version(
                        &command.execution_id,
                        &command.version_id,
                        &(&command.lease).into(),
                        command.expected_execution_revision,
                        command.expected_workspace_revision,
                        issued_at,
                        now_ms,
                    )
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Execution(execution.into()))
            }
            ProtocolCall::CreateCheckpoint(command) => {
                self.require_execution_workspace(
                    caller,
                    &command.execution_id,
                    &command.workspace_id,
                )?;
                let checkpoint = AgentControl::new(self.repository)
                    .create_checkpoint(CheckpointCreation {
                        checkpoint_id: command.checkpoint_id,
                        task_id: command.task_id,
                        execution_id: command.execution_id,
                        workspace_id: command.workspace_id,
                        version_id: command.version_id,
                        operation_id: command.source_operation_id,
                        reason: command.reason_code,
                        actor_agent_id: caller.into(),
                        request_id: request_id.into(),
                        created_at: issued_at.into(),
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Checkpoint(checkpoint.into()))
            }
            ProtocolCall::ResumeFromCheckpoint(command) => {
                let resume = AgentControl::new(self.repository)
                    .resume_from_checkpoint(ResumeCreation {
                        execution_id: command.execution_id,
                        task_id: command.task_id,
                        agent_id: caller.into(),
                        parent_execution_id: command.parent_execution_id,
                        workspace_id: Some(command.workspace_id),
                        source_version_id: None,
                        checkpoint_id: Some(command.checkpoint_id),
                        request_id: request_id.into(),
                        created_at: issued_at.into(),
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Resume(resume.into()))
            }
            ProtocolCall::CreateHandoff(command) => {
                self.require_execution_owner(caller, &command.from_execution_id)?;
                let handoff = AgentControl::new(self.repository)
                    .create_handoff(HandoffCreation {
                        handoff_id: command.handoff_id,
                        task_id: command.task_id,
                        from_execution_id: command.from_execution_id.clone(),
                        to_execution_id: command.to_execution_id,
                        source_version_id: command.source_version_id,
                        checkpoint_id: command.checkpoint_id,
                        reason: command.reason_code,
                        actor_agent_id: caller.into(),
                        requester_execution_id: Some(command.from_execution_id),
                        request_id: request_id.into(),
                        created_at: issued_at.into(),
                    })
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Handoff(handoff.into()))
            }
            ProtocolCall::MaterializeVersion(command) => {
                self.require_execution_workspace(
                    caller,
                    &command.execution_id,
                    &command.workspace_id,
                )?;
                self.require_workspace_revision(
                    &command.workspace_id,
                    command.expected_workspace_revision,
                )?;
                self.require_lease(caller, &command.lease, now_ms)?;
                let snapshot = AgentControl::new(self.repository)
                    .materialize_from_version(
                        &command.workspace_id,
                        &command.source_version_id,
                        &(&command.lease).into(),
                        command.expected_workspace_revision,
                        now_ms,
                        issued_at,
                    )
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Snapshot(snapshot.into()))
            }
            ProtocolCall::StartOperation(command) => {
                let operation_id = required_operation_id(operation_id)?;
                let execution = self.require_execution_owner(caller, &command.execution_id)?;
                if execution.state != "running" {
                    return Err(ProtocolFailure::new(
                        ProtocolErrorCode::InvalidState,
                        "Operation requires a running Execution",
                        false,
                    )
                    .with_entity("execution", &command.execution_id));
                }
                if command.action.trim().is_empty() {
                    return Err(ProtocolFailure::validation(
                        "operation action must not be empty",
                    ));
                }
                let environment_id = match execution.workspace_id.as_deref() {
                    Some(workspace_id) => {
                        AgentControl::new(self.repository)
                            .workspace(workspace_id)
                            .map_err(ProtocolFailure::from_pong)?
                            .ok_or_else(|| ProtocolFailure::not_found("workspace", workspace_id))?
                            .environment_id
                    }
                    None => None,
                };
                let operation = AgentControl::new(self.repository)
                    .start_execution_operation(
                        &command.execution_id,
                        OperationEnvelope {
                            operation_id: operation_id.into(),
                            project_id: execution.project_id,
                            request_id: request_id.into(),
                            agent_id: caller.into(),
                            session_id: format!("execution:{}", command.execution_id),
                            workspace_id: execution.workspace_id,
                            environment_id,
                            parent_operation_id: None,
                            schema_version: OPERATION_SCHEMA_VERSION.into(),
                            started_at: issued_at.into(),
                            tool: "external-agent-runtime".into(),
                            action: command.action,
                            input_refs: Vec::new(),
                            output_refs: Vec::new(),
                            resource: None,
                            before_state: None,
                            after_state: None,
                            reversibility: "UNKNOWN".into(),
                            replayability: "UNKNOWN".into(),
                            side_effect: "EXTERNAL".into(),
                            policy_decision: None,
                        },
                        issued_at,
                    )
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Operation(OperationResource::from_record(
                    operation,
                    Some(command.execution_id),
                )))
            }
            ProtocolCall::FinishOperation(command) => {
                let operation_id = required_operation_id(operation_id)?;
                self.require_execution_owner(caller, &command.execution_id)?;
                let operation = AgentControl::new(self.repository)
                    .operation(operation_id)
                    .map_err(ProtocolFailure::from_pong)?
                    .ok_or_else(|| ProtocolFailure::not_found("operation", operation_id))?;
                if operation.agent_id != caller {
                    return Err(ProtocolFailure::forbidden(
                        "caller does not own the target Operation",
                    )
                    .with_entity("operation", operation_id));
                }
                validate_operation_outcome_command(&command)?;
                let status = command.status.as_str();
                let error = command.failure.map(|failure| OperationError {
                    code: failure.code,
                    message: "external Agent reported a terminal Operation outcome".into(),
                    retryable: failure.retryable,
                    details: None,
                    safe_to_expose: false,
                });
                let result = (status == "completed").then(|| {
                    serde_json::json!({
                        "acknowledgement": "external Agent reported completion"
                    })
                });
                let operation = AgentControl::new(self.repository)
                    .finish_execution_operation(
                        &command.execution_id,
                        operation_id,
                        OperationOutcome {
                            status: status.into(),
                            finished_at: issued_at.into(),
                            output_refs: None,
                            after_state: None,
                            result,
                            error,
                        },
                    )
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Operation(OperationResource::from_record(
                    operation,
                    Some(command.execution_id),
                )))
            }
            ProtocolCall::GetAgent(query) => {
                let agent = AgentControl::new(self.repository)
                    .agent(&query.id)
                    .map_err(ProtocolFailure::from_pong)?
                    .ok_or_else(|| ProtocolFailure::not_found("agent", &query.id))?;
                Ok(ProtocolResult::Agent(agent.into()))
            }
            ProtocolCall::GetTask(query) => {
                let task = AgentControl::new(self.repository)
                    .task(&query.id)
                    .map_err(ProtocolFailure::from_pong)?
                    .ok_or_else(|| ProtocolFailure::not_found("task", &query.id))?;
                Ok(ProtocolResult::Task(task.into()))
            }
            ProtocolCall::GetExecution(query) => {
                let execution = self.require_execution_owner(caller, &query.id)?;
                Ok(ProtocolResult::Execution(execution.into()))
            }
            ProtocolCall::GetWorkspace(query) => {
                let workspace = AgentControl::new(self.repository)
                    .workspace(&query.id)
                    .map_err(ProtocolFailure::from_pong)?
                    .ok_or_else(|| ProtocolFailure::not_found("workspace", &query.id))?;
                let lease = AgentControl::new(self.repository)
                    .lease(&query.id, now_ms)
                    .map_err(ProtocolFailure::from_pong)?
                    .and_then(lease_resource_from_view);
                Ok(ProtocolResult::WorkspaceInspection(WorkspaceInspection {
                    workspace: workspace.into(),
                    lease,
                }))
            }
            ProtocolCall::GetVersion(query) => {
                let version = self
                    .repository
                    .metadata()
                    .version_record(&query.id)
                    .map_err(ProtocolFailure::from_pong)?
                    .ok_or_else(|| ProtocolFailure::not_found("version", &query.id))?;
                Ok(ProtocolResult::Version(version.into()))
            }
            ProtocolCall::GetCheckpoint(query) => {
                let checkpoint = AgentControl::new(self.repository)
                    .checkpoint(&query.id)
                    .map_err(ProtocolFailure::from_pong)?
                    .ok_or_else(|| ProtocolFailure::not_found("checkpoint", &query.id))?;
                Ok(ProtocolResult::Checkpoint(checkpoint.into()))
            }
            ProtocolCall::GetHandoff(query) => {
                let handoff = AgentControl::new(self.repository)
                    .handoff(&query.id)
                    .map_err(ProtocolFailure::from_pong)?
                    .ok_or_else(|| ProtocolFailure::not_found("handoff", &query.id))?;
                Ok(ProtocolResult::Handoff(handoff.into()))
            }
            ProtocolCall::GetOperation(query) => {
                let operation = AgentControl::new(self.repository)
                    .operation(&query.id)
                    .map_err(ProtocolFailure::from_pong)?
                    .ok_or_else(|| ProtocolFailure::not_found("operation", &query.id))?;
                if operation.agent_id != caller {
                    return Err(ProtocolFailure::forbidden(
                        "caller does not own the target Operation",
                    )
                    .with_entity("operation", &query.id));
                }
                let execution_id = AgentControl::new(self.repository)
                    .execution_for_operation(&query.id)
                    .map_err(ProtocolFailure::from_pong)?
                    .map(|association| association.execution_id);
                Ok(ProtocolResult::Operation(OperationResource::from_record(
                    operation,
                    execution_id,
                )))
            }
            ProtocolCall::ResolveOperation(query) => {
                let operation = AgentControl::new(self.repository)
                    .operation_for_request(&query.project_id, caller, &query.request_id)
                    .map_err(ProtocolFailure::from_pong)?
                    .ok_or_else(|| ProtocolFailure::not_found("operation", &query.request_id))?;
                let execution_id = AgentControl::new(self.repository)
                    .execution_for_operation(&operation.operation_id)
                    .map_err(ProtocolFailure::from_pong)?
                    .map(|association| association.execution_id);
                Ok(ProtocolResult::Operation(OperationResource::from_record(
                    operation,
                    execution_id,
                )))
            }
            ProtocolCall::InspectExecution(query) => {
                self.require_execution_owner(caller, &query.id)?;
                let state = AgentControl::new(self.repository)
                    .state(&query.id, now_ms)
                    .map_err(ProtocolFailure::from_pong)?;
                let operations = AgentControl::new(self.repository)
                    .operations_for_execution(&query.id)
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::ExecutionInspection(Box::new(
                    ExecutionInspection::from_state(state, operations),
                )))
            }
            ProtocolCall::ListCheckpoints(query) => {
                let records = AgentControl::new(self.repository)
                    .list_checkpoints(&query.task_id)
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Checkpoints(ResourceList {
                    items: records.into_iter().map(Into::into).collect(),
                }))
            }
            ProtocolCall::ListHandoffs(query) => {
                let records = AgentControl::new(self.repository)
                    .list_handoffs(&query.task_id)
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Handoffs(ResourceList {
                    items: records.into_iter().map(Into::into).collect(),
                }))
            }
            ProtocolCall::DiffWorkspaceVersion(query) => {
                let diff = AgentControl::new(self.repository)
                    .diff_against_version(&query.workspace_id, &query.source_version_id)
                    .map_err(ProtocolFailure::from_pong)?;
                Ok(ProtocolResult::Diff(diff.into()))
            }
            ProtocolCall::Hello | ProtocolCall::RegisterAgent(_) => Err(ProtocolFailure::new(
                ProtocolErrorCode::InternalError,
                "protocol dispatcher reached an invalid state",
                false,
            )),
        }
    }

    fn execution_transition(
        &mut self,
        caller: &str,
        command: ExecutionOutcomeCommand,
        next_state: &str,
        issued_at: &str,
    ) -> Result<ProtocolResult, ProtocolFailure> {
        self.require_execution_transition(
            caller,
            &command.execution_id,
            command.expected_revision,
            next_state,
            command.outcome_code.as_deref(),
        )?;
        let execution = AgentControl::new(self.repository)
            .finish_execution(
                &command.execution_id,
                next_state,
                command.outcome_code.as_deref(),
                command.expected_revision,
                issued_at,
            )
            .map_err(ProtocolFailure::from_pong)?;
        Ok(ProtocolResult::Execution(execution.into()))
    }

    fn require_registered_agent(&mut self, agent_id: &str) -> Result<(), ProtocolFailure> {
        if AgentControl::new(self.repository)
            .agent(agent_id)
            .map_err(ProtocolFailure::from_pong)?
            .is_none()
        {
            return Err(ProtocolFailure::new(
                ProtocolErrorCode::Unauthorized,
                "caller Agent identity is not registered",
                false,
            )
            .with_entity("agent", agent_id));
        }
        Ok(())
    }

    fn require_execution_owner(
        &mut self,
        caller: &str,
        execution_id: &str,
    ) -> Result<ExecutionRecord, ProtocolFailure> {
        let execution = AgentControl::new(self.repository)
            .execution(execution_id)
            .map_err(ProtocolFailure::from_pong)?
            .ok_or_else(|| ProtocolFailure::not_found("execution", execution_id))?;
        if execution.agent_id != caller {
            return Err(
                ProtocolFailure::forbidden("caller does not own the target Execution")
                    .with_entity("execution", execution_id),
            );
        }
        Ok(execution)
    }

    fn require_execution_transition(
        &mut self,
        caller: &str,
        execution_id: &str,
        expected_revision: i64,
        next_state: &str,
        outcome_code: Option<&str>,
    ) -> Result<ExecutionRecord, ProtocolFailure> {
        if expected_revision < 0 {
            return Err(ProtocolFailure::validation(
                "expected Execution revision must not be negative",
            ));
        }
        let execution = self.require_execution_owner(caller, execution_id)?;
        if execution.state == next_state && execution.outcome.as_deref() == outcome_code {
            return Ok(execution);
        }
        if execution.revision != expected_revision {
            return Err(ProtocolFailure::revision(
                "execution",
                execution_id,
                expected_revision,
                execution.revision,
            ));
        }
        if !protocol_execution_transition_allowed(&execution.state, next_state) {
            return Err(ProtocolFailure::new(
                ProtocolErrorCode::InvalidState,
                "Execution lifecycle transition is not allowed",
                false,
            )
            .with_entity("execution", execution_id));
        }
        Ok(execution)
    }

    #[allow(clippy::too_many_arguments)]
    fn require_execution_version_revisions(
        &mut self,
        caller: &str,
        execution_id: &str,
        workspace_id: &str,
        version_id: &str,
        expected_execution_revision: i64,
        expected_workspace_revision: i64,
    ) -> Result<(), ProtocolFailure> {
        if expected_execution_revision < 0 || expected_workspace_revision < 0 {
            return Err(ProtocolFailure::validation(
                "expected revision must not be negative",
            ));
        }
        let execution = self.require_execution_workspace(caller, execution_id, workspace_id)?;
        let workspace = AgentControl::new(self.repository)
            .workspace(workspace_id)
            .map_err(ProtocolFailure::from_pong)?
            .ok_or_else(|| ProtocolFailure::not_found("workspace", workspace_id))?;
        let exact_retry = execution.current_version_id.as_deref() == Some(version_id)
            && execution.revision == expected_execution_revision.saturating_add(1)
            && workspace.revision == expected_workspace_revision.saturating_add(1);
        if exact_retry {
            return Ok(());
        }
        if execution.revision != expected_execution_revision {
            return Err(ProtocolFailure::revision(
                "execution",
                execution_id,
                expected_execution_revision,
                execution.revision,
            ));
        }
        if workspace.revision != expected_workspace_revision {
            return Err(ProtocolFailure::revision(
                "workspace",
                workspace_id,
                expected_workspace_revision,
                workspace.revision,
            ));
        }
        Ok(())
    }

    fn require_execution_workspace(
        &mut self,
        caller: &str,
        execution_id: &str,
        workspace_id: &str,
    ) -> Result<ExecutionRecord, ProtocolFailure> {
        let execution = self.require_execution_owner(caller, execution_id)?;
        if execution.workspace_id.as_deref() != Some(workspace_id) {
            return Err(
                ProtocolFailure::forbidden("Execution does not own the target Workspace")
                    .with_entity("workspace", workspace_id),
            );
        }
        Ok(execution)
    }

    fn require_workspace_revision(
        &mut self,
        workspace_id: &str,
        expected_revision: i64,
    ) -> Result<(), ProtocolFailure> {
        if expected_revision < 0 {
            return Err(ProtocolFailure::validation(
                "expected Workspace revision must not be negative",
            ));
        }
        let workspace = AgentControl::new(self.repository)
            .workspace(workspace_id)
            .map_err(ProtocolFailure::from_pong)?
            .ok_or_else(|| ProtocolFailure::not_found("workspace", workspace_id))?;
        if workspace.revision != expected_revision {
            return Err(ProtocolFailure::revision(
                "workspace",
                workspace_id,
                expected_revision,
                workspace.revision,
            ));
        }
        Ok(())
    }

    fn require_lease(
        &mut self,
        caller: &str,
        authority: &LeaseAuthority,
        now_ms: i64,
    ) -> Result<(), ProtocolFailure> {
        if authority.agent_id != caller {
            return Err(ProtocolFailure::lease(&authority.workspace_id));
        }
        let lease = AgentControl::new(self.repository)
            .lease(&authority.workspace_id, now_ms)
            .map_err(ProtocolFailure::from_pong)?
            .ok_or_else(|| ProtocolFailure::lease(&authority.workspace_id))?;
        if !lease.active
            || lease.agent_id.as_deref() != Some(caller)
            || lease.epoch != authority.epoch
            || lease.expires_at_ms != authority.expires_at_ms
        {
            return Err(ProtocolFailure::lease(&authority.workspace_id));
        }
        Ok(())
    }
}

fn validate_request_header(request: &ProtocolRequest) -> Result<(), ProtocolFailure> {
    for (value, label) in [
        (request.protocol_version.as_str(), "protocol_version"),
        (request.request_id.as_str(), "request_id"),
        (request.issued_at.as_str(), "issued_at"),
    ] {
        if value.trim().is_empty() {
            return Err(ProtocolFailure::validation(&format!(
                "{label} must not be empty"
            )));
        }
    }
    for (value, label) in [
        (request.caller_agent_id.as_deref(), "caller_agent_id"),
        (request.operation_id.as_deref(), "operation_id"),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            return Err(ProtocolFailure::validation(&format!(
                "{label} must not be empty"
            )));
        }
    }
    Ok(())
}

fn required_caller(caller: Option<&str>) -> Result<&str, ProtocolFailure> {
    caller.ok_or_else(|| {
        ProtocolFailure::new(
            ProtocolErrorCode::Unauthorized,
            "caller Agent identity is required",
            false,
        )
    })
}

fn required_operation_id(operation_id: Option<&str>) -> Result<&str, ProtocolFailure> {
    operation_id.ok_or_else(|| ProtocolFailure::validation("operation_id is required"))
}

fn validate_operation_outcome_command(
    command: &FinishOperationCommand,
) -> Result<(), ProtocolFailure> {
    match (&command.status, &command.failure) {
        (OperationTerminalStatus::Completed, None) => Ok(()),
        (OperationTerminalStatus::Completed, Some(_)) => Err(ProtocolFailure::validation(
            "completed Operation must not include failure",
        )),
        (_, Some(failure)) if !failure.code.trim().is_empty() => Ok(()),
        (_, Some(_)) => Err(ProtocolFailure::validation(
            "operation failure code must not be empty",
        )),
        (_, None) => Err(ProtocolFailure::validation(
            "non-completed Operation must include failure",
        )),
    }
}

fn protocol_execution_transition_allowed(current: &str, next: &str) -> bool {
    match current {
        "created" => matches!(
            next,
            "running" | "paused" | "failed" | "interrupted" | "unknown"
        ),
        "running" => matches!(
            next,
            "running" | "paused" | "completed" | "failed" | "interrupted" | "unknown"
        ),
        "paused" => matches!(
            next,
            "paused" | "running" | "completed" | "failed" | "interrupted" | "unknown"
        ),
        "completed" | "failed" | "interrupted" | "unknown" => false,
        _ => false,
    }
}

#[derive(Debug)]
struct ProtocolFailure {
    code: ProtocolErrorCode,
    message: String,
    retryable: bool,
    details: Option<ProtocolErrorDetails>,
}

impl ProtocolFailure {
    fn new(code: ProtocolErrorCode, message: &str, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            details: None,
        }
    }

    fn validation(message: &str) -> Self {
        Self::new(ProtocolErrorCode::ValidationError, message, false)
    }

    fn forbidden(message: &str) -> Self {
        Self::new(ProtocolErrorCode::Forbidden, message, false)
    }

    fn not_found(kind: &str, id: &str) -> Self {
        Self::new(
            ProtocolErrorCode::NotFound,
            "requested entity does not exist",
            false,
        )
        .with_entity(kind, id)
    }

    fn revision(kind: &str, id: &str, expected: i64, actual: i64) -> Self {
        let mut failure = Self::new(
            ProtocolErrorCode::RevisionConflict,
            "expected revision does not match durable state",
            true,
        )
        .with_entity(kind, id);
        if let Some(details) = failure.details.as_mut() {
            details.expected_revision = Some(expected);
            details.actual_revision = Some(actual);
        }
        failure
    }

    fn lease(workspace_id: &str) -> Self {
        Self::new(
            ProtocolErrorCode::LeaseConflict,
            "Workspace lease authority is missing, stale, foreign, or expired",
            true,
        )
        .with_entity("workspace", workspace_id)
    }

    fn with_entity(mut self, kind: &str, id: &str) -> Self {
        self.details = Some(ProtocolErrorDetails {
            entity_kind: Some(kind.into()),
            entity_id: Some(id.into()),
            expected_revision: None,
            actual_revision: None,
        });
        self
    }

    fn from_pong(error: PongError) -> Self {
        match error {
            PongError::InvalidInput(_) => Self::new(
                ProtocolErrorCode::ValidationError,
                "domain request validation failed",
                false,
            ),
            PongError::Integrity(_) => Self::new(
                ProtocolErrorCode::IntegrityError,
                "durable state failed integrity validation",
                false,
            ),
            PongError::NotFound(_) => Self::new(
                ProtocolErrorCode::NotFound,
                "required durable entity does not exist",
                false,
            ),
            PongError::Conflict(_) => Self::new(
                ProtocolErrorCode::Conflict,
                "durable state conflicts with the request",
                true,
            ),
            PongError::IdempotencyKeyReuse(_) => Self::new(
                ProtocolErrorCode::IdempotencyConflict,
                "request identity was reused with different content",
                false,
            ),
            PongError::Unsupported(_) => Self::new(
                ProtocolErrorCode::NotSupported,
                "operation is not supported by this implementation",
                false,
            ),
            PongError::RecoveryRequired(_) => Self::new(
                ProtocolErrorCode::RecoveryRequired,
                "durable state requires explicit recovery",
                true,
            ),
            PongError::ResourceExhausted(_) => Self::new(
                ProtocolErrorCode::ResourceExhausted,
                "a required host resource is exhausted",
                true,
            ),
            PongError::PermissionDenied(_) | PongError::PermissionDeniedWithOsError { .. } => {
                Self::new(
                    ProtocolErrorCode::Forbidden,
                    "operation was denied by the protected storage boundary",
                    false,
                )
            }
            PongError::FaultInjected(_) => Self::new(
                ProtocolErrorCode::RecoveryRequired,
                "operation result is unconfirmed; inspect durable state before retry",
                true,
            ),
            PongError::Io(_) | PongError::Sqlite(_) | PongError::Serialization(_) => Self::new(
                ProtocolErrorCode::InternalError,
                "internal persistence failure",
                true,
            ),
        }
    }

    fn from_workspace_binding(error: WorkspaceBindingError) -> Self {
        match error {
            WorkspaceBindingError::Invalid => {
                Self::validation("workspace binding reference is invalid")
            }
            WorkspaceBindingError::NotFound => Self::new(
                ProtocolErrorCode::NotFound,
                "workspace binding reference does not exist",
                false,
            ),
            WorkspaceBindingError::Forbidden => Self::new(
                ProtocolErrorCode::Forbidden,
                "workspace binding reference is not authorized",
                false,
            ),
            WorkspaceBindingError::Unavailable => Self::new(
                ProtocolErrorCode::InternalError,
                "workspace binding service is unavailable",
                true,
            ),
        }
    }
}

impl From<ProtocolFailure> for ProtocolError {
    fn from(value: ProtocolFailure) -> Self {
        Self {
            code: value.code,
            message: value.message,
            retryable: value.retryable,
            details: value.details,
        }
    }
}
