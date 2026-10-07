//! Provider-neutral completion workflow for the focused `pong` CLI.

use pong_core::bootstrap::{self, BootstrapError, BootstrapResolution};
use pong_core::control::{
    AcquireWorkspaceRequest, AgentControl, PublishVersionRequest, ReleaseWorkspaceRequest,
};
use pong_core::workspace::WorkspaceManager;
use pong_core::{PongError, Repository, RuntimeIdentityAdapter};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize)]
pub struct FinishReport {
    pub status: String,
    pub execution_id: String,
    pub workspace_id: String,
    pub version_id: String,
    pub snapshot_id: String,
    pub root_digest: String,
    pub execution_state: String,
}

struct FinishVersion {
    version_id: String,
    snapshot_id: String,
    root_digest: String,
    workspace_revision: i64,
}

pub fn finish(
    project_root: Option<&Path>,
    execution_id: &str,
    state: &str,
    outcome: Option<&str>,
) -> Result<FinishReport, String> {
    let state = state.trim();
    if !matches!(state, "completed" | "failed" | "interrupted") {
        return Err("finish state must be completed, failed, or interrupted".into());
    }
    let execution_id = execution_id.trim();
    if execution_id.is_empty() {
        return Err("finish requires a non-empty execution id".into());
    }
    let resolution = discover(project_root)?;
    let root = resolution.repository_root;
    let identity_path = root
        .join(".pong")
        .join("runtime-identities")
        .join("local.json");
    let identity = RuntimeIdentityAdapter::open(identity_path, "local").map_err(format_pong)?;
    let mut repository = Repository::open(&root).map_err(format_pong)?;
    let execution = repository
        .metadata()
        .execution(execution_id)
        .map_err(format_pong)?
        .ok_or_else(|| "execution does not exist".to_string())?;
    if execution.agent_id != identity.agent_id() {
        return Err("execution belongs to another Agent identity".into());
    }
    if execution.state != "running" {
        return Err(format!(
            "execution is not running (state: {})",
            execution.state
        ));
    }
    let workspace_id = execution
        .workspace_id
        .clone()
        .ok_or_else(|| "execution has no workspace".to_string())?;
    let workspace = repository
        .metadata()
        .workspace(&workspace_id)
        .map_err(format_pong)?
        .ok_or_else(|| "execution workspace does not exist".to_string())?;
    let now_ms = unix_millis() as i64;
    let now = format!("unix-ms:{now_ms}");
    let workspace_status =
        WorkspaceManager::new(&mut repository, pong_core::redaction::Redactor::default())
            .status(&workspace_id, now_ms)
            .map_err(format_pong)?;
    let existing_version = if workspace_status.changed == Some(false) {
        if let Some(version_id) = workspace.version_head_id.as_deref() {
            let version = repository
                .metadata()
                .version_record(version_id)
                .map_err(format_pong)?
                .ok_or_else(|| "workspace Version Head is missing".to_string())?;
            let snapshot = repository
                .metadata()
                .snapshot_record(&version.snapshot_id)
                .map_err(format_pong)?
                .ok_or_else(|| "workspace Snapshot is missing".to_string())?;
            Some(FinishVersion {
                version_id: version.version_id,
                snapshot_id: snapshot.snapshot_id,
                root_digest: snapshot.root_digest,
                workspace_revision: workspace.revision,
            })
        } else {
            None
        }
    } else {
        None
    };
    let mut control = AgentControl::new(&mut repository);
    let mut lease = None;
    let version = if let Some(existing_version) = existing_version {
        existing_version
    } else {
        let acquired = control
            .acquire_workspace(AcquireWorkspaceRequest {
                workspace_id: workspace.workspace_id.clone(),
                agent_id: identity.agent_id().to_owned(),
                now_ms,
                ttl_ms: 60_000,
            })
            .map_err(format_pong)?;
        let publication = control
            .publish_version(PublishVersionRequest {
                workspace_id: workspace.workspace_id.clone(),
                lease: acquired.clone(),
                expected_workspace_revision: workspace.revision,
                now_ms,
                created_at: now.clone(),
                operation_id: unique_id("operation", execution_id, now_ms),
                request_id: unique_id("request", execution_id, now_ms),
                session_id: None,
                tool: Some("pong finish".into()),
                parent_version_id: workspace.version_head_id.clone(),
                update_version_head: true,
            })
            .map_err(format_pong)?;
        lease = Some(acquired);
        FinishVersion {
            version_id: publication.version.version_id,
            snapshot_id: publication.snapshot.snapshot_id,
            root_digest: publication.snapshot.root_digest,
            workspace_revision: publication.workspace.revision,
        }
    };
    let execution_after_version =
        if execution.current_version_id.as_deref() == Some(version.version_id.as_str()) {
            execution
        } else {
            let active_lease = match lease.as_ref() {
                Some(value) => value,
                None => {
                    let acquired = control
                        .acquire_workspace(AcquireWorkspaceRequest {
                            workspace_id: workspace.workspace_id.clone(),
                            agent_id: identity.agent_id().to_owned(),
                            now_ms,
                            ttl_ms: 60_000,
                        })
                        .map_err(format_pong)?;
                    lease = Some(acquired);
                    lease.as_ref().unwrap()
                }
            };
            control
                .set_execution_current_version(
                    execution_id,
                    &version.version_id,
                    active_lease,
                    execution.revision,
                    version.workspace_revision,
                    &now,
                    now_ms,
                )
                .map_err(format_pong)?
        };
    let finished_execution = control
        .finish_execution(
            execution_id,
            state,
            outcome,
            execution_after_version.revision,
            &now,
        )
        .map_err(format_pong)?;
    if let Some(active_lease) = lease {
        control
            .release_workspace(ReleaseWorkspaceRequest {
                lease: active_lease,
                now_ms,
            })
            .map_err(format_pong)?;
    }

    Ok(FinishReport {
        status: "finished".into(),
        execution_id: finished_execution.execution_id,
        workspace_id,
        version_id: version.version_id,
        snapshot_id: version.snapshot_id,
        root_digest: version.root_digest,
        execution_state: finished_execution.state,
    })
}

fn discover(project_root: Option<&Path>) -> Result<BootstrapResolution, String> {
    match project_root {
        Some(root) => bootstrap::discover(root).map_err(format_bootstrap),
        None => bootstrap::discover_from_cwd().map_err(format_bootstrap),
    }
}

fn unique_id(prefix: &str, execution_id: &str, now: i64) -> String {
    let sequence = ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let material = format!("{prefix}\0{execution_id}\0{now}\0{sequence}");
    let digest = Sha256::digest(material.as_bytes());
    format!("{prefix}:local:{}", hex::encode(digest))
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn format_bootstrap(error: BootstrapError) -> String {
    format!("{} ({})", error, error.code())
}

fn format_pong(error: PongError) -> String {
    format!("{} ({})", error, error.code())
}
