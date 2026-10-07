//! Read-only project status projection for the `pong` CLI.

use pong_core::bootstrap::{self, BootstrapError, BootstrapResolution};
use pong_core::metadata::{CheckpointRecord, ExecutionRecord, TaskRecord, WorkspaceRecord};
use pong_core::{PongError, Repository};
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub project_root: String,
    pub initialized: bool,
    pub repository: Option<RepositoryStatus>,
    pub projects: Vec<ProjectStatus>,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RepositoryStatus {
    pub root: String,
    pub format: String,
    pub schema: String,
    pub storage: String,
    pub active_generation: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectStatus {
    pub project_id: String,
    pub workspaces: Vec<WorkspaceStatus>,
    pub tasks: Vec<TaskStatus>,
    pub active_executions: Vec<ExecutionStatus>,
    pub latest_checkpoint: Option<CheckpointStatus>,
    pub resumable_checkpoints: Vec<CheckpointStatus>,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceStatus {
    pub id: String,
    pub status: String,
    pub head: Option<String>,
    pub version_head_id: Option<String>,
    pub current_version_id: Option<String>,
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskStatus {
    pub id: String,
    pub state: String,
    pub goal: String,
}

#[derive(Debug, Serialize)]
pub struct ExecutionStatus {
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
    pub state: String,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckpointStatus {
    pub id: String,
    pub task_id: String,
    pub execution_id: String,
    pub workspace_id: String,
    pub version_id: String,
    pub reason: String,
    pub created_at: String,
    pub resumable: bool,
}

pub fn initialize(project_root: &Path) -> Result<BootstrapResolution, BootstrapError> {
    bootstrap::initialize(project_root)
}

pub fn inspect(project_root: Option<&Path>) -> Result<StatusReport, String> {
    let (resolution, uninitialized_root) = match project_root {
        Some(root) if !root.join(".pong").is_dir() => {
            let root = canonical_directory(root)?;
            return Ok(uninitialized(root));
        }
        Some(root) => (bootstrap::discover(root).map_err(format_bootstrap)?, None),
        None => match bootstrap::discover_from_cwd() {
            Ok(resolution) => (resolution, None),
            Err(BootstrapError::PongDirectoryMissing(_)) => {
                let root = canonical_directory(&env::current_dir().map_err(|e| e.to_string())?)?;
                return Ok(uninitialized(root));
            }
            Err(error) => return Err(format_bootstrap(error)),
        },
    };

    if let Some(root) = uninitialized_root {
        return Ok(uninitialized(root));
    }

    let repository = Repository::open(&resolution.repository_root).map_err(format_pong)?;
    let metadata = repository.metadata();
    let project_ids = metadata.list_project_ids().map_err(format_pong)?;
    let mut projects = Vec::with_capacity(project_ids.len());

    for project_id in project_ids {
        let workspaces = metadata
            .list_workspaces(&project_id)
            .map_err(format_pong)?
            .into_iter()
            .map(|workspace| workspace_status(metadata, workspace))
            .collect::<Result<Vec<_>, _>>()?;
        let tasks = metadata.list_tasks(&project_id).map_err(format_pong)?;
        let mut active_executions = Vec::new();
        let mut checkpoints = Vec::new();
        for task in &tasks {
            for execution in metadata
                .list_executions(&task.task_id)
                .map_err(format_pong)?
            {
                if is_active_execution(&execution) {
                    active_executions.push(execution_status(execution));
                }
            }
            for checkpoint in metadata
                .list_checkpoints(&task.task_id)
                .map_err(format_pong)?
            {
                checkpoints.push(checkpoint_status(metadata, checkpoint)?);
            }
        }
        checkpoints.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        let latest_checkpoint = checkpoints.last().cloned();
        let resumable_checkpoints = checkpoints
            .iter()
            .filter(|checkpoint| checkpoint.resumable)
            .cloned()
            .collect();
        projects.push(ProjectStatus {
            project_id,
            workspaces,
            tasks: tasks.into_iter().map(task_status).collect(),
            active_executions,
            latest_checkpoint,
            resumable_checkpoints,
        });
    }

    Ok(StatusReport {
        project_root: resolution.project_root.display().to_string(),
        initialized: true,
        repository: Some(RepositoryStatus {
            root: resolution.repository_root.display().to_string(),
            format: repository.marker().repository_format.clone(),
            schema: repository.marker().schema_version.clone(),
            storage: repository.marker().storage_driver.clone(),
            active_generation: repository.active_generation_id().map(str::to_owned),
        }),
        projects,
        note: None,
    })
}

fn uninitialized(root: PathBuf) -> StatusReport {
    StatusReport {
        project_root: root.display().to_string(),
        initialized: false,
        repository: None,
        projects: Vec::new(),
        note: Some("Pong is not initialized in this project root".into()),
    }
}

fn workspace_status(
    metadata: &pong_core::metadata::MetadataStore,
    workspace: WorkspaceRecord,
) -> Result<WorkspaceStatus, String> {
    let current_version_id = workspace.version_head_id.clone();
    let snapshot_id = current_version_id
        .as_deref()
        .map(|version_id| {
            metadata
                .version_record(version_id)
                .map_err(format_pong)
                .and_then(|version| {
                    version
                        .map(|value| value.snapshot_id)
                        .ok_or_else(|| "workspace version head is unavailable".into())
                })
        })
        .transpose()?;
    Ok(WorkspaceStatus {
        id: workspace.workspace_id,
        status: workspace.status,
        head: workspace.head,
        version_head_id: workspace.version_head_id,
        current_version_id,
        snapshot_id,
    })
}

fn task_status(task: TaskRecord) -> TaskStatus {
    TaskStatus {
        id: task.task_id,
        state: task.state,
        goal: task.goal,
    }
}

fn execution_status(execution: ExecutionRecord) -> ExecutionStatus {
    ExecutionStatus {
        id: execution.execution_id,
        task_id: execution.task_id,
        agent_id: execution.agent_id,
        state: execution.state,
        workspace_id: execution.workspace_id,
    }
}

fn checkpoint_status(
    metadata: &pong_core::metadata::MetadataStore,
    checkpoint: CheckpointRecord,
) -> Result<CheckpointStatus, String> {
    let task = metadata.task(&checkpoint.task_id).map_err(format_pong)?;
    let execution = metadata
        .execution(&checkpoint.execution_id)
        .map_err(format_pong)?;
    let workspace = metadata
        .workspace(&checkpoint.workspace_id)
        .map_err(format_pong)?;
    let version = metadata
        .version_record(&checkpoint.version_id)
        .map_err(format_pong)?;
    let snapshot = match version.as_ref() {
        Some(version) => metadata
            .snapshot_record(&version.snapshot_id)
            .map_err(format_pong)?,
        None => None,
    };
    let resumable = matches!((task, execution, workspace, version, snapshot), (Some(task), Some(execution), Some(workspace), Some(version), Some(snapshot))
        if execution.task_id == task.task_id
            && execution.workspace_id.as_deref() == Some(checkpoint.workspace_id.as_str())
            && workspace.project_id == task.project_id
            && version.workspace_id == checkpoint.workspace_id
            && version.project_id == task.project_id
            && snapshot.snapshot_id == version.snapshot_id
            && snapshot.workspace_id == checkpoint.workspace_id
            && snapshot.project_id == task.project_id);
    Ok(CheckpointStatus {
        id: checkpoint.checkpoint_id,
        task_id: checkpoint.task_id,
        execution_id: checkpoint.execution_id,
        workspace_id: checkpoint.workspace_id,
        version_id: checkpoint.version_id,
        reason: checkpoint.reason,
        created_at: checkpoint.created_at,
        resumable,
    })
}

fn is_active_execution(execution: &ExecutionRecord) -> bool {
    matches!(
        execution.state.as_str(),
        "created" | "running" | "paused" | "unknown"
    )
}

fn canonical_directory(path: &Path) -> Result<PathBuf, String> {
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    if !canonical.is_dir() {
        return Err(format!(
            "project root is not a directory: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

fn format_bootstrap(error: BootstrapError) -> String {
    format!("{} ({})", error, error.code())
}

fn format_pong(error: PongError) -> String {
    format!("{} ({})", error, error.code())
}
