//! Durable project-start workflow for the provider-neutral `pong` CLI.

use pong_core::bootstrap::{self, BootstrapError, BootstrapResolution};
use pong_core::control::{
    AcquireWorkspaceRequest, AgentControl, CreateExecutionRequest, CreateTaskRequest,
    CreateWorkspaceRequest, PublishVersionRequest, RegisterAgentRequest, ReleaseWorkspaceRequest,
};
use pong_core::workspace::LocalWorkspace;
use pong_core::{PongError, Repository, RuntimeIdentityAdapter};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize)]
pub struct StartReport {
    pub status: String,
    pub project_root: String,
    pub project_id: String,
    pub task_id: String,
    pub workspace_id: String,
    pub execution_id: String,
    pub agent_id: String,
    pub workspace_root: String,
    pub initial_version_id: Option<String>,
    pub initial_snapshot_id: Option<String>,
}

pub fn start(
    project_root: Option<&Path>,
    goal: &str,
    from_project: bool,
) -> Result<StartReport, String> {
    let goal = goal.trim();
    if goal.is_empty() {
        return Err("start requires a non-empty task goal".into());
    }
    let resolution = discover(project_root)?;
    let root = resolution.repository_root.clone();
    let project_id = stable_project_id(&root);
    let now = unix_millis();
    let nonce = unix_nanos();
    let task_id = unique_id("task", &root, goal, nonce);
    let workspace_id = unique_id("workspace", &root, goal, nonce);
    let execution_id = unique_id("execution", &root, goal, nonce);
    let workspace_root = workspace_path(&root, &project_id, &workspace_id)?;
    let identity_path = root
        .join(".pong")
        .join("runtime-identities")
        .join("local.json");
    let identity = RuntimeIdentityAdapter::open(identity_path, "local").map_err(format_pong)?;

    let mut repository = Repository::open(&root).map_err(format_pong)?;
    let environment_id = stable_environment_id(&root);
    repository
        .metadata_mut()
        .record_environment(
            &environment_id,
            &project_id,
            &json!({
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "family": std::env::consts::FAMILY,
            }),
            &format!("unix-ms:{now}"),
        )
        .map_err(format_pong)?;
    let mut control = AgentControl::new(&mut repository);
    control
        .register_agent(RegisterAgentRequest {
            agent_id: identity.agent_id().to_owned(),
            provider: identity.provider_metadata().to_owned(),
            display_name: Some("local runtime".into()),
            created_at: identity.metadata().created_at.clone(),
        })
        .map_err(format_pong)?;
    control
        .create_task(CreateTaskRequest {
            task_id: task_id.clone(),
            project_id: project_id.clone(),
            goal: goal.to_owned(),
            context_ref: None,
            created_at: format!("unix-ms:{now}"),
        })
        .map_err(format_pong)?;
    control
        .create_workspace(CreateWorkspaceRequest {
            workspace_id: workspace_id.clone(),
            project_id: project_id.clone(),
            path: workspace_root.clone(),
            branch_ref: None,
            environment_id: Some(environment_id),
            now: format!("unix-ms:{now}"),
        })
        .map_err(format_pong)?;
    if from_project {
        let local = LocalWorkspace::open(
            &workspace_id,
            &project_id,
            &workspace_root,
            pong_core::redaction::Redactor::default(),
        )
        .map_err(format_pong)?;
        local
            .copy_from_directory(&root, ".pong")
            .map_err(format_pong)?;
    }
    let execution = control
        .create_execution(CreateExecutionRequest {
            execution_id: execution_id.clone(),
            task_id: task_id.clone(),
            agent_id: identity.agent_id().to_owned(),
            parent_execution_id: None,
            workspace_id: Some(workspace_id.clone()),
            base_version_id: None,
            current_version_id: None,
            created_at: format!("unix-ms:{now}"),
        })
        .map_err(format_pong)?;
    let execution = control
        .start_execution(
            &execution.execution_id,
            execution.revision,
            &format!("unix-ms:{now}"),
        )
        .map_err(format_pong)?;

    let (initial_version_id, initial_snapshot_id) = if from_project {
        let now_ms = unix_millis() as i64;
        let lease = control
            .acquire_workspace(AcquireWorkspaceRequest {
                workspace_id: workspace_id.clone(),
                agent_id: identity.agent_id().to_owned(),
                now_ms,
                ttl_ms: 60_000,
            })
            .map_err(format_pong)?;
        let publication = control
            .publish_version(PublishVersionRequest {
                workspace_id: workspace_id.clone(),
                lease: lease.clone(),
                expected_workspace_revision: 0,
                now_ms,
                created_at: format!("unix-ms:{now_ms}"),
                operation_id: unique_id("operation", &root, goal, unix_nanos()),
                request_id: unique_id("request", &root, goal, unix_nanos()),
                session_id: None,
                tool: Some("pong start --from-project".into()),
                parent_version_id: None,
                update_version_head: true,
            })
            .map_err(format_pong)?;
        let updated = control
            .set_execution_current_version(
                &execution.execution_id,
                &publication.version.version_id,
                &lease,
                execution.revision,
                publication.workspace.revision,
                &format!("unix-ms:{now_ms}"),
                now_ms,
            )
            .map_err(format_pong)?;
        let _ = updated;
        control
            .release_workspace(ReleaseWorkspaceRequest {
                lease,
                now_ms: unix_millis() as i64,
            })
            .map_err(format_pong)?;
        (
            Some(publication.version.version_id),
            Some(publication.snapshot.snapshot_id),
        )
    } else {
        (None, None)
    };

    Ok(StartReport {
        status: "started".into(),
        project_root: resolution.project_root.display().to_string(),
        project_id,
        task_id,
        workspace_id,
        execution_id,
        agent_id: identity.agent_id().to_owned(),
        workspace_root: workspace_root.display().to_string(),
        initial_version_id,
        initial_snapshot_id,
    })
}

fn discover(project_root: Option<&Path>) -> Result<BootstrapResolution, String> {
    match project_root {
        Some(root) => bootstrap::discover(root).map_err(format_bootstrap),
        None => bootstrap::discover_from_cwd().map_err(format_bootstrap),
    }
}

fn workspace_path(root: &Path, project_id: &str, workspace_id: &str) -> Result<PathBuf, String> {
    let parent = root
        .parent()
        .ok_or_else(|| "project root has no parent directory".to_string())?;
    let project_dir = parent
        .join(".pong-workspaces")
        .join(project_id.replace(':', "-"));
    fs::create_dir_all(&project_dir).map_err(|error| error.to_string())?;
    Ok(project_dir.join(workspace_id.replace(':', "-")))
}

fn stable_project_id(root: &Path) -> String {
    let digest = Sha256::digest(root.to_string_lossy().as_bytes());
    format!("project:local:{}", hex::encode(digest))
}

fn stable_environment_id(root: &Path) -> String {
    let material = format!(
        "{}\0{}\0{}",
        root.display(),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let digest = Sha256::digest(material.as_bytes());
    format!("environment:local:{}", hex::encode(digest))
}

fn unique_id(prefix: &str, root: &Path, goal: &str, now: u128) -> String {
    let sequence = ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let material = format!(
        "{prefix}\0{}\0{}\0{}\0{}\0{}",
        root.display(),
        goal,
        now,
        std::process::id(),
        sequence
    );
    let digest = Sha256::digest(material.as_bytes());
    format!("{prefix}:local:{}", hex::encode(digest))
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn format_bootstrap(error: BootstrapError) -> String {
    format!("{} ({})", error, error.code())
}

fn format_pong(error: PongError) -> String {
    format!("{} ({})", error, error.code())
}
