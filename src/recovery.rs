//! Provider-neutral recovery inspection and resume projections.

use crate::control::AgentControl;
use crate::metadata::{
    CheckpointRecord, ExecutionRecord, ResumeRecord, SnapshotRecord, TaskRecord, VersionRecord,
    WorkspaceRecord,
};
use crate::{PongError, Repository};
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static RECOVERY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryInspection {
    pub checkpoint: CheckpointSummary,
    pub task: TaskSummary,
    pub execution: ExecutionSummary,
    pub workspace: WorkspaceSummary,
    pub version: VersionSummary,
    pub snapshot: SnapshotSummary,
    pub resume: ResumeSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryResume {
    pub checkpoint_id: String,
    pub original_execution_id: String,
    pub resumed_execution_id: String,
    pub task_id: String,
    pub workspace_id: String,
    pub version_id: String,
    pub snapshot_id: String,
    pub resume_status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckpointSummary {
    pub id: String,
    pub status: &'static str,
    pub task_id: String,
    pub execution_id: String,
    pub workspace_id: String,
    pub version_id: String,
    pub operation_id: Option<String>,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskSummary {
    pub id: String,
    pub status: String,
    pub project_id: String,
    pub goal: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionSummary {
    pub id: String,
    pub status: String,
    pub agent_id: String,
    pub workspace_id: Option<String>,
    pub current_version_id: Option<String>,
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceSummary {
    pub id: String,
    pub status: String,
    pub locator: String,
    pub head: Option<String>,
    pub version_head_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionSummary {
    pub id: String,
    pub workspace_id: String,
    pub snapshot_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotSummary {
    pub id: String,
    pub root_digest: String,
    pub workspace_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResumeSummary {
    pub available: bool,
    pub attempts: Vec<ResumeRecord>,
}

pub fn inspect(
    repository_root: &Path,
    checkpoint_id: &str,
) -> Result<RecoveryInspection, PongError> {
    let repository = Repository::open(repository_root)?;
    let metadata = repository.metadata();
    let checkpoint = metadata
        .checkpoint(checkpoint_id)?
        .ok_or_else(|| PongError::NotFound("checkpoint does not exist".into()))?;
    inspection_from_records(metadata, checkpoint)
}

pub fn resume(
    repository_root: &Path,
    checkpoint_id: &str,
    agent_id: Option<&str>,
) -> Result<RecoveryResume, PongError> {
    let mut repository = Repository::open(repository_root)?;
    let checkpoint = repository
        .metadata()
        .checkpoint(checkpoint_id)?
        .ok_or_else(|| PongError::NotFound("checkpoint does not exist".into()))?;
    let task = repository
        .metadata()
        .task(&checkpoint.task_id)?
        .ok_or_else(|| PongError::Integrity("checkpoint task is missing".into()))?;
    let agent_id = agent_id.unwrap_or(&checkpoint.actor_agent_id).to_owned();
    let token = unique_token();
    let execution_id = format!("recovery:execution:{token}");
    let request_id = format!("recovery:resume:{token}");
    let resumed = AgentControl::new(&mut repository).resume_from_checkpoint(
        crate::metadata::ResumeCreation {
            execution_id: execution_id.clone(),
            task_id: task.task_id.clone(),
            agent_id,
            parent_execution_id: Some(checkpoint.execution_id.clone()),
            workspace_id: Some(checkpoint.workspace_id.clone()),
            source_version_id: None,
            checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
            request_id,
            created_at: format!("unix-ms:{}", unix_millis()),
        },
    )?;
    let version = repository
        .metadata()
        .version_record(&resumed.source_version_id)?
        .ok_or_else(|| PongError::Integrity("resume Version is missing".into()))?;
    let snapshot = repository
        .metadata()
        .snapshot_record(&version.snapshot_id)?
        .ok_or_else(|| PongError::Integrity("resume Snapshot is missing".into()))?;
    Ok(RecoveryResume {
        checkpoint_id: checkpoint.checkpoint_id,
        original_execution_id: checkpoint.execution_id,
        resumed_execution_id: resumed.execution_id,
        task_id: resumed.task_id,
        workspace_id: version.workspace_id,
        version_id: version.version_id,
        snapshot_id: snapshot.snapshot_id,
        resume_status: "created",
    })
}

fn inspection_from_records(
    metadata: &crate::metadata::MetadataStore,
    checkpoint: CheckpointRecord,
) -> Result<RecoveryInspection, PongError> {
    let task = metadata
        .task(&checkpoint.task_id)?
        .ok_or_else(|| PongError::Integrity("checkpoint task is missing".into()))?;
    let execution = metadata
        .execution(&checkpoint.execution_id)?
        .ok_or_else(|| PongError::Integrity("checkpoint execution is missing".into()))?;
    let workspace = metadata
        .workspace(&checkpoint.workspace_id)?
        .ok_or_else(|| PongError::Integrity("checkpoint workspace is missing".into()))?;
    let version = metadata
        .version_record(&checkpoint.version_id)?
        .ok_or_else(|| PongError::Integrity("checkpoint Version is missing".into()))?;
    let snapshot = metadata
        .snapshot_record(&version.snapshot_id)?
        .ok_or_else(|| PongError::Integrity("checkpoint Snapshot is missing".into()))?;
    let attempts = metadata.list_resume_attempts(&task.task_id)?;
    Ok(RecoveryInspection {
        checkpoint: checkpoint_summary(checkpoint),
        task: task_summary(task),
        execution: execution_summary(execution),
        workspace: workspace_summary(workspace),
        version: version_summary(version),
        snapshot: snapshot_summary(snapshot),
        resume: ResumeSummary {
            available: true,
            attempts,
        },
    })
}

fn checkpoint_summary(value: CheckpointRecord) -> CheckpointSummary {
    CheckpointSummary {
        id: value.checkpoint_id,
        status: "available",
        task_id: value.task_id,
        execution_id: value.execution_id,
        workspace_id: value.workspace_id,
        version_id: value.version_id,
        operation_id: value.operation_id,
        reason: value.reason,
        created_at: value.created_at,
    }
}

fn task_summary(value: TaskRecord) -> TaskSummary {
    TaskSummary {
        id: value.task_id,
        status: value.state,
        project_id: value.project_id,
        goal: value.goal,
    }
}

fn execution_summary(value: ExecutionRecord) -> ExecutionSummary {
    ExecutionSummary {
        id: value.execution_id,
        status: value.state,
        agent_id: value.agent_id,
        workspace_id: value.workspace_id,
        current_version_id: value.current_version_id,
        outcome: value.outcome,
    }
}

fn workspace_summary(value: WorkspaceRecord) -> WorkspaceSummary {
    WorkspaceSummary {
        id: value.workspace_id,
        status: value.status,
        locator: value.locator,
        head: value.head,
        version_head_id: value.version_head_id,
    }
}

fn version_summary(value: VersionRecord) -> VersionSummary {
    VersionSummary {
        id: value.version_id,
        workspace_id: value.workspace_id,
        snapshot_id: value.snapshot_id,
        created_at: value.created_at,
    }
}

fn snapshot_summary(value: SnapshotRecord) -> SnapshotSummary {
    SnapshotSummary {
        id: value.snapshot_id,
        root_digest: value.root_digest,
        workspace_id: value.workspace_id,
        created_at: value.created_at,
    }
}

pub fn as_json<T: Serialize>(value: &T) -> Result<String, PongError> {
    serde_json::to_string_pretty(value).map_err(|error| PongError::Serialization(error.to_string()))
}

pub fn as_human_inspection(value: &RecoveryInspection) -> String {
    format!(
        "Checkpoint: {}\nCheckpoint Status: {}\nTask: {}\nTask Status: {}\nExecution: {}\nExecution Status: {}\nWorkspace: {}\nWorkspace Status: {}\nWorkspace Locator: {}\nSnapshot: {}\nVersion: {}\nResume Available: {}\nResume Attempts: {}",
        value.checkpoint.id, value.checkpoint.status, value.task.id, value.task.status,
        value.execution.id, value.execution.status, value.workspace.id, value.workspace.status,
        value.workspace.locator, value.snapshot.id, value.version.id,
        value.resume.available, value.resume.attempts.len()
    )
}

pub fn as_human_resume(value: &RecoveryResume) -> String {
    format!(
        "Checkpoint: {}\nOriginal Execution: {}\nResumed Execution: {}\nTask: {}\nWorkspace: {}\nSnapshot: {}\nVersion: {}\nResume Status: {}",
        value.checkpoint_id, value.original_execution_id, value.resumed_execution_id,
        value.task_id, value.workspace_id, value.snapshot_id, value.version_id,
        value.resume_status
    )
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default()
}

fn unique_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let sequence = RECOVERY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", nanos, std::process::id(), sequence)
}
