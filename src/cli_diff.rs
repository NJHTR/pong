//! Read-only Workspace diff workflow for the focused `pong` CLI.

use pong_core::bootstrap::{self, BootstrapError, BootstrapResolution};
use pong_core::workspace::{SnapshotChangeType, WorkspaceManager};
use pong_core::{PongError, Repository};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct DiffReport {
    pub status: String,
    pub execution_id: String,
    pub workspace_id: String,
    pub reference_snapshot_id: String,
    pub observation_revision: i64,
    pub observation_stability: String,
    pub current_tree_id: String,
    pub changes: Vec<DiffChange>,
}

#[derive(Debug, Serialize)]
pub struct DiffChange {
    pub path: String,
    pub change_type: SnapshotChangeType,
    pub old_digest: Option<String>,
    pub new_digest: Option<String>,
    pub old_size: Option<u64>,
    pub new_size: Option<u64>,
    pub old_type: Option<String>,
    pub new_type: Option<String>,
}

pub fn diff(project_root: Option<&Path>, execution_id: &str) -> Result<DiffReport, String> {
    let execution_id = execution_id.trim();
    if execution_id.is_empty() {
        return Err("diff requires a non-empty execution id".into());
    }
    let resolution = discover(project_root)?;
    let mut repository = Repository::open(&resolution.repository_root).map_err(format_pong)?;
    let execution = repository
        .metadata()
        .execution(execution_id)
        .map_err(format_pong)?
        .ok_or_else(|| "execution does not exist".to_string())?;
    let workspace_id = execution
        .workspace_id
        .ok_or_else(|| "execution has no workspace".to_string())?;
    let result = WorkspaceManager::new(&mut repository, pong_core::redaction::Redactor::default())
        .diff_workspace(&workspace_id)
        .map_err(format_pong)?;
    let changes = result
        .diff
        .entries
        .into_iter()
        .map(|entry| DiffChange {
            path: entry.path,
            change_type: entry.change_type,
            old_digest: entry.old_digest,
            new_digest: entry.new_digest,
            old_size: entry.old_size,
            new_size: entry.new_size,
            old_type: entry.old_type,
            new_type: entry.new_type,
        })
        .collect::<Vec<_>>();
    Ok(DiffReport {
        status: if changes.is_empty() {
            "clean".into()
        } else {
            "changed".into()
        },
        execution_id: execution_id.into(),
        workspace_id: result.workspace_id,
        reference_snapshot_id: result.reference_snapshot_id,
        observation_revision: result.observation_revision,
        observation_stability: result.observation_stability,
        current_tree_id: result.current_tree_id,
        changes,
    })
}

fn discover(project_root: Option<&Path>) -> Result<BootstrapResolution, String> {
    match project_root {
        Some(root) => bootstrap::discover(root).map_err(format_bootstrap),
        None => bootstrap::discover_from_cwd().map_err(format_bootstrap),
    }
}

fn format_bootstrap(error: BootstrapError) -> String {
    format!("{} ({})", error, error.code())
}

fn format_pong(error: PongError) -> String {
    format!("{} ({})", error, error.code())
}
