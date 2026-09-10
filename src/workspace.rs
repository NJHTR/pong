//! Provider-neutral workspace handles and the bounded local filesystem driver.
//!
//! A workspace ID is logical metadata. `LocalWorkspace` owns only a checked
//! physical materialization and never treats its path as identity. Snapshots
//! are immutable CAS blobs plus a canonical tree manifest; materialization
//! always builds a new directory so a partial write cannot masquerade as a
//! complete snapshot.

use crate::atomic_replace::{rename_new_with_retry, sync_directory};
use crate::canonical::canonical_bytes;
use crate::cas::{digest_for, Cas, Digest};
use crate::error::PongError;
use crate::metadata::{
    LeaseRecord, LeaseToken, NewEventEnvelope, OperationEnvelope, OperationError, OperationOutcome,
    OperationRef, PublishedRollbackCompletionInput, RollbackCompletionInput, RollbackCreation,
    RollbackRecord, SnapshotPublication, WorkspaceLifecycleOperationInput, WorkspaceRecord,
    WorkspaceUpdate,
};
use crate::redaction::Redactor;
use crate::repository::Repository;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

const TREE_MANIFEST_VERSION: u32 = 1;
const BLOB_DOMAIN: &str = "workspace/blob/v1";
const TREE_DOMAIN: &str = "workspace/tree/v1";
const DEFAULT_MAX_FILES: usize = 100_000;
const DEFAULT_MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// Durable boundaries at which the local workspace test harness may inject a
/// deterministic filesystem failure.
///
/// The schedule is owned by one [`LocalWorkspace`] or
/// [`WorkspaceManager`] instance. Production constructors install the
/// disabled schedule, so no environment variable or process-global switch
/// can alter normal materialization behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceFailPoint {
    /// Before writing bytes to a newly-created materialization file.
    MaterializeFileWrite,
    /// Before syncing a newly-created materialization file.
    MaterializeFileSync,
    /// Before syncing a directory in the completed temporary materialization.
    MaterializeTreeDirectorySync,
    /// Immediately before the no-replace temporary-directory rename.
    MaterializeRename,
    /// After publication, before syncing the destination parent directory.
    MaterializeParentDirectorySync,
    /// Before removing an abandoned temporary materialization directory.
    CleanupTemporary,
}

impl WorkspaceFailPoint {
    fn label(self) -> &'static str {
        match self {
            Self::MaterializeFileWrite => "workspace_materialize_file_write",
            Self::MaterializeFileSync => "workspace_materialize_file_sync",
            Self::MaterializeTreeDirectorySync => "workspace_materialize_tree_directory_sync",
            Self::MaterializeRename => "workspace_materialize_rename",
            Self::MaterializeParentDirectorySync => "workspace_materialize_parent_directory_sync",
            Self::CleanupTemporary => "workspace_cleanup_temporary",
        }
    }
}

/// Result of a deterministic workspace fault-injection decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceFaultAction {
    /// Continue with the real filesystem operation.
    Continue,
    /// Return an injected fault without performing the operation.
    Fail,
    /// Return a deterministic permission-denied fault without touching ACLs.
    PermissionDenied,
    /// Return a deterministic resource-exhausted fault without touching host
    /// quotas.
    ResourceExhausted,
    /// Write a prefix at the file-write boundary, then report a fault.
    ShortWrite(usize),
}

/// One-shot fault configuration owned by one workspace/materialization
/// instance. The default configuration is disabled.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct WorkspaceFailpoints {
    armed: Option<(WorkspaceFailPoint, WorkspaceFaultAction)>,
}

impl WorkspaceFailpoints {
    pub const fn disabled() -> Self {
        Self { armed: None }
    }

    pub const fn once(point: WorkspaceFailPoint, action: WorkspaceFaultAction) -> Self {
        Self {
            armed: Some((point, action)),
        }
    }

    pub fn arm(&mut self, point: WorkspaceFailPoint, action: WorkspaceFaultAction) {
        self.armed = Some((point, action));
    }

    pub fn disarm(&mut self) {
        self.armed = None;
    }

    pub const fn armed(&self) -> Option<(WorkspaceFailPoint, WorkspaceFaultAction)> {
        self.armed
    }

    fn take_if(&mut self, point: WorkspaceFailPoint) -> Option<WorkspaceFaultAction> {
        if self.armed.map(|(armed, _)| armed) == Some(point) {
            self.armed.take().map(|(_, action)| action)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotOptions {
    pub max_files: usize,
    pub max_file_bytes: u64,
}

/// Inputs for one durable snapshot restore request. The request and operation
/// identities are caller-owned so retries can reuse the same durable intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreOptions {
    pub operation_id: String,
    pub request_id: String,
    pub agent_id: String,
    pub now: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLifecycleOperationOptions {
    pub operation_id: String,
    pub request_id: String,
    pub now_ms: i64,
    pub updated_at: String,
}

/// Durable result of a restore attempt. `status` is one of `completed` or
/// `unknown`; failed restores are returned as errors while their terminal
/// operation record remains queryable in metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreResult {
    pub operation_id: String,
    pub snapshot_id: String,
    pub destination: String,
    pub status: String,
}

/// Read-only summary of the durable lease row. An expired or released row is
/// retained as an inactive epoch tombstone so status never treats a stale
/// token as current.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceLeaseStatus {
    pub epoch: Option<i64>,
    pub agent_id: Option<String>,
    pub expires_at_ms: Option<i64>,
    pub active: bool,
}

/// Read-only summary of the workspace's environment binding. Facts are kept
/// in metadata and are intentionally not copied into this view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEnvironmentStatus {
    pub environment_id: Option<String>,
    pub status: String,
}

/// Read-only summary of the latest durable operation attached to a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceOperationSummary {
    pub operation_id: String,
    pub action: String,
    pub lifecycle_status: String,
    pub recording_status: String,
    pub updated_at: String,
}

/// Stable internal status view for one point-in-time local workspace query.
/// Authoritative metadata is separated from derived filesystem/recovery state;
/// the query never acquires or mutates a lease or workspace revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceStatus {
    pub workspace_id: String,
    pub project_id: String,
    pub driver: String,
    pub revision: i64,
    pub status: String,
    pub head_digest: Option<String>,
    pub head_snapshot_id: Option<String>,
    pub lease: WorkspaceLeaseStatus,
    pub environment: WorkspaceEnvironmentStatus,
    pub filesystem_accessible: bool,
    pub changed: Option<bool>,
    pub change_state: String,
    pub healthy: bool,
    pub execution_ready: bool,
    pub recovery_required: bool,
    pub latest_operation: Option<WorkspaceOperationSummary>,
}

/// The durable lifecycle vocabulary currently supported by the M2 workspace
/// metadata model.  These values intentionally mirror the existing SQLite
/// status strings; adding a new state requires a separate M2 ADR and schema
/// compatibility review.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkspaceLifecycleState {
    Created,
    Preparing,
    Ready,
    Active,
    Paused,
    Reconciling,
    Archived,
}

impl WorkspaceLifecycleState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Preparing => "preparing",
            Self::Ready => "ready",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Reconciling => "reconciling",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PongError> {
        match value {
            "created" => Ok(Self::Created),
            "preparing" => Ok(Self::Preparing),
            "ready" => Ok(Self::Ready),
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "reconciling" => Ok(Self::Reconciling),
            "archived" => Ok(Self::Archived),
            _ => Err(PongError::InvalidInput(
                "workspace lifecycle state is unsupported".into(),
            )),
        }
    }
}

/// Provider-neutral lifecycle actions. `Open` is a handle acquisition and is
/// therefore idempotent without changing the durable status. `Close` archives
/// the logical workspace; physical materialization cleanup remains provider
/// responsibility and is deliberately not implied by this action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkspaceLifecycleAction {
    Open,
    Close,
    BeginCapture,
    CompleteCapture,
    Activate,
    Pause,
    Resume,
    BeginRecovery,
    CompleteRecovery,
}

/// The only capabilities negotiated by the M2 contract.  Versioning,
/// branching, merging, and commit semantics are intentionally absent.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceCapabilities {
    pub snapshot: bool,
    pub restore: bool,
    pub diff: bool,
    pub status: bool,
}

impl WorkspaceCapabilities {
    pub const fn local() -> Self {
        Self {
            snapshot: true,
            restore: true,
            diff: true,
            status: true,
        }
    }

    pub const fn supports(self, capability: WorkspaceCapability) -> bool {
        match capability {
            WorkspaceCapability::Snapshot => self.snapshot,
            WorkspaceCapability::Restore => self.restore,
            WorkspaceCapability::Diff => self.diff,
            WorkspaceCapability::Status => self.status,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceCapability {
    Snapshot,
    Restore,
    Diff,
    Status,
}

/// Logical identity and durable facts passed across the provider boundary.
/// No filesystem path, provider metadata, or platform-specific handle is
/// included in this context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceIdentity {
    pub workspace_id: String,
    pub project_id: String,
    pub provider: String,
}

/// Read-only context a provider may receive for one lifecycle action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceProviderContext {
    pub identity: WorkspaceIdentity,
    pub state: WorkspaceLifecycleState,
    pub revision: i64,
    pub capabilities: WorkspaceCapabilities,
}

/// Validate one lifecycle transition without touching a provider or durable
/// storage.  Persistence and provider side effects are orchestrated by the
/// caller around this pure decision so failure cannot fabricate success.
pub fn transition_workspace_lifecycle(
    current: WorkspaceLifecycleState,
    action: WorkspaceLifecycleAction,
) -> Result<WorkspaceLifecycleState, PongError> {
    use WorkspaceLifecycleAction as Action;
    use WorkspaceLifecycleState as State;

    let next = match (current, action) {
        (State::Archived, Action::Close) => State::Archived,
        (State::Archived, _) => {
            return Err(PongError::Conflict(
                "archived workspace cannot perform this lifecycle action".into(),
            ))
        }
        (State::Reconciling, Action::BeginRecovery) => State::Reconciling,
        (State::Reconciling, Action::CompleteRecovery) => State::Ready,
        (State::Reconciling, _) => {
            return Err(PongError::RecoveryRequired(
                "workspace requires reconciliation before this lifecycle action".into(),
            ))
        }
        (
            State::Created | State::Preparing | State::Ready | State::Active | State::Paused,
            Action::Open,
        ) => current,
        (
            State::Created | State::Preparing | State::Ready | State::Active | State::Paused,
            Action::Close,
        ) => State::Archived,
        (State::Created, Action::BeginCapture) => State::Preparing,
        (State::Preparing, Action::BeginCapture) => State::Preparing,
        (State::Preparing, Action::CompleteCapture) => State::Ready,
        (State::Ready, Action::CompleteCapture) => State::Ready,
        (State::Ready | State::Paused, Action::Activate) => State::Active,
        (State::Active, Action::Activate) => State::Active,
        (State::Active, Action::Pause) => State::Paused,
        (State::Paused, Action::Pause) => State::Paused,
        (State::Paused, Action::Resume) => State::Active,
        (State::Active, Action::Resume) => State::Active,
        (
            State::Created | State::Preparing | State::Ready | State::Active | State::Paused,
            Action::BeginRecovery,
        ) => State::Reconciling,
        (State::Ready | State::Active, Action::CompleteRecovery) => current,
        _ => {
            return Err(PongError::Conflict(format!(
                "invalid workspace lifecycle transition: {} + {:?}",
                current.as_str(),
                action
            )))
        }
    };
    Ok(next)
}

pub fn require_workspace_capability(
    capabilities: WorkspaceCapabilities,
    capability: WorkspaceCapability,
) -> Result<(), PongError> {
    if capabilities.supports(capability) {
        Ok(())
    } else {
        Err(PongError::Unsupported(format!(
            "workspace capability {:?} is not supported",
            capability
        )))
    }
}

fn lifecycle_operation_action(action: WorkspaceLifecycleAction) -> &'static str {
    match action {
        WorkspaceLifecycleAction::Open => "workspace.lifecycle.open",
        WorkspaceLifecycleAction::Close => "workspace.lifecycle.close",
        WorkspaceLifecycleAction::BeginCapture => "workspace.lifecycle.begin_capture",
        WorkspaceLifecycleAction::CompleteCapture => "workspace.lifecycle.complete_capture",
        WorkspaceLifecycleAction::Activate => "workspace.lifecycle.activate",
        WorkspaceLifecycleAction::Pause => "workspace.lifecycle.pause",
        WorkspaceLifecycleAction::Resume => "workspace.lifecycle.resume",
        WorkspaceLifecycleAction::BeginRecovery => "workspace.lifecycle.begin_recovery",
        WorkspaceLifecycleAction::CompleteRecovery => "workspace.lifecycle.complete_recovery",
    }
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            max_files: DEFAULT_MAX_FILES,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeEntry {
    pub path: String,
    pub kind: String,
    pub size: u64,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeManifest {
    pub manifest_version: u32,
    pub workspace_id: String,
    pub project_id: String,
    pub entries: Vec<TreeEntry>,
    pub redaction_profile_id: String,
    pub redaction_profile_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub snapshot_id: String,
    pub digest: Digest,
    pub workspace_id: String,
    pub project_id: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

/// The factual change kinds emitted by the internal snapshot diff boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SnapshotChangeType {
    Added,
    Removed,
    Modified,
    TypeChanged,
}

/// One deterministic path-level change between two verified manifests.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDiffEntry {
    pub path: String,
    pub change_type: SnapshotChangeType,
    pub old_digest: Option<String>,
    pub new_digest: Option<String>,
    pub old_size: Option<u64>,
    pub new_size: Option<u64>,
    pub old_type: Option<String>,
    pub new_type: Option<String>,
}

/// Deterministic, read-only comparison of two snapshot states.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub old_snapshot_id: String,
    pub new_snapshot_id: String,
    pub entries: Vec<SnapshotDiffEntry>,
}

/// Read-only result for comparing the current local tree with a workspace's
/// selected reference snapshot. `diff` reuses the snapshot-to-snapshot schema;
/// `current_tree_id` is an ephemeral canonical manifest identity and is not a
/// durable snapshot or workspace revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceDiffResult {
    pub workspace_id: String,
    pub project_id: String,
    pub reference_snapshot_id: String,
    /// The durable workspace revision observed before the filesystem scan.
    /// This is an optimistic-concurrency/lifecycle revision, not a
    /// filesystem observation counter.
    pub observation_revision: i64,
    /// The immutable environment binding observed with the workspace row.
    pub environment_id: Option<String>,
    /// Successful results are stable with respect to the durable workspace
    /// identity checked before and after the scan. File-level mutations are
    /// still governed by the existing point-in-time scanner checks.
    pub observation_stability: String,
    pub current_tree_id: String,
    pub diff: SnapshotDiff,
}

#[derive(Debug, Clone)]
pub struct LocalWorkspace {
    workspace_id: String,
    project_id: String,
    root: PathBuf,
    redactor: Redactor,
    failpoints: Arc<Mutex<WorkspaceFailpoints>>,
}

impl LocalWorkspace {
    /// Create a new empty materialization. The caller must ensure the path is
    /// outside the repository control directory before calling this method.
    pub fn create(
        workspace_id: impl Into<String>,
        project_id: impl Into<String>,
        root: impl AsRef<Path>,
        redactor: Redactor,
    ) -> Result<Self, PongError> {
        Self::create_with_failpoints(
            workspace_id,
            project_id,
            root,
            redactor,
            WorkspaceFailpoints::disabled(),
        )
    }

    /// Create a local workspace with an explicit deterministic fault plan.
    /// This constructor exists for fault-injection tests; normal callers
    /// should use [`LocalWorkspace::create`].
    pub fn create_with_failpoints(
        workspace_id: impl Into<String>,
        project_id: impl Into<String>,
        root: impl AsRef<Path>,
        redactor: Redactor,
        failpoints: WorkspaceFailpoints,
    ) -> Result<Self, PongError> {
        let workspace_id = workspace_id.into();
        let project_id = project_id.into();
        validate_identity(&workspace_id, "workspace id")?;
        validate_identity(&project_id, "project id")?;
        let root = root.as_ref().to_path_buf();
        let parent = root
            .parent()
            .ok_or_else(|| PongError::InvalidInput("workspace path has no parent".into()))?;
        ensure_no_reparse_ancestors(parent)?;
        fs::create_dir_all(parent)?;
        ensure_no_reparse_ancestors(parent)?;
        let parent_metadata = fs::symlink_metadata(parent)?;
        if !parent_metadata.is_dir() || is_reparse_point(&parent_metadata) {
            return Err(PongError::Integrity(
                "workspace parent is not a safe directory".into(),
            ));
        }
        if fs::symlink_metadata(&root).is_ok() {
            return Err(PongError::Conflict(
                "workspace materialization already exists".into(),
            ));
        }
        fs::create_dir(&root)?;
        sync_directory(parent)?;
        Self::from_existing(
            workspace_id,
            project_id,
            root,
            redactor,
            Arc::new(Mutex::new(failpoints)),
        )
    }

    pub fn open(
        workspace_id: impl Into<String>,
        project_id: impl Into<String>,
        root: impl AsRef<Path>,
        redactor: Redactor,
    ) -> Result<Self, PongError> {
        Self::open_with_failpoints(
            workspace_id,
            project_id,
            root,
            redactor,
            WorkspaceFailpoints::disabled(),
        )
    }

    /// Open a local workspace with an explicit deterministic fault plan.
    /// This constructor exists for fault-injection tests; normal callers
    /// should use [`LocalWorkspace::open`].
    pub fn open_with_failpoints(
        workspace_id: impl Into<String>,
        project_id: impl Into<String>,
        root: impl AsRef<Path>,
        redactor: Redactor,
        failpoints: WorkspaceFailpoints,
    ) -> Result<Self, PongError> {
        Self::open_with_failpoint_state(
            workspace_id,
            project_id,
            root,
            redactor,
            Arc::new(Mutex::new(failpoints)),
        )
    }

    fn open_with_failpoint_state(
        workspace_id: impl Into<String>,
        project_id: impl Into<String>,
        root: impl AsRef<Path>,
        redactor: Redactor,
        failpoints: Arc<Mutex<WorkspaceFailpoints>>,
    ) -> Result<Self, PongError> {
        let workspace_id = workspace_id.into();
        let project_id = project_id.into();
        validate_identity(&workspace_id, "workspace id")?;
        validate_identity(&project_id, "project id")?;
        Self::from_existing(
            workspace_id,
            project_id,
            root.as_ref().to_path_buf(),
            redactor,
            failpoints,
        )
    }

    fn from_existing(
        workspace_id: String,
        project_id: String,
        root: PathBuf,
        redactor: Redactor,
        failpoints: Arc<Mutex<WorkspaceFailpoints>>,
    ) -> Result<Self, PongError> {
        ensure_no_reparse_ancestors(&root)?;
        let metadata = fs::symlink_metadata(&root).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                PongError::NotFound("workspace materialization does not exist".into())
            } else {
                PongError::from(error)
            }
        })?;
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return Err(PongError::Integrity(
                "workspace materialization is not a safe directory".into(),
            ));
        }
        let root = fs::canonicalize(root).map_err(PongError::from)?;
        Ok(Self {
            workspace_id,
            project_id,
            root,
            redactor,
            failpoints,
        })
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Return the current test fault configuration.
    pub fn failpoints(&self) -> WorkspaceFailpoints {
        *self
            .failpoints
            .lock()
            .expect("workspace failpoint mutex is not poisoned")
    }

    /// Replace the instance-scoped test fault configuration.
    pub fn set_failpoints(&self, failpoints: WorkspaceFailpoints) {
        *self
            .failpoints
            .lock()
            .expect("workspace failpoint mutex is not poisoned") = failpoints;
    }

    /// Capture a deterministic tree. File bytes are published before the
    /// manifest; an error leaves only unreachable, verified CAS objects.
    pub fn snapshot(&self, cas: &Cas, options: SnapshotOptions) -> Result<Snapshot, PongError> {
        if options.max_files == 0 || options.max_file_bytes == 0 {
            return Err(PongError::InvalidInput(
                "snapshot limits must be positive".into(),
            ));
        }
        let mut entries = Vec::new();
        collect_entries(
            &self.root,
            Path::new(""),
            cas,
            &self.redactor,
            options,
            &mut entries,
        )?;
        entries.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
        let manifest = TreeManifest {
            manifest_version: TREE_MANIFEST_VERSION,
            workspace_id: self.workspace_id.clone(),
            project_id: self.project_id.clone(),
            entries,
            redaction_profile_id: self.redactor.profile_id().into(),
            redaction_profile_version: self.redactor.version().into(),
        };
        let manifest_value = serde_json::to_value(&manifest).map_err(|error| {
            PongError::Serialization(format!("cannot encode workspace tree manifest: {error}"))
        })?;
        let manifest_bytes = canonical_bytes(&manifest_value)?;
        let digest = cas.put(TREE_DOMAIN, &manifest_bytes)?;
        let file_count = manifest
            .entries
            .iter()
            .filter(|entry| entry.kind == "file")
            .count();
        let total_bytes = manifest
            .entries
            .iter()
            .filter(|entry| entry.kind == "file")
            .try_fold(0u64, |total, entry| {
                total.checked_add(entry.size).ok_or_else(|| {
                    PongError::ResourceExhausted(
                        "workspace snapshot total size exceeds supported range".into(),
                    )
                })
            })?;
        Ok(Snapshot {
            snapshot_id: format!("snp-{}", digest),
            digest,
            workspace_id: self.workspace_id.clone(),
            project_id: self.project_id.clone(),
            file_count,
            total_bytes,
        })
    }

    pub fn read_manifest(&self, cas: &Cas, digest: Digest) -> Result<TreeManifest, PongError> {
        self.read_manifest_for_identity(cas, digest, &self.workspace_id, &self.project_id)
    }

    /// Read a canonical manifest owned by another Workspace without treating
    /// that Workspace as the current driver's identity. This is the narrow
    /// read-only source boundary used by cross-Workspace materialization.
    pub fn read_manifest_for_identity(
        &self,
        cas: &Cas,
        digest: Digest,
        workspace_id: &str,
        project_id: &str,
    ) -> Result<TreeManifest, PongError> {
        let bytes = cas.get(TREE_DOMAIN, digest)?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| PongError::Integrity("workspace tree manifest is invalid JSON".into()))?;
        let canonical = canonical_bytes(&value)?;
        if canonical != bytes {
            return Err(PongError::Integrity(
                "workspace tree manifest is not canonical".into(),
            ));
        }
        let manifest: TreeManifest = serde_json::from_value(value).map_err(|_| {
            PongError::Integrity("workspace tree manifest fields are invalid".into())
        })?;
        validate_manifest(&manifest, workspace_id, project_id, &self.redactor)?;
        Ok(manifest)
    }

    /// Rebind a verified immutable source manifest to the target Workspace's
    /// local Snapshot identity. File blob digests remain shared; only the
    /// canonical manifest metadata changes, so the source CAS object is never
    /// modified or reassigned.
    pub fn rebind_manifest_for_workspace(
        &self,
        cas: &Cas,
        source_digest: Digest,
        source_workspace_id: &str,
        source_project_id: &str,
        target_workspace_id: &str,
        target_project_id: &str,
    ) -> Result<(Digest, TreeManifest), PongError> {
        let mut manifest = self.read_manifest_for_identity(
            cas,
            source_digest,
            source_workspace_id,
            source_project_id,
        )?;
        manifest.workspace_id = target_workspace_id.to_owned();
        manifest.project_id = target_project_id.to_owned();
        validate_manifest(
            &manifest,
            target_workspace_id,
            target_project_id,
            &self.redactor,
        )?;
        let value = serde_json::to_value(&manifest).map_err(|error| {
            PongError::Serialization(format!("cannot encode rebound workspace manifest: {error}"))
        })?;
        let bytes = canonical_bytes(&value)?;
        let digest = cas.put(TREE_DOMAIN, &bytes)?;
        Ok((digest, manifest))
    }

    /// Compare two verified snapshot manifests without reading their file
    /// blobs. The existing manifest validator supplies identity, canonical
    /// path, redaction, and digest-shape checks before the linear merge.
    pub fn diff_snapshots(
        &self,
        cas: &Cas,
        old_digest: Digest,
        new_digest: Digest,
    ) -> Result<SnapshotDiff, PongError> {
        let old_manifest = self.read_manifest(cas, old_digest)?;
        let new_manifest = self.read_manifest(cas, new_digest)?;
        let entries = diff_manifests(&old_manifest, &new_manifest);
        Ok(SnapshotDiff {
            old_snapshot_id: format!("snp-{old_digest}"),
            new_snapshot_id: format!("snp-{new_digest}"),
            entries,
        })
    }

    /// Compare the current local tree with a verified reference manifest.
    /// Current entries are generated in memory through the same path and file
    /// safety checks used by workspace status; no CAS object is published.
    pub fn diff_against_snapshot(
        &self,
        cas: &Cas,
        reference_digest: Digest,
    ) -> Result<SnapshotDiff, PongError> {
        let reference = self.read_manifest(cas, reference_digest)?;
        let current = self.current_tree_manifest()?;
        let current_digest = manifest_digest(&current)?;
        let diff = diff_manifests(&reference, &current);
        let current_tree_id = format!("workspace-current-{current_digest}");
        Ok(SnapshotDiff {
            old_snapshot_id: format!("snp-{reference_digest}"),
            new_snapshot_id: current_tree_id,
            entries: diff,
        })
    }

    /// Compare the current tree with a source manifest whose Workspace
    /// identity has already been validated by the metadata layer. This keeps
    /// source reads read-only while reusing the deterministic M2 diff shape.
    pub fn diff_against_manifest(
        &self,
        source_snapshot_id: &str,
        source_manifest: &TreeManifest,
    ) -> Result<SnapshotDiff, PongError> {
        let current = self.current_tree_manifest()?;
        let current_digest = manifest_digest(&current)?;
        Ok(SnapshotDiff {
            old_snapshot_id: source_snapshot_id.to_owned(),
            new_snapshot_id: format!("workspace-current-{current_digest}"),
            entries: diff_manifests(source_manifest, &current),
        })
    }

    fn current_tree_manifest(&self) -> Result<TreeManifest, PongError> {
        let mut entries = Vec::new();
        collect_status_entries(
            &self.root,
            Path::new(""),
            &self.redactor,
            SnapshotOptions::default(),
            &mut entries,
        )?;
        entries.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
        Ok(TreeManifest {
            manifest_version: TREE_MANIFEST_VERSION,
            workspace_id: self.workspace_id.clone(),
            project_id: self.project_id.clone(),
            entries,
            redaction_profile_id: self.redactor.profile_id().into(),
            redaction_profile_version: self.redactor.version().into(),
        })
    }

    /// Materialize a verified snapshot into a new directory. The destination
    /// must not already exist; this avoids deleting user data on a failed
    /// restore and makes the operation easy to reconcile.
    pub fn materialize(
        &self,
        cas: &Cas,
        digest: Digest,
        destination: impl AsRef<Path>,
    ) -> Result<PathBuf, PongError> {
        let manifest = self.read_manifest(cas, digest)?;
        let destination = destination.as_ref().to_path_buf();
        if fs::symlink_metadata(&destination).is_ok() {
            return Err(PongError::Conflict(
                "materialization destination already exists".into(),
            ));
        }
        let parent = destination
            .parent()
            .ok_or_else(|| PongError::InvalidInput("materialization path has no parent".into()))?;
        ensure_no_reparse_ancestors(parent)?;
        fs::create_dir_all(parent)?;
        ensure_no_reparse_ancestors(parent)?;
        let destination_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                PongError::InvalidInput("materialization destination is not valid UTF-8".into())
            })?;
        let temporary = parent.join(format!(
            ".{}.snapshot-{}",
            destination_name,
            unique_suffix()
        ));
        fs::create_dir(&temporary)?;
        let result = self.materialize_into(cas, &manifest, &temporary);
        if let Err(error) = result {
            return Err(self.cleanup_after_failure(&temporary, error));
        }
        if let Err(error) = self.sync_tree_directory(&temporary) {
            return Err(self.cleanup_after_failure(&temporary, error));
        }
        if fs::symlink_metadata(&destination).is_ok() {
            return Err(self.cleanup_after_failure(
                &temporary,
                PongError::Conflict("materialization destination already exists".into()),
            ));
        }
        if let Some(action) = self.take_fault(WorkspaceFailPoint::MaterializeRename) {
            if !matches!(action, WorkspaceFaultAction::Continue) {
                return Err(self.cleanup_after_failure(
                    &temporary,
                    workspace_fault_error(WorkspaceFailPoint::MaterializeRename, action),
                ));
            }
        }
        if let Err(error) = rename_new_with_retry(&temporary, &destination) {
            return Err(self.cleanup_after_failure(&temporary, error));
        }
        // The destination is now committed. A parent sync failure is
        // outcome-unknown; keep the materialization for reconciliation.
        if let Some(action) = self.take_fault(WorkspaceFailPoint::MaterializeParentDirectorySync) {
            if !matches!(action, WorkspaceFaultAction::Continue) {
                return Err(workspace_fault_error(
                    WorkspaceFailPoint::MaterializeParentDirectorySync,
                    action,
                ));
            }
        }
        sync_directory(parent)?;
        Ok(destination)
    }

    /// Replace this workspace's existing physical tree with a verified
    /// Snapshot.  The replacement is staged in a sibling directory and the
    /// old tree is retained as a short-lived sibling while the new tree is
    /// published.  This is intentionally local-driver machinery; durable
    /// Workspace heads are published by the metadata transaction after this
    /// method returns successfully.
    pub fn replace_with_snapshot(&self, cas: &Cas, digest: Digest) -> Result<(), PongError> {
        let manifest = self.read_manifest(cas, digest)?;
        let parent = self
            .root
            .parent()
            .ok_or_else(|| PongError::InvalidInput("workspace path has no parent".into()))?;
        ensure_no_reparse_ancestors(parent)?;

        recover_rollback_layout(parent, self.root.file_name(), &self.root)?;

        // A retry after the filesystem publication boundary may already have
        // the target tree.  Treat that as a deterministic completed physical
        // step instead of attempting a second swap.
        if self.verify_materialized(cas, digest, &self.root).is_ok() {
            cleanup_rollback_backups(parent, self.root.file_name())?;
            return Ok(());
        }

        let name = self
            .root
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| PongError::InvalidInput("workspace path is not valid UTF-8".into()))?;
        let temporary = parent.join(format!(".{name}.rollback-{}", unique_suffix()));
        fs::create_dir(&temporary)?;
        if let Err(error) = self.materialize_into(cas, &manifest, &temporary) {
            return Err(self.cleanup_after_failure(&temporary, error));
        }
        if let Err(error) = self.sync_tree_directory(&temporary) {
            return Err(self.cleanup_after_failure(&temporary, error));
        }
        self.verify_materialized(cas, digest, &temporary)
            .map_err(|error| self.cleanup_after_failure(&temporary, error))?;

        if let Some(action) = self.take_fault(WorkspaceFailPoint::MaterializeRename) {
            if !matches!(action, WorkspaceFaultAction::Continue) {
                return Err(self.cleanup_after_failure(
                    &temporary,
                    workspace_fault_error(WorkspaceFailPoint::MaterializeRename, action),
                ));
            }
        }

        let backup = parent.join(format!(".{name}.rollback-old-{}", unique_suffix()));
        if let Err(error) = fs::rename(&self.root, &backup) {
            return Err(self.cleanup_after_failure(&temporary, PongError::from(error)));
        }
        if let Err(error) = fs::rename(&temporary, &self.root) {
            let restore = fs::rename(&backup, &self.root);
            let original = PongError::from(error);
            return match restore {
                Ok(()) => Err(self.cleanup_after_failure(&temporary, original)),
                Err(restore_error) => Err(PongError::RecoveryRequired(format!(
                    "rollback replacement failed ({original}); restoring old workspace failed ({restore_error})"
                ))),
            };
        }

        // The new tree is now visible.  A parent-sync fault deliberately leaves
        // the replacement in place so a caller can verify/retry deterministically.
        if let Some(action) = self.take_fault(WorkspaceFailPoint::MaterializeParentDirectorySync) {
            if !matches!(action, WorkspaceFaultAction::Continue) {
                return Err(workspace_fault_error(
                    WorkspaceFailPoint::MaterializeParentDirectorySync,
                    action,
                ));
            }
        }
        sync_directory(parent)?;
        fs::remove_dir_all(&backup).map_err(PongError::from)?;
        sync_directory(parent)?;
        Ok(())
    }

    /// Verify a previously published materialization against the immutable
    /// manifest and CAS blobs. This is used to reconcile a retry after the
    /// filesystem publication succeeded but durable operation recording was
    /// interrupted.
    pub fn verify_materialized(
        &self,
        cas: &Cas,
        digest: Digest,
        destination: impl AsRef<Path>,
    ) -> Result<(), PongError> {
        let manifest = self.read_manifest(cas, digest)?;
        let destination = destination.as_ref();
        ensure_no_reparse_ancestors(destination)?;
        let metadata = fs::symlink_metadata(destination).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                PongError::NotFound("materialized destination does not exist".into())
            } else {
                PongError::from(error)
            }
        })?;
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return Err(PongError::Integrity(
                "materialized destination is not a safe directory".into(),
            ));
        }

        let mut expected = HashSet::new();
        for entry in &manifest.entries {
            let relative = safe_relative_path(&entry.path)?;
            expected.insert(entry.path.clone());
            let target = destination.join(&relative);
            let metadata = fs::symlink_metadata(&target).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    PongError::Integrity(format!(
                        "materialized destination is missing {}",
                        entry.path
                    ))
                } else {
                    PongError::from(error)
                }
            })?;
            if is_reparse_point(&metadata) {
                return Err(PongError::Integrity(
                    "materialized destination contains a symlink or reparse point".into(),
                ));
            }
            match entry.kind.as_str() {
                "directory" => {
                    if !metadata.is_dir() {
                        return Err(PongError::Integrity(format!(
                            "materialized directory {} is not a directory",
                            entry.path
                        )));
                    }
                }
                "file" => {
                    if !metadata.is_file() {
                        return Err(PongError::Integrity(format!(
                            "materialized file {} is not a regular file",
                            entry.path
                        )));
                    }
                    let digest_text = entry.digest.as_deref().ok_or_else(|| {
                        PongError::Integrity("snapshot file is missing its digest".into())
                    })?;
                    let blob_digest = Digest::from_hex(
                        digest_text.strip_prefix("sha256:").unwrap_or(digest_text),
                    )?;
                    let bytes = fs::read(&target).map_err(PongError::from_protected_io)?;
                    if bytes.len() as u64 != entry.size
                        || digest_for(BLOB_DOMAIN, &bytes) != blob_digest
                    {
                        return Err(PongError::Integrity(format!(
                            "materialized file {} failed size or digest verification",
                            entry.path
                        )));
                    }
                    let cas_bytes = cas.get(BLOB_DOMAIN, blob_digest)?;
                    if cas_bytes != bytes {
                        return Err(PongError::Integrity(format!(
                            "materialized file {} differs from its CAS blob",
                            entry.path
                        )));
                    }
                }
                _ => {
                    return Err(PongError::Unsupported(
                        "snapshot entry kind is unsupported".into(),
                    ))
                }
            }
        }

        let mut actual = Vec::new();
        collect_materialized_paths(destination, Path::new(""), &mut actual)?;
        for path in actual {
            if !expected.contains(&path) {
                return Err(PongError::Integrity(format!(
                    "materialized destination contains unexpected entry {path}"
                )));
            }
        }
        Ok(())
    }

    /// Compare the current local tree to a verified immutable manifest without
    /// publishing any new CAS objects. This is intentionally a point-in-time
    /// read for status; mutations remain guarded by the workspace lease APIs.
    fn matches_manifest(
        &self,
        cas: &Cas,
        manifest_digest: Digest,
        manifest: &TreeManifest,
    ) -> Result<bool, PongError> {
        verify_manifest_blobs(cas, manifest)?;
        let mut current = Vec::new();
        collect_status_entries(
            &self.root,
            Path::new(""),
            &self.redactor,
            SnapshotOptions::default(),
            &mut current,
        )?;
        current.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
        let expected_digest = digest_for(
            TREE_DOMAIN,
            &canonical_bytes(&serde_json::to_value(manifest).map_err(|error| {
                PongError::Serialization(format!(
                    "cannot encode workspace tree manifest for status: {error}"
                ))
            })?)?,
        );
        if expected_digest != manifest_digest {
            return Err(PongError::Integrity(
                "workspace tree manifest digest does not match its canonical bytes".into(),
            ));
        }
        Ok(current == manifest.entries)
    }

    fn materialize_into(
        &self,
        cas: &Cas,
        manifest: &TreeManifest,
        destination: &Path,
    ) -> Result<(), PongError> {
        for entry in &manifest.entries {
            let relative = safe_relative_path(&entry.path)?;
            let target = destination.join(&relative);
            match entry.kind.as_str() {
                "directory" => {
                    ensure_no_reparse_components(destination, &relative)?;
                    fs::create_dir_all(&target)?;
                    ensure_no_reparse_components(destination, &relative)?;
                }
                "file" => {
                    let parent = target.parent().ok_or_else(|| {
                        PongError::Integrity("snapshot file has no parent".into())
                    })?;
                    let parent_relative = relative.parent().ok_or_else(|| {
                        PongError::Integrity("snapshot file parent is invalid".into())
                    })?;
                    ensure_no_reparse_components(destination, parent_relative)?;
                    fs::create_dir_all(parent)?;
                    ensure_no_reparse_components(destination, &relative)?;
                    let digest_text = entry.digest.as_deref().ok_or_else(|| {
                        PongError::Integrity("snapshot file is missing its digest".into())
                    })?;
                    let blob_digest = Digest::from_hex(
                        digest_text.strip_prefix("sha256:").unwrap_or(digest_text),
                    )?;
                    let bytes = cas.get(BLOB_DOMAIN, blob_digest)?;
                    if bytes.len() as u64 != entry.size {
                        return Err(PongError::Integrity(
                            "snapshot file size does not match its manifest".into(),
                        ));
                    }
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&target)?;
                    match self.take_fault(WorkspaceFailPoint::MaterializeFileWrite) {
                        None | Some(WorkspaceFaultAction::Continue) => file.write_all(&bytes)?,
                        Some(WorkspaceFaultAction::ShortWrite(limit)) => {
                            let prefix_len = limit.min(bytes.len());
                            if prefix_len > 0 {
                                file.write_all(&bytes[..prefix_len])?;
                            }
                            return Err(workspace_fault_error(
                                WorkspaceFailPoint::MaterializeFileWrite,
                                WorkspaceFaultAction::ShortWrite(limit),
                            ));
                        }
                        Some(action) => {
                            return Err(workspace_fault_error(
                                WorkspaceFailPoint::MaterializeFileWrite,
                                action,
                            ));
                        }
                    }
                    match self.take_fault(WorkspaceFailPoint::MaterializeFileSync) {
                        None | Some(WorkspaceFaultAction::Continue) => file.sync_all()?,
                        Some(action) => {
                            return Err(workspace_fault_error(
                                WorkspaceFailPoint::MaterializeFileSync,
                                action,
                            ));
                        }
                    }
                }
                _ => {
                    return Err(PongError::Unsupported(
                        "snapshot entry kind is unsupported".into(),
                    ))
                }
            }
        }
        Ok(())
    }

    fn sync_tree_directory(&self, path: &Path) -> Result<(), PongError> {
        for entry in fs::read_dir(path).map_err(PongError::from)? {
            let entry = entry.map_err(PongError::from)?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(PongError::from)?;
            if metadata.is_dir() {
                self.sync_tree_directory(&entry.path())?;
            }
        }
        match self.take_fault(WorkspaceFailPoint::MaterializeTreeDirectorySync) {
            None | Some(WorkspaceFaultAction::Continue) => sync_directory(path),
            Some(action) => Err(workspace_fault_error(
                WorkspaceFailPoint::MaterializeTreeDirectorySync,
                action,
            )),
        }
    }

    fn cleanup_after_failure(&self, temporary: &Path, error: PongError) -> PongError {
        match self.remove_temporary(temporary) {
            Ok(()) => error,
            Err(cleanup_error) => PongError::RecoveryRequired(format!(
                "materialization failed ({error}); temporary directory cleanup failed ({cleanup_error})"
            )),
        }
    }

    fn remove_temporary(&self, temporary: &Path) -> Result<(), PongError> {
        match self.take_fault(WorkspaceFailPoint::CleanupTemporary) {
            None | Some(WorkspaceFaultAction::Continue) => {}
            Some(action) => {
                return Err(workspace_fault_error(
                    WorkspaceFailPoint::CleanupTemporary,
                    action,
                ));
            }
        }
        match fs::symlink_metadata(temporary) {
            Ok(metadata) if is_reparse_point(&metadata) => Err(PongError::Integrity(
                "temporary materialization is a symlink or reparse point".into(),
            )),
            Ok(metadata) if !metadata.is_dir() => Err(PongError::Integrity(
                "temporary materialization is not a directory".into(),
            )),
            Ok(_) => fs::remove_dir_all(temporary).map_err(PongError::from),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(PongError::from_protected_io(error)),
        }
    }

    /// Remove abandoned sibling temporary directories created for a
    /// destination by a previous materialization attempt or process. This is
    /// an explicit reconciliation action: a caller must choose the
    /// destination whose temporary siblings are safe to remove.
    ///
    /// Only directories with the exact `.<destination>.snapshot-` prefix are
    /// considered. A matching symlink/reparse point is rejected rather than
    /// followed or removed.
    pub fn cleanup_orphan_materializations(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<usize, PongError> {
        let destination = destination.as_ref();
        let parent = destination
            .parent()
            .ok_or_else(|| PongError::InvalidInput("materialization path has no parent".into()))?;
        ensure_no_reparse_ancestors(parent)?;
        let destination_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                PongError::InvalidInput("materialization destination is not valid UTF-8".into())
            })?;
        let prefix = format!(".{destination_name}.snapshot-");
        let mut removed = 0usize;
        for entry in fs::read_dir(parent).map_err(PongError::from_protected_io)? {
            let entry = entry.map_err(PongError::from_protected_io)?;
            let name = entry.file_name();
            if name
                .to_str()
                .map_or(true, |name| !name.starts_with(&prefix))
            {
                continue;
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(PongError::from_protected_io)?;
            if is_reparse_point(&metadata) || !metadata.is_dir() {
                return Err(PongError::Integrity(
                    "orphan materialization entry is not a regular directory".into(),
                ));
            }
            self.remove_temporary(&path)?;
            removed = removed
                .checked_add(1)
                .ok_or_else(|| PongError::ResourceExhausted("orphan count overflow".into()))?;
        }
        if removed > 0 {
            sync_directory(parent)?;
        }
        Ok(removed)
    }

    fn take_fault(&self, point: WorkspaceFailPoint) -> Option<WorkspaceFaultAction> {
        self.failpoints
            .lock()
            .expect("workspace failpoint mutex is not poisoned")
            .take_if(point)
    }
}

/// Small orchestration layer that binds a logical workspace to the existing
/// repository metadata and CAS without exposing SQLite handles to providers.
pub struct WorkspaceManager<'a> {
    repository: &'a mut Repository,
    redactor: Redactor,
    failpoints: Arc<Mutex<WorkspaceFailpoints>>,
}

impl<'a> WorkspaceManager<'a> {
    pub fn new(repository: &'a mut Repository, redactor: Redactor) -> Self {
        Self::new_with_failpoints(repository, redactor, WorkspaceFailpoints::disabled())
    }

    /// Construct a manager with an explicit deterministic materialization
    /// fault plan. Normal callers should use [`WorkspaceManager::new`].
    pub fn new_with_failpoints(
        repository: &'a mut Repository,
        redactor: Redactor,
        failpoints: WorkspaceFailpoints,
    ) -> Self {
        Self {
            repository,
            redactor,
            failpoints: Arc::new(Mutex::new(failpoints)),
        }
    }

    /// Return the current test fault configuration.
    pub fn failpoints(&self) -> WorkspaceFailpoints {
        *self
            .failpoints
            .lock()
            .expect("workspace failpoint mutex is not poisoned")
    }

    /// Replace the instance-scoped test fault configuration.
    pub fn set_failpoints(&self, failpoints: WorkspaceFailpoints) {
        *self
            .failpoints
            .lock()
            .expect("workspace failpoint mutex is not poisoned") = failpoints;
    }

    pub fn create_local(
        &mut self,
        workspace_id: &str,
        project_id: &str,
        path: impl AsRef<Path>,
        branch_ref: Option<&str>,
        environment_id: Option<&str>,
        now: &str,
    ) -> Result<WorkspaceRecord, PongError> {
        let locator = local_locator(self.repository.root(), path.as_ref())?;
        if self.redactor.redact_text(&locator) != locator {
            return Err(PongError::InvalidInput(
                "workspace locator contains configured secret material".into(),
            ));
        }
        let workspace = LocalWorkspace::create(
            workspace_id,
            project_id,
            path.as_ref(),
            self.redactor.clone(),
        )?;
        let record = WorkspaceRecord {
            workspace_id: workspace_id.into(),
            project_id: project_id.into(),
            driver: "local".into(),
            locator,
            branch_ref: branch_ref.map(str::to_owned),
            head: None,
            version_head_id: None,
            environment_id: environment_id.map(str::to_owned),
            status: "created".into(),
            revision: 0,
            created_at: now.into(),
            updated_at: now.into(),
        };
        match self.repository.metadata_mut().create_workspace(&record) {
            Ok(record) => Ok(record),
            Err(error) => {
                // An after-commit error is intentionally ambiguous. Re-read
                // the durable row before cleaning up the physical directory;
                // deleting it after SQLite committed would leave a valid
                // logical workspace pointing at missing state.
                let committed = self
                    .repository
                    .metadata()
                    .workspace(&record.workspace_id)
                    .ok()
                    .flatten()
                    .is_some();
                if !committed {
                    let _ = fs::remove_dir(workspace.root());
                }
                Err(error)
            }
        }
    }

    pub fn acquire_lease(
        &mut self,
        workspace_id: &str,
        agent_id: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<LeaseToken, PongError> {
        self.repository.metadata_mut().acquire_workspace_lease(
            workspace_id,
            agent_id,
            now_ms,
            ttl_ms,
        )
    }

    pub fn renew_lease(
        &mut self,
        token: &LeaseToken,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<LeaseToken, PongError> {
        self.repository
            .metadata_mut()
            .renew_workspace_lease(token, now_ms, ttl_ms)
    }

    pub fn release_lease(&mut self, token: &LeaseToken, now_ms: i64) -> Result<(), PongError> {
        self.repository
            .metadata_mut()
            .release_workspace_lease(token, now_ms)
    }

    /// Materialize an immutable Version owned by any compatible Workspace
    /// into the explicitly authorized target Workspace.  The source manifest
    /// is rebound to the target identity before the existing local replacement
    /// and snapshot-publication paths are used; source metadata and CAS blobs
    /// remain immutable and shared.
    pub fn materialize_from_version(
        &mut self,
        target_workspace_id: &str,
        source_version_id: &str,
        lease: &LeaseToken,
        expected_revision: i64,
        now_ms: i64,
        now: &str,
    ) -> Result<Snapshot, PongError> {
        let target = self
            .repository
            .metadata()
            .workspace(target_workspace_id)?
            .ok_or_else(|| PongError::NotFound("target workspace does not exist".into()))?;
        if target.driver != "local" {
            return Err(PongError::Unsupported(
                "workspace is only materializable through the local driver".into(),
            ));
        }
        if target.revision != expected_revision {
            return Err(PongError::Conflict("workspace revision is stale".into()));
        }
        self.validate_workspace_lease(target_workspace_id, lease, now_ms)?;
        let (source_version, source_snapshot, source_digest, source_manifest) =
            self.source_manifest_for_target(&target, source_version_id)?;
        let local = LocalWorkspace::open_with_failpoint_state(
            &target.workspace_id,
            &target.project_id,
            PathBuf::from(&target.locator),
            self.redactor.clone(),
            Arc::clone(&self.failpoints),
        )?;
        let (target_digest, target_manifest) = local.rebind_manifest_for_workspace(
            self.repository.cas(),
            source_digest,
            &source_version.workspace_id,
            &source_version.project_id,
            &target.workspace_id,
            &target.project_id,
        )?;
        verify_manifest_blobs(self.repository.cas(), &source_manifest)?;
        local.replace_with_snapshot(self.repository.cas(), target_digest)?;
        local.verify_materialized(self.repository.cas(), target_digest, local.root())?;
        let published = self.snapshot_local(
            target_workspace_id,
            lease,
            SnapshotOptions::default(),
            now_ms,
            now,
        )?;
        let published_manifest = local.read_manifest(self.repository.cas(), published.digest)?;
        if published_manifest != target_manifest
            || published.workspace_id != target.workspace_id
            || published.project_id != target.project_id
            || source_snapshot.project_id != target.project_id
        {
            return Err(PongError::Integrity(
                "target Snapshot publication does not match source materialization".into(),
            ));
        }
        Ok(published)
    }

    /// Restore an immutable Version into a target Workspace.  This is the
    /// cross-Workspace counterpart to `restore_local`: unlike the legacy
    /// destination restore API it publishes a target-local Workspace Head.
    pub fn restore_from_version(
        &mut self,
        target_workspace_id: &str,
        source_version_id: &str,
        lease: &LeaseToken,
        expected_revision: i64,
        now_ms: i64,
        now: &str,
    ) -> Result<Snapshot, PongError> {
        self.materialize_from_version(
            target_workspace_id,
            source_version_id,
            lease,
            expected_revision,
            now_ms,
            now,
        )
    }

    /// Read-only diff of the target physical tree against a compatible
    /// immutable source Version.  No lease, revision, operation, event, CAS,
    /// or Workspace Head is mutated.
    pub fn diff_workspace_against_version(
        &self,
        target_workspace_id: &str,
        source_version_id: &str,
    ) -> Result<SnapshotDiff, PongError> {
        let target = self
            .repository
            .metadata()
            .workspace(target_workspace_id)?
            .ok_or_else(|| PongError::NotFound("target workspace does not exist".into()))?;
        if target.driver != "local" {
            return Err(PongError::Unsupported(
                "workspace diff is only implemented for the local driver".into(),
            ));
        }
        let (_version, snapshot, source_digest, manifest) =
            self.source_manifest_for_target(&target, source_version_id)?;
        let local = LocalWorkspace::open_with_failpoint_state(
            &target.workspace_id,
            &target.project_id,
            PathBuf::from(&target.locator),
            self.redactor.clone(),
            Arc::clone(&self.failpoints),
        )?;
        let result = local.diff_against_manifest(&snapshot.snapshot_id, &manifest)?;
        if result.old_snapshot_id != snapshot.snapshot_id {
            return Err(PongError::Integrity(
                "source diff reference is inconsistent".into(),
            ));
        }
        let _ = source_digest;
        Ok(result)
    }

    fn validate_workspace_lease(
        &self,
        workspace_id: &str,
        lease: &LeaseToken,
        now_ms: i64,
    ) -> Result<(), PongError> {
        if lease.workspace_id != workspace_id {
            return Err(PongError::Conflict(
                "lease belongs to another workspace".into(),
            ));
        }
        let current = self
            .repository
            .metadata()
            .workspace_lease(workspace_id)?
            .ok_or_else(|| PongError::Conflict("workspace lease is missing".into()))?;
        if current.agent_id.as_deref() != Some(lease.agent_id.as_str())
            || current.epoch != lease.epoch
            || current.expires_at_ms <= now_ms
        {
            return Err(PongError::Conflict(
                "workspace lease is stale or expired".into(),
            ));
        }
        Ok(())
    }

    fn source_manifest_for_target(
        &self,
        target: &WorkspaceRecord,
        source_version_id: &str,
    ) -> Result<
        (
            crate::metadata::VersionRecord,
            crate::metadata::SnapshotRecord,
            Digest,
            TreeManifest,
        ),
        PongError,
    > {
        let version = self
            .repository
            .metadata()
            .version_record(source_version_id)?
            .ok_or_else(|| PongError::NotFound("source Version does not exist".into()))?;
        if version.project_id != target.project_id
            || version.environment_id != target.environment_id
        {
            return Err(PongError::Conflict(
                "source Version is incompatible with target workspace".into(),
            ));
        }
        let source_workspace = self
            .repository
            .metadata()
            .workspace(&version.workspace_id)?
            .ok_or_else(|| PongError::Integrity("source Workspace does not exist".into()))?;
        if source_workspace.project_id != version.project_id
            || source_workspace.environment_id != version.environment_id
        {
            return Err(PongError::Integrity(
                "source Version Workspace binding is inconsistent".into(),
            ));
        }
        let snapshot = self
            .repository
            .metadata()
            .snapshot_record(&version.snapshot_id)?
            .ok_or_else(|| PongError::Integrity("source Snapshot does not exist".into()))?;
        if snapshot.workspace_id != version.workspace_id
            || snapshot.project_id != version.project_id
            || snapshot.environment_id != version.environment_id
            || snapshot.generation_id != version.generation_id
            || snapshot.migration_id != version.migration_id
        {
            return Err(PongError::Integrity(
                "source Snapshot binding is inconsistent".into(),
            ));
        }
        let digest_text = snapshot
            .root_digest
            .strip_prefix("sha256:")
            .ok_or_else(|| PongError::Integrity("source Snapshot digest is not prefixed".into()))?;
        let digest = Digest::from_hex(digest_text)?;
        if snapshot.snapshot_id != format!("snp-{digest}") {
            return Err(PongError::Integrity(
                "source Snapshot identity does not match its digest".into(),
            ));
        }
        let target_local = LocalWorkspace::open_with_failpoint_state(
            &target.workspace_id,
            &target.project_id,
            PathBuf::from(&target.locator),
            self.redactor.clone(),
            Arc::clone(&self.failpoints),
        )?;
        let manifest = target_local.read_manifest_for_identity(
            self.repository.cas(),
            digest,
            &version.workspace_id,
            &version.project_id,
        )?;
        if snapshot.manifest_version != TREE_MANIFEST_VERSION {
            return Err(PongError::Integrity(
                "source Snapshot manifest is incompatible".into(),
            ));
        }
        Ok((version, snapshot, digest, manifest))
    }

    /// Restore one immutable Version/Checkpoint into the existing local
    /// Workspace and publish the resulting Snapshot Head and Version Head in
    /// one guarded metadata transaction.  The rollback record is prepared
    /// before filesystem mutation and remains retryable until both authorities
    /// agree on the target.
    pub fn rollback_local(
        &mut self,
        request: &RollbackCreation,
    ) -> Result<RollbackRecord, PongError> {
        let expected_revision = request.expected_workspace_revision.ok_or_else(|| {
            PongError::InvalidInput("rollback requires an expected workspace revision".into())
        })?;
        let prepared = self.repository.metadata_mut().prepare_rollback(request)?;
        let workspace = self
            .repository
            .metadata()
            .workspace(&prepared.workspace_id)?
            .ok_or_else(|| PongError::NotFound("rollback workspace does not exist".into()))?;
        if workspace.driver != "local" {
            return Err(PongError::Unsupported(
                "rollback is only implemented for the local workspace driver".into(),
            ));
        }
        if prepared.status == "prepared" {
            let lease = request.lease.as_ref().ok_or_else(|| {
                PongError::Conflict("rollback requires a current workspace lease".into())
            })?;
            let lease_record = self
                .repository
                .metadata()
                .workspace_lease(&prepared.workspace_id)?
                .ok_or_else(|| PongError::Conflict("workspace lease is missing".into()))?;
            if lease_record.agent_id.as_deref() != Some(lease.agent_id.as_str())
                || lease_record.epoch != lease.epoch
                || lease_record.expires_at_ms <= request.now_ms.unwrap_or(i64::MAX)
            {
                return Err(PongError::Conflict(
                    "workspace lease is stale or expired".into(),
                ));
            }
        }
        let target_version = self
            .repository
            .metadata()
            .version_record(&prepared.target_version_id)?
            .ok_or_else(|| PongError::Integrity("rollback target Version disappeared".into()))?;
        if target_version.workspace_id != workspace.workspace_id {
            return self.rollback_foreign_source(
                request,
                prepared,
                workspace,
                target_version,
                expected_revision,
            );
        }
        let snapshot = self
            .repository
            .metadata()
            .snapshot_record(&target_version.snapshot_id)?
            .ok_or_else(|| PongError::Integrity("rollback target Snapshot disappeared".into()))?;
        let digest_text = snapshot
            .root_digest
            .strip_prefix("sha256:")
            .ok_or_else(|| {
                PongError::Integrity("rollback Snapshot digest is not prefixed".into())
            })?;
        let digest = Digest::from_hex(digest_text)?;
        if snapshot.snapshot_id != format!("snp-{digest}")
            || snapshot.workspace_id != workspace.workspace_id
            || snapshot.project_id != workspace.project_id
        {
            return Err(PongError::Integrity(
                "rollback target Snapshot binding is inconsistent".into(),
            ));
        }
        let local = LocalWorkspace::open_with_failpoint_state(
            &workspace.workspace_id,
            &workspace.project_id,
            PathBuf::from(&workspace.locator),
            self.redactor.clone(),
            Arc::clone(&self.failpoints),
        )?;
        local.replace_with_snapshot(self.repository.cas(), digest)?;
        local.verify_materialized(self.repository.cas(), digest, local.root())?;
        let result_version_head = if target_version.workspace_id == workspace.workspace_id {
            Some(target_version.version_id.as_str())
        } else {
            workspace.version_head_id.as_deref()
        };
        self.repository
            .metadata_mut()
            .complete_rollback(RollbackCompletionInput {
                rollback_id: &prepared.rollback_id,
                workspace_id: &prepared.workspace_id,
                target_version_id: &prepared.target_version_id,
                target_workspace_head: &snapshot.root_digest,
                source_version_id: Some(&target_version.version_id),
                source_snapshot_id: Some(&snapshot.snapshot_id),
                previous_workspace_head: workspace.head.as_deref(),
                previous_version_head: workspace.version_head_id.as_deref(),
                result_version_head,
                lease: request.lease.as_ref().ok_or_else(|| {
                    PongError::Conflict("rollback requires a current workspace lease".into())
                })?,
                expected_revision,
                updated_at: &request.created_at,
                now_ms: request.now_ms.unwrap_or(i64::MAX),
            })?;
        self.repository
            .metadata()
            .rollback_record(&prepared.rollback_id)?
            .ok_or_else(|| {
                PongError::Integrity("rollback record disappeared after completion".into())
            })
    }

    fn rollback_foreign_source(
        &mut self,
        request: &RollbackCreation,
        prepared: RollbackRecord,
        workspace: WorkspaceRecord,
        source_version: crate::metadata::VersionRecord,
        expected_revision: i64,
    ) -> Result<RollbackRecord, PongError> {
        let lease = request.lease.as_ref().ok_or_else(|| {
            PongError::Conflict("rollback requires a current workspace lease".into())
        })?;
        let (_version, source_snapshot, source_digest, source_manifest) =
            self.source_manifest_for_target(&workspace, &source_version.version_id)?;
        let local = LocalWorkspace::open_with_failpoint_state(
            &workspace.workspace_id,
            &workspace.project_id,
            PathBuf::from(&workspace.locator),
            self.redactor.clone(),
            Arc::clone(&self.failpoints),
        )?;
        let (target_digest, target_manifest) = local.rebind_manifest_for_workspace(
            self.repository.cas(),
            source_digest,
            &source_version.workspace_id,
            &source_version.project_id,
            &workspace.workspace_id,
            &workspace.project_id,
        )?;
        verify_manifest_blobs(self.repository.cas(), &source_manifest)?;
        let target_head = format!("sha256:{target_digest}");
        let target_snapshot_id = format!("snp-{target_digest}");

        // A retry after local Snapshot publication must not repeat the
        // filesystem replacement or increment the target revision again.
        let already_published = workspace.revision == expected_revision + 1
            && workspace.head.as_deref() == Some(target_head.as_str())
            && self
                .repository
                .metadata()
                .snapshot_record(&target_snapshot_id)?
                .is_some();
        let publication_revision = if already_published {
            expected_revision + 1
        } else {
            if workspace.revision != expected_revision {
                return Err(PongError::Conflict("workspace revision is stale".into()));
            }
            local.replace_with_snapshot(self.repository.cas(), target_digest)?;
            local.verify_materialized(self.repository.cas(), target_digest, local.root())?;
            let snapshot = self.snapshot_local(
                &workspace.workspace_id,
                lease,
                SnapshotOptions::default(),
                request.now_ms.unwrap_or(i64::MAX),
                &request.created_at,
            )?;
            let published_manifest = local.read_manifest(self.repository.cas(), snapshot.digest)?;
            if published_manifest != target_manifest
                || snapshot.snapshot_id != target_snapshot_id
                || snapshot.workspace_id != workspace.workspace_id
            {
                return Err(PongError::Integrity(
                    "foreign rollback Snapshot publication is inconsistent".into(),
                ));
            }
            expected_revision + 1
        };
        self.repository.metadata_mut().complete_published_rollback(
            PublishedRollbackCompletionInput {
                rollback_id: &prepared.rollback_id,
                workspace_id: &workspace.workspace_id,
                target_version_id: &source_version.version_id,
                target_workspace_head: &target_head,
                source_version_id: Some(&source_version.version_id),
                source_snapshot_id: Some(&source_snapshot.snapshot_id),
                previous_workspace_head: workspace.head.as_deref(),
                previous_version_head: workspace.version_head_id.as_deref(),
                result_version_head: workspace.version_head_id.as_deref(),
                lease,
                expected_revision: publication_revision,
                updated_at: &request.created_at,
                now_ms: request.now_ms.unwrap_or(i64::MAX),
            },
        )?;
        self.repository
            .metadata()
            .rollback_record(&prepared.rollback_id)?
            .ok_or_else(|| {
                PongError::Integrity("rollback record disappeared after completion".into())
            })
    }

    /// Apply one provider-neutral lifecycle action through the existing
    /// lease- and revision-guarded workspace update transaction.
    ///
    /// `Open` is read-like and never advances durable revision. For mutating
    /// actions, a state that already equals the deterministic action result is
    /// recognized as an exact retry and returned without a second revision
    /// increment. No operation or event is synthesized here; callers that
    /// already have an operation contract continue to use the existing ledger.
    pub fn transition_lifecycle(
        &mut self,
        workspace_id: &str,
        lease: &LeaseToken,
        expected_revision: i64,
        action: WorkspaceLifecycleAction,
        now_ms: i64,
        updated_at: &str,
    ) -> Result<WorkspaceRecord, PongError> {
        let record = self
            .repository
            .metadata()
            .workspace(workspace_id)?
            .ok_or_else(|| PongError::NotFound("workspace does not exist".into()))?;
        if expected_revision < 0 {
            return Err(PongError::InvalidInput(
                "workspace revision must not be negative".into(),
            ));
        }
        if expected_revision > record.revision {
            return Err(PongError::Conflict("workspace revision is stale".into()));
        }
        let current = WorkspaceLifecycleState::parse(&record.status)?;
        let next = transition_workspace_lifecycle(current, action)?;

        if action == WorkspaceLifecycleAction::Open {
            if expected_revision != record.revision {
                return Err(PongError::Conflict("workspace revision is stale".into()));
            }
            return Ok(record);
        }

        self.validate_transition_lease(workspace_id, lease, now_ms)?;

        // A transition that already reached its deterministic target is a
        // completed retry. Accept the same revision or exactly the preceding
        // caller revision, but reject older stale callers.
        if current == next {
            let same_revision_retry = expected_revision == record.revision;
            let completed_transition_retry = expected_revision.checked_add(1)
                == Some(record.revision)
                && matches!(
                    (action, current),
                    (
                        WorkspaceLifecycleAction::CompleteCapture,
                        WorkspaceLifecycleState::Ready
                    ) | (
                        WorkspaceLifecycleAction::Activate,
                        WorkspaceLifecycleState::Active
                    ) | (
                        WorkspaceLifecycleAction::Pause,
                        WorkspaceLifecycleState::Paused
                    ) | (
                        WorkspaceLifecycleAction::Resume,
                        WorkspaceLifecycleState::Active
                    ) | (
                        WorkspaceLifecycleAction::CompleteRecovery,
                        WorkspaceLifecycleState::Ready
                    ) | (
                        WorkspaceLifecycleAction::Close,
                        WorkspaceLifecycleState::Archived
                    )
                );
            if same_revision_retry || completed_transition_retry {
                return Ok(record);
            }
        }
        if expected_revision != record.revision {
            return Err(PongError::Conflict("workspace revision is stale".into()));
        }

        self.repository
            .metadata_mut()
            .update_workspace(WorkspaceUpdate {
                workspace_id,
                expected_revision,
                lease,
                branch_ref: record.branch_ref.as_deref(),
                head: record.head.as_deref(),
                environment_id: record.environment_id.as_deref(),
                status: next.as_str(),
                updated_at,
                now_ms,
            })
    }

    /// Execute a mutating lifecycle action with one durable operation
    /// identity. Operation intent, workspace state, terminal result, and the
    /// operation event stream are committed atomically by the metadata store.
    /// Read-like `Open` is deliberately excluded from this API.
    pub fn transition_lifecycle_operation(
        &mut self,
        workspace_id: &str,
        lease: &LeaseToken,
        expected_revision: i64,
        action: WorkspaceLifecycleAction,
        options: WorkspaceLifecycleOperationOptions,
    ) -> Result<(WorkspaceRecord, crate::metadata::OperationRecord), PongError> {
        if action == WorkspaceLifecycleAction::Open {
            return Err(PongError::InvalidInput(
                "read-like workspace open cannot create an operation".into(),
            ));
        }
        let record = self
            .repository
            .metadata()
            .workspace(workspace_id)?
            .ok_or_else(|| PongError::NotFound("workspace does not exist".into()))?;
        if let Some(existing) = self.repository.metadata().operation_record_for_request(
            &record.project_id,
            &lease.agent_id,
            &options.request_id,
        )? {
            if existing.operation_id != options.operation_id.as_str() {
                return Err(PongError::IdempotencyKeyReuse(options.request_id.clone()));
            }
            if existing.action != lifecycle_operation_action(action) {
                return Err(PongError::IdempotencyKeyReuse(options.request_id.clone()));
            }
            let recorded_revision = existing
                .resource
                .as_ref()
                .and_then(|resource| resource.get("expected_revision"))
                .and_then(Value::as_i64);
            if recorded_revision != Some(expected_revision) {
                return Err(PongError::IdempotencyKeyReuse(options.request_id.clone()));
            }
            match existing.lifecycle_status.as_str() {
                "completed" => {
                    let result = existing.result.as_ref().ok_or_else(|| {
                        PongError::Integrity("completed lifecycle operation has no result".into())
                    })?;
                    if result.get("workspace_id").and_then(Value::as_str) != Some(workspace_id)
                        || result.get("action").and_then(Value::as_str)
                            != Some(lifecycle_operation_action(action))
                    {
                        return Err(PongError::Integrity(
                            "completed lifecycle operation result is inconsistent".into(),
                        ));
                    }
                    return Ok((record, existing));
                }
                "started" => {
                    return Err(PongError::RecoveryRequired(
                        "lifecycle operation is still pending recovery".into(),
                    ));
                }
                _ => {
                    return Err(PongError::Conflict(
                        "lifecycle operation already has a terminal non-success outcome".into(),
                    ));
                }
            }
        }
        let current = WorkspaceLifecycleState::parse(&record.status)?;
        let next = transition_workspace_lifecycle(current, action)?;
        let action_name = lifecycle_operation_action(action);
        let operation = OperationEnvelope {
            operation_id: options.operation_id,
            project_id: record.project_id.clone(),
            request_id: options.request_id,
            agent_id: lease.agent_id.clone(),
            session_id: format!("workspace:{workspace_id}"),
            workspace_id: Some(workspace_id.into()),
            environment_id: record.environment_id.clone(),
            parent_operation_id: None,
            schema_version: crate::metadata::OPERATION_SCHEMA_VERSION.into(),
            started_at: options.updated_at.clone(),
            tool: "workspace".into(),
            action: action_name.into(),
            input_refs: Vec::new(),
            output_refs: Vec::new(),
            resource: Some(serde_json::json!({
                "workspace_id": workspace_id,
                "action": action_name,
                "expected_revision": expected_revision,
                "expected_status": current.as_str(),
                "next_status": next.as_str(),
            })),
            before_state: Some(serde_json::json!({
                "status": current.as_str(),
                "revision": expected_revision,
            })),
            after_state: None,
            reversibility: "REVERSIBLE".into(),
            replayability: "REPLAYABLE".into(),
            side_effect: "WORKSPACE".into(),
            policy_decision: None,
        };
        self.repository
            .metadata_mut()
            .apply_workspace_lifecycle_operation(WorkspaceLifecycleOperationInput {
                operation,
                lease,
                expected_revision,
                expected_status: current.as_str(),
                next_status: next.as_str(),
                updated_at: &options.updated_at,
                now_ms: options.now_ms,
            })
    }

    fn validate_transition_lease(
        &self,
        workspace_id: &str,
        lease: &LeaseToken,
        now_ms: i64,
    ) -> Result<(), PongError> {
        if lease.workspace_id != workspace_id {
            return Err(PongError::Conflict(
                "lease belongs to another workspace".into(),
            ));
        }
        let lease_record = self
            .repository
            .metadata()
            .workspace_lease(workspace_id)?
            .ok_or_else(|| PongError::Conflict("workspace lease is missing".into()))?;
        if lease_record.agent_id.as_deref() != Some(lease.agent_id.as_str())
            || lease_record.epoch != lease.epoch
            || lease_record.expires_at_ms <= now_ms
        {
            return Err(PongError::Conflict(
                "workspace lease is stale or expired".into(),
            ));
        }
        Ok(())
    }

    pub fn snapshot_local(
        &mut self,
        workspace_id: &str,
        lease: &LeaseToken,
        options: SnapshotOptions,
        now_ms: i64,
        now: &str,
    ) -> Result<Snapshot, PongError> {
        let record = self
            .repository
            .metadata()
            .workspace(workspace_id)?
            .ok_or_else(|| PongError::NotFound("workspace does not exist".into()))?;
        if record.driver != "local" {
            return Err(PongError::Unsupported(
                "workspace is not bound to the local driver".into(),
            ));
        }
        let lease_record = self
            .repository
            .metadata()
            .workspace_lease(workspace_id)?
            .ok_or_else(|| PongError::Conflict("workspace lease is missing".into()))?;
        if lease_record.agent_id.as_deref() != Some(lease.agent_id.as_str())
            || lease_record.epoch != lease.epoch
            || lease_record.expires_at_ms <= now_ms
        {
            return Err(PongError::Conflict(
                "workspace lease is stale or expired".into(),
            ));
        }
        let workspace_path = PathBuf::from(&record.locator);
        let resolved_locator = local_locator(self.repository.root(), &workspace_path)?;
        if resolved_locator != record.locator {
            return Err(PongError::Integrity(
                "workspace locator changed or is not canonical".into(),
            ));
        }
        let workspace = LocalWorkspace::open_with_failpoint_state(
            &record.workspace_id,
            &record.project_id,
            workspace_path,
            self.redactor.clone(),
            Arc::clone(&self.failpoints),
        )?;
        let snapshot = workspace.snapshot(self.repository.cas(), options)?;
        let head = format!("sha256:{}", snapshot.digest);
        if let Some(existing) = self
            .repository
            .metadata()
            .snapshot_record(&snapshot.snapshot_id)?
        {
            let profile = self.redactor.profile();
            let metadata_matches = existing.root_digest == head
                && existing.workspace_id == record.workspace_id
                && existing.project_id == record.project_id
                && existing.environment_id == record.environment_id
                && existing.manifest_version == TREE_MANIFEST_VERSION
                && existing.redaction_profile_id == profile.id
                && existing.redaction_profile_version == profile.version
                && existing.file_count == snapshot.file_count
                && existing.total_bytes == snapshot.total_bytes
                && record.head.as_deref() == Some(existing.root_digest.as_str());
            if !metadata_matches {
                return Err(PongError::Integrity(
                    "existing snapshot does not match the current workspace publication".into(),
                ));
            }
            let event = self
                .repository
                .metadata()
                .list_event_envelopes(&record.project_id, 0)?
                .into_iter()
                .find(|event| event.event_id == existing.event_id)
                .ok_or_else(|| {
                    PongError::Integrity(
                        "existing snapshot metadata has no publication event".into(),
                    )
                })?;
            if event.event_type != "snapshot.created"
                || event.operation_id.as_deref() != Some(existing.operation_id.as_str())
                || event.workspace_id.as_deref() != Some(existing.workspace_id.as_str())
            {
                return Err(PongError::Integrity(
                    "existing snapshot publication event is inconsistent".into(),
                ));
            }
            let operation = self
                .repository
                .metadata()
                .operation_record(&existing.operation_id)?
                .ok_or_else(|| {
                    PongError::Integrity(
                        "existing snapshot metadata has no publication operation".into(),
                    )
                })?;
            if operation.project_id != existing.project_id
                || operation.workspace_id.as_deref() != Some(existing.workspace_id.as_str())
                || operation.environment_id != existing.environment_id
                || operation.action != "snapshot.create"
            {
                return Err(PongError::Integrity(
                    "existing snapshot publication operation is inconsistent".into(),
                ));
            }
            match operation.lifecycle_status.as_str() {
                "completed" => return Ok(snapshot),
                "started" => {
                    self.repository.metadata_mut().finish_operation(
                        &existing.operation_id,
                        OperationOutcome {
                            status: "completed".into(),
                            finished_at: now.into(),
                            output_refs: Some(vec![OperationRef {
                                kind: "snapshot".into(),
                                reference: existing.snapshot_id,
                                media_type: Some("application/vnd.pong.snapshot".into()),
                            }]),
                            after_state: Some(serde_json::json!({"head": head})),
                            result: Some(serde_json::json!({"snapshot_id": snapshot.snapshot_id})),
                            error: None,
                        },
                    )?;
                    return Ok(snapshot);
                }
                _ => {
                    return Err(PongError::Integrity(
                        "existing snapshot operation is not a successful publication".into(),
                    ));
                }
            }
        }
        let operation_id = format!("operation:{workspace_id}:{}", snapshot.digest);
        let operation = OperationEnvelope {
            operation_id: operation_id.clone(),
            project_id: record.project_id.clone(),
            request_id: operation_id.clone(),
            agent_id: lease.agent_id.clone(),
            session_id: format!("workspace:{workspace_id}"),
            workspace_id: Some(workspace_id.to_owned()),
            environment_id: record.environment_id.clone(),
            parent_operation_id: None,
            schema_version: crate::metadata::OPERATION_SCHEMA_VERSION.into(),
            started_at: now.into(),
            tool: "workspace".into(),
            action: "snapshot.create".into(),
            input_refs: vec![OperationRef {
                kind: "tree".into(),
                reference: head.clone(),
                media_type: Some("application/vnd.pong.tree+json".into()),
            }],
            output_refs: Vec::new(),
            resource: Some(serde_json::json!({"workspace_id": workspace_id})),
            before_state: record
                .head
                .clone()
                .map(|value| serde_json::json!({"head": value})),
            after_state: None,
            reversibility: "REVERSIBLE".into(),
            replayability: "REPLAYABLE".into(),
            side_effect: "WORKSPACE".into(),
            policy_decision: None,
        };
        self.repository.metadata_mut().start_operation(operation)?;
        let event_id = format!("snapshot:{snapshot_id}", snapshot_id = snapshot.snapshot_id);
        let publication = SnapshotPublication {
            snapshot_id: snapshot.snapshot_id.clone(),
            root_digest: head.clone(),
            workspace_id: workspace_id.to_owned(),
            project_id: record.project_id.clone(),
            environment_id: record.environment_id.clone(),
            manifest_version: 1,
            file_count: snapshot.file_count,
            total_bytes: snapshot.total_bytes,
            created_at: now.into(),
            operation_id: operation_id.clone(),
            event_id,
            causation_id: Some(format!("operation:{operation_id}:1")),
            correlation_id: Some(operation_id.clone()),
            expected_revision: record.revision,
            lease: lease.clone(),
            now_ms,
        };
        let metadata = self
            .repository
            .metadata_mut()
            .publish_snapshot(publication)?;
        self.repository.metadata_mut().finish_operation(
            &operation_id,
            OperationOutcome {
                status: "completed".into(),
                finished_at: now.into(),
                output_refs: Some(vec![OperationRef {
                    kind: "snapshot".into(),
                    reference: metadata.snapshot_id.clone(),
                    media_type: Some("application/vnd.pong.snapshot".into()),
                }]),
                after_state: Some(serde_json::json!({"head": head})),
                result: Some(serde_json::json!({"snapshot_id": metadata.snapshot_id})),
                error: None,
            },
        )?;
        Ok(snapshot)
    }

    /// Restore one verified snapshot to a new destination and durably record
    /// the operation outcome together with its domain event.
    pub fn restore_local(
        &mut self,
        workspace_id: &str,
        snapshot_id: &str,
        destination: impl AsRef<Path>,
        options: RestoreOptions,
    ) -> Result<RestoreResult, PongError> {
        let record = self
            .repository
            .metadata()
            .workspace(workspace_id)?
            .ok_or_else(|| PongError::NotFound("workspace does not exist".into()))?;
        if record.driver != "local" {
            return Err(PongError::Unsupported(
                "workspace is not bound to the local driver".into(),
            ));
        }
        let snapshot = self
            .repository
            .metadata()
            .snapshot_record(snapshot_id)?
            .ok_or_else(|| PongError::NotFound("snapshot does not exist".into()))?;
        if snapshot.workspace_id != record.workspace_id || snapshot.project_id != record.project_id
        {
            return Err(PongError::Conflict(
                "snapshot is not bound to the requested workspace".into(),
            ));
        }
        let root_digest = snapshot
            .root_digest
            .strip_prefix("sha256:")
            .ok_or_else(|| PongError::Integrity("snapshot root digest is not prefixed".into()))?;
        let digest = Digest::from_hex(root_digest)?;
        if snapshot.snapshot_id != format!("snp-{digest}") {
            return Err(PongError::Integrity(
                "snapshot identity does not match its root digest".into(),
            ));
        }
        let destination = destination.as_ref().to_path_buf();
        let destination_locator = local_locator(self.repository.root(), &destination)?;
        let destination = PathBuf::from(&destination_locator);
        let destination_string = destination_locator.clone();
        let workspace = LocalWorkspace::open_with_failpoint_state(
            &record.workspace_id,
            &record.project_id,
            PathBuf::from(&record.locator),
            self.redactor.clone(),
            Arc::clone(&self.failpoints),
        )?;
        // Reading the manifest before starting the operation rejects missing,
        // wrong-domain, and wrong-identity CAS objects without any durable
        // restore intent being created.
        let manifest = workspace.read_manifest(self.repository.cas(), digest)?;
        let manifest_file_count = manifest
            .entries
            .iter()
            .filter(|entry| entry.kind == "file")
            .count();
        let manifest_total_bytes = manifest
            .entries
            .iter()
            .filter(|entry| entry.kind == "file")
            .try_fold(0u64, |total, entry| total.checked_add(entry.size))
            .ok_or_else(|| {
                PongError::ResourceExhausted("snapshot total size exceeds supported range".into())
            })?;
        if snapshot.manifest_version != 1
            || snapshot.file_count != manifest_file_count
            || snapshot.total_bytes != manifest_total_bytes
        {
            return Err(PongError::Integrity(
                "snapshot metadata does not match its manifest".into(),
            ));
        }
        let current_identity = self.repository.metadata().generation_identity()?;
        let expected_identity =
            current_identity.unwrap_or_else(|| ("legacy-v0.1".into(), "legacy".into()));
        if snapshot.generation_id != expected_identity.0
            || snapshot.migration_id != expected_identity.1
        {
            return Err(PongError::Integrity(
                "snapshot generation identity is incompatible with the opened repository".into(),
            ));
        }

        let prior_operation = self.repository.metadata().operation_record_for_request(
            &record.project_id,
            &options.agent_id,
            &options.request_id,
        )?;

        let operation = OperationEnvelope {
            operation_id: options.operation_id.clone(),
            project_id: record.project_id.clone(),
            request_id: options.request_id.clone(),
            agent_id: options.agent_id.clone(),
            session_id: format!("workspace:{workspace_id}"),
            workspace_id: Some(workspace_id.to_owned()),
            environment_id: record.environment_id.clone(),
            parent_operation_id: None,
            schema_version: crate::metadata::OPERATION_SCHEMA_VERSION.into(),
            started_at: prior_operation
                .as_ref()
                .map(|operation| operation.started_at.clone())
                .unwrap_or_else(|| options.now.clone()),
            tool: "workspace".into(),
            action: "snapshot.restore".into(),
            input_refs: vec![OperationRef {
                kind: "snapshot".into(),
                reference: snapshot.snapshot_id.clone(),
                media_type: Some("application/vnd.pong.snapshot".into()),
            }],
            output_refs: Vec::new(),
            resource: Some(serde_json::json!({
                "workspace_id": workspace_id,
                "destination": destination_string,
            })),
            before_state: None,
            after_state: None,
            reversibility: "REVERSIBLE".into(),
            replayability: "REPLAYABLE".into(),
            side_effect: "WORKSPACE".into(),
            policy_decision: None,
        };
        let existing = self.repository.metadata_mut().start_operation(operation)?;
        if existing.action != "snapshot.restore"
            || existing.workspace_id.as_deref() != Some(workspace_id)
            || existing.project_id != record.project_id
        {
            return Err(PongError::Integrity(
                "restore operation identity is inconsistent".into(),
            ));
        }

        if existing.lifecycle_status == "completed" || existing.lifecycle_status == "unknown" {
            let status = existing.lifecycle_status.clone();
            let result = existing
                .result
                .clone()
                .or_else(|| {
                    existing
                        .error
                        .as_ref()
                        .and_then(|error| error.details.clone())
                })
                .ok_or_else(|| {
                    PongError::Integrity("terminal restore operation has no result".into())
                })?;
            let result_snapshot = result
                .get("snapshot_id")
                .and_then(Value::as_str)
                .ok_or_else(|| PongError::Integrity("restore result has no snapshot id".into()))?;
            let result_destination = result
                .get("destination")
                .and_then(Value::as_str)
                .ok_or_else(|| PongError::Integrity("restore result has no destination".into()))?;
            if result_snapshot != snapshot.snapshot_id || result_destination != destination_string {
                return Err(PongError::Integrity(
                    "restore operation result does not match the request".into(),
                ));
            }
            workspace.verify_materialized(self.repository.cas(), digest, &destination)?;
            return Ok(RestoreResult {
                operation_id: existing.operation_id,
                snapshot_id: snapshot.snapshot_id,
                destination: destination_string,
                status,
            });
        }
        if existing.lifecycle_status == "failed" {
            return Err(PongError::Conflict(
                "restore operation previously failed".into(),
            ));
        }
        if existing.lifecycle_status != "started" {
            return Err(PongError::Integrity(
                "restore operation has an unsupported lifecycle state".into(),
            ));
        }

        if prior_operation.is_none() && destination.exists() {
            let conflict = PongError::Conflict("materialization destination already exists".into());
            let _ = self.finish_restore_failed(&snapshot, &options, &destination_string, &conflict);
            return Err(conflict);
        }

        if destination.exists() {
            workspace.verify_materialized(self.repository.cas(), digest, &destination)?;
            return self.finish_restore_completed(
                &workspace,
                &snapshot,
                &options,
                &destination_string,
                &digest,
            );
        }

        let materialized = workspace.materialize(self.repository.cas(), digest, &destination);
        match materialized {
            Ok(_) => self.finish_restore_completed(
                &workspace,
                &snapshot,
                &options,
                &destination_string,
                &digest,
            ),
            Err(error) => {
                let published = destination.exists();
                let valid_published = published
                    && workspace
                        .verify_materialized(self.repository.cas(), digest, &destination)
                        .is_ok();
                if valid_published {
                    let _ = self.finish_restore_unknown(
                        &snapshot,
                        &options,
                        &destination_string,
                        &error,
                    );
                } else {
                    let _ = self.finish_restore_failed(
                        &snapshot,
                        &options,
                        &destination_string,
                        &error,
                    );
                }
                Err(error)
            }
        }
    }

    fn finish_restore_completed(
        &mut self,
        workspace: &LocalWorkspace,
        snapshot: &crate::metadata::SnapshotRecord,
        options: &RestoreOptions,
        destination: &str,
        digest: &Digest,
    ) -> Result<RestoreResult, PongError> {
        workspace.verify_materialized(self.repository.cas(), *digest, destination)?;
        let result = serde_json::json!({
            "snapshot_id": snapshot.snapshot_id,
            "destination": destination,
            "status": "completed",
        });
        let event = self.restore_event(
            snapshot,
            options,
            destination,
            "snapshot.restore.completed",
            result.clone(),
        );
        self.repository.metadata_mut().finish_operation_with_event(
            &options.operation_id,
            OperationOutcome {
                status: "completed".into(),
                finished_at: options.now.clone(),
                output_refs: Some(vec![OperationRef {
                    kind: "restore".into(),
                    reference: destination.into(),
                    media_type: Some("application/vnd.pong.workspace+directory".into()),
                }]),
                after_state: Some(serde_json::json!({"destination": destination})),
                result: Some(result),
                error: None,
            },
            event,
        )?;
        Ok(RestoreResult {
            operation_id: options.operation_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            destination: destination.into(),
            status: "completed".into(),
        })
    }

    fn finish_restore_failed(
        &mut self,
        snapshot: &crate::metadata::SnapshotRecord,
        options: &RestoreOptions,
        destination: &str,
        error: &PongError,
    ) -> Result<(), PongError> {
        let result = serde_json::json!({"snapshot_id": snapshot.snapshot_id, "destination": destination, "status": "failed"});
        let event = self.restore_event(
            snapshot,
            options,
            destination,
            "snapshot.restore.failed",
            result,
        );
        self.repository.metadata_mut().finish_operation_with_event(
            &options.operation_id,
            OperationOutcome {
                status: "failed".into(),
                finished_at: options.now.clone(),
                output_refs: None,
                after_state: None,
                result: None,
                error: Some(operation_error(error)),
            },
            event,
        )?;
        Ok(())
    }

    fn finish_restore_unknown(
        &mut self,
        snapshot: &crate::metadata::SnapshotRecord,
        options: &RestoreOptions,
        destination: &str,
        error: &PongError,
    ) -> Result<(), PongError> {
        let result = serde_json::json!({"snapshot_id": snapshot.snapshot_id, "destination": destination, "status": "unknown"});
        let event = self.restore_event(
            snapshot,
            options,
            destination,
            "snapshot.restore.unknown",
            result.clone(),
        );
        self.repository.metadata_mut().finish_operation_with_event(
            &options.operation_id,
            OperationOutcome {
                status: "unknown".into(),
                finished_at: options.now.clone(),
                output_refs: Some(vec![OperationRef {
                    kind: "restore".into(),
                    reference: destination.into(),
                    media_type: Some("application/vnd.pong.workspace+directory".into()),
                }]),
                after_state: Some(serde_json::json!({"destination": destination})),
                result: None,
                error: Some(OperationError {
                    code: "OUTCOME_UNKNOWN".into(),
                    message: error.to_string(),
                    retryable: false,
                    details: Some(serde_json::json!({
                        "snapshot_id": snapshot.snapshot_id,
                        "destination": destination,
                        "status": "unknown",
                        "materialization_error": error.code()
                    })),
                    safe_to_expose: true,
                }),
            },
            event,
        )?;
        Ok(())
    }

    fn restore_event(
        &self,
        snapshot: &crate::metadata::SnapshotRecord,
        options: &RestoreOptions,
        destination: &str,
        event_type: &str,
        payload: Value,
    ) -> NewEventEnvelope {
        NewEventEnvelope {
            event_id: format!("restore:{}:{}", options.operation_id, event_type),
            project_id: snapshot.project_id.clone(),
            stream_id: format!("workspace:{}", snapshot.workspace_id),
            event_type: event_type.into(),
            schema_version: crate::metadata::EVENT_ENVELOPE_SCHEMA_VERSION.into(),
            occurred_at: options.now.clone(),
            recorded_at: options.now.clone(),
            actor_id: Some(options.agent_id.clone()),
            workspace_id: Some(snapshot.workspace_id.clone()),
            task_id: None,
            operation_id: Some(options.operation_id.clone()),
            causation_id: Some(format!("operation:{}:1", options.operation_id)),
            correlation_id: Some(options.operation_id.clone()),
            parent_event_ids: vec![format!("operation:{}:1", options.operation_id)],
            capture_confidence: Some("observed".into()),
            redaction_status: "redacted".into(),
            generation_id: Some(snapshot.generation_id.clone()),
            migration_id: Some(snapshot.migration_id.clone()),
            payload: serde_json::json!({"snapshot_id": snapshot.snapshot_id, "destination": destination, "result": payload}),
        }
    }

    pub fn workspace(&self, workspace_id: &str) -> Result<Option<WorkspaceRecord>, PongError> {
        self.repository.metadata().workspace(workspace_id)
    }

    /// Compare a local workspace's current point-in-time tree with its
    /// durable head snapshot. A workspace without a head is an integrity
    /// failure; no alternate snapshot is selected implicitly.
    pub fn diff_workspace(&self, workspace_id: &str) -> Result<WorkspaceDiffResult, PongError> {
        let record = self
            .repository
            .metadata()
            .workspace(workspace_id)?
            .ok_or_else(|| PongError::NotFound("workspace does not exist".into()))?;
        if record.driver != "local" {
            return Err(PongError::Unsupported(
                "workspace diff is not implemented for this driver".into(),
            ));
        }
        if let Some(environment_id) = record.environment_id.as_deref() {
            let environment = self.repository.metadata().environment(environment_id)?;
            if environment.as_ref().map_or(true, |environment| {
                environment.project_id != record.project_id
            }) {
                return Err(PongError::Integrity(
                    "workspace environment binding is missing or belongs to another project".into(),
                ));
            }
        }
        let head = record.head.as_deref().ok_or_else(|| {
            PongError::Integrity("workspace has no reference snapshot head".into())
        })?;
        let digest_text = head
            .strip_prefix("sha256:")
            .ok_or_else(|| PongError::Integrity("workspace head is not a sha256 digest".into()))?;
        let reference_digest = Digest::from_hex(digest_text)?;
        let snapshot_id = self
            .repository
            .metadata()
            .snapshot_id_for_root(head)?
            .ok_or_else(|| {
                PongError::Integrity("workspace head points to a missing snapshot".into())
            })?;
        let snapshot = self
            .repository
            .metadata()
            .snapshot_record(&snapshot_id)?
            .ok_or_else(|| {
                PongError::Integrity("workspace head snapshot metadata is missing".into())
            })?;
        let expected_snapshot_id = format!("snp-{reference_digest}");
        if snapshot.snapshot_id != expected_snapshot_id
            || snapshot.root_digest != head
            || snapshot.workspace_id != record.workspace_id
            || snapshot.project_id != record.project_id
            || snapshot.environment_id != record.environment_id
        {
            return Err(PongError::Integrity(
                "workspace head snapshot identity is inconsistent".into(),
            ));
        }
        let expected_identity = self
            .repository
            .metadata()
            .generation_identity()?
            .unwrap_or_else(|| ("legacy-v0.1".into(), "legacy".into()));
        if snapshot.generation_id != expected_identity.0
            || snapshot.migration_id != expected_identity.1
        {
            return Err(PongError::Integrity(
                "workspace head snapshot generation identity is incompatible".into(),
            ));
        }
        if snapshot.manifest_version != TREE_MANIFEST_VERSION {
            return Err(PongError::Integrity(
                "workspace head snapshot manifest version is incompatible".into(),
            ));
        }
        let operation = self
            .repository
            .metadata()
            .operation_record(&snapshot.operation_id)?
            .ok_or_else(|| {
                PongError::Integrity("workspace head snapshot operation is missing".into())
            })?;
        if operation.project_id != record.project_id
            || operation.workspace_id.as_deref() != Some(record.workspace_id.as_str())
            || operation.action != "snapshot.create"
        {
            return Err(PongError::Integrity(
                "workspace head snapshot operation is inconsistent".into(),
            ));
        }
        let event = self
            .repository
            .metadata()
            .list_event_envelopes(&record.project_id, 0)?
            .into_iter()
            .find(|event| event.event_id == snapshot.event_id)
            .ok_or_else(|| {
                PongError::Integrity("workspace head snapshot event is missing".into())
            })?;
        if event.event_type != "snapshot.created"
            || event.operation_id.as_deref() != Some(snapshot.operation_id.as_str())
            || event.workspace_id.as_deref() != Some(record.workspace_id.as_str())
            || !event.payload_json.contains(&snapshot.snapshot_id)
        {
            return Err(PongError::Integrity(
                "workspace head snapshot event is inconsistent".into(),
            ));
        }
        let workspace_path = PathBuf::from(&record.locator);
        let resolved_locator = local_locator(self.repository.root(), &workspace_path)?;
        if resolved_locator != record.locator {
            return Err(PongError::Integrity(
                "workspace locator changed or is not canonical".into(),
            ));
        }
        let workspace = LocalWorkspace::open(
            &record.workspace_id,
            &record.project_id,
            workspace_path,
            self.redactor.clone(),
        )?;
        let reference_manifest =
            workspace.read_manifest(self.repository.cas(), reference_digest)?;
        let file_count = reference_manifest
            .entries
            .iter()
            .filter(|entry| entry.kind == "file")
            .count();
        let total_bytes = reference_manifest
            .entries
            .iter()
            .filter(|entry| entry.kind == "file")
            .try_fold(0_u64, |total, entry| total.checked_add(entry.size))
            .ok_or_else(|| {
                PongError::ResourceExhausted(
                    "workspace diff snapshot size exceeds supported range".into(),
                )
            })?;
        if snapshot.file_count != file_count || snapshot.total_bytes != total_bytes {
            return Err(PongError::Integrity(
                "workspace head snapshot counters are inconsistent".into(),
            ));
        }
        let diff = workspace.diff_against_snapshot(self.repository.cas(), reference_digest)?;
        let after = self
            .repository
            .metadata()
            .workspace(workspace_id)?
            .ok_or_else(|| PongError::NotFound("workspace disappeared during diff".into()))?;
        if after.workspace_id != record.workspace_id
            || after.project_id != record.project_id
            || after.driver != record.driver
            || after.locator != record.locator
            || after.revision != record.revision
            || after.head != record.head
            || after.environment_id != record.environment_id
        {
            return Err(PongError::Conflict(
                "UNSTABLE_OBSERVATION: workspace head, revision, environment, or locator changed during diff".into(),
            ));
        }
        Ok(WorkspaceDiffResult {
            workspace_id: record.workspace_id,
            project_id: record.project_id,
            reference_snapshot_id: snapshot.snapshot_id,
            observation_revision: record.revision,
            environment_id: record.environment_id,
            observation_stability: "stable".into(),
            current_tree_id: diff.new_snapshot_id.clone(),
            diff,
        })
    }

    /// Return a read-only, point-in-time status view for a local workspace.
    /// The query validates the durable head/snapshot/environment relations and
    /// computes filesystem change state in memory without writing CAS,
    /// metadata, leases, revisions, or events.
    pub fn status(&self, workspace_id: &str, now_ms: i64) -> Result<WorkspaceStatus, PongError> {
        let record = self
            .repository
            .metadata()
            .workspace(workspace_id)?
            .ok_or_else(|| PongError::NotFound("workspace does not exist".into()))?;
        if record.revision < 0 {
            return Err(PongError::Integrity(
                "workspace revision is negative".into(),
            ));
        }
        if !matches!(
            record.status.as_str(),
            "created" | "preparing" | "ready" | "active" | "paused" | "reconciling" | "archived"
        ) {
            return Err(PongError::Integrity(
                "workspace status is unsupported".into(),
            ));
        }
        if matches!(record.status.as_str(), "ready" | "active" | "paused")
            && (record.head.is_none() || record.environment_id.is_none())
        {
            return Err(PongError::Integrity(
                "ready workspace is missing a head or environment".into(),
            ));
        }
        if record.driver != "local" {
            return Err(PongError::Unsupported(
                "workspace status is not implemented for this driver".into(),
            ));
        }

        let workspace_path = PathBuf::from(&record.locator);
        let resolved_locator = local_locator(self.repository.root(), &workspace_path)?;
        if resolved_locator != record.locator {
            return Err(PongError::Integrity(
                "workspace locator changed or is not canonical".into(),
            ));
        }
        let workspace = LocalWorkspace::open(
            &record.workspace_id,
            &record.project_id,
            workspace_path,
            self.redactor.clone(),
        )?;

        let lease = self
            .repository
            .metadata()
            .workspace_lease(&record.workspace_id)?
            .map(|lease: LeaseRecord| WorkspaceLeaseStatus {
                epoch: Some(lease.epoch),
                agent_id: lease.agent_id.clone(),
                expires_at_ms: Some(lease.expires_at_ms),
                active: lease.agent_id.is_some() && lease.expires_at_ms > now_ms,
            })
            .unwrap_or(WorkspaceLeaseStatus {
                epoch: None,
                agent_id: None,
                expires_at_ms: None,
                active: false,
            });

        let environment = match record.environment_id.clone() {
            Some(environment_id) => {
                let environment_record = self
                    .repository
                    .metadata()
                    .environment(&environment_id)?
                    .ok_or_else(|| {
                    PongError::Integrity("workspace environment binding is missing".into())
                })?;
                if environment_record.project_id != record.project_id {
                    return Err(PongError::Integrity(
                        "workspace environment belongs to another project".into(),
                    ));
                }
                WorkspaceEnvironmentStatus {
                    environment_id: Some(environment_id),
                    status: "bound".into(),
                }
            }
            None => WorkspaceEnvironmentStatus {
                environment_id: None,
                status: "unbound".into(),
            },
        };

        let mut head_digest = None;
        let mut head_snapshot_id = None;
        let mut changed = None;
        let mut change_state = "no_snapshot".to_owned();
        if let Some(head) = record.head.clone() {
            let digest_text = head.strip_prefix("sha256:").ok_or_else(|| {
                PongError::Integrity("workspace head is not a sha256 digest".into())
            })?;
            let digest = Digest::from_hex(digest_text)?;
            let snapshot_id = self
                .repository
                .metadata()
                .snapshot_id_for_root(&head)?
                .ok_or_else(|| {
                    PongError::Integrity("workspace head points to a missing snapshot".into())
                })?;
            let snapshot = self
                .repository
                .metadata()
                .snapshot_record(&snapshot_id)?
                .ok_or_else(|| {
                    PongError::Integrity("workspace head snapshot metadata is missing".into())
                })?;
            let expected_snapshot_id = format!("snp-{digest}");
            if snapshot.snapshot_id != expected_snapshot_id
                || snapshot.root_digest != head
                || snapshot.workspace_id != record.workspace_id
                || snapshot.project_id != record.project_id
            {
                return Err(PongError::Integrity(
                    "workspace head snapshot identity is inconsistent".into(),
                ));
            }
            let expected_identity = self
                .repository
                .metadata()
                .generation_identity()?
                .unwrap_or_else(|| ("legacy-v0.1".into(), "legacy".into()));
            if snapshot.generation_id != expected_identity.0
                || snapshot.migration_id != expected_identity.1
            {
                return Err(PongError::Integrity(
                    "workspace head snapshot generation identity is incompatible".into(),
                ));
            }
            if snapshot.environment_id != record.environment_id
                || snapshot.manifest_version != TREE_MANIFEST_VERSION
            {
                return Err(PongError::Integrity(
                    "workspace head snapshot metadata is inconsistent".into(),
                ));
            }
            let operation = self
                .repository
                .metadata()
                .operation_record(&snapshot.operation_id)?
                .ok_or_else(|| {
                    PongError::Integrity("workspace head snapshot operation is missing".into())
                })?;
            if operation.project_id != record.project_id
                || operation.workspace_id.as_deref() != Some(record.workspace_id.as_str())
                || operation.action != "snapshot.create"
            {
                return Err(PongError::Integrity(
                    "workspace head snapshot operation is inconsistent".into(),
                ));
            }
            let event = self
                .repository
                .metadata()
                .list_event_envelopes(&record.project_id, 0)?
                .into_iter()
                .find(|event| event.event_id == snapshot.event_id)
                .ok_or_else(|| {
                    PongError::Integrity("workspace head snapshot event is missing".into())
                })?;
            if event.event_type != "snapshot.created"
                || event.operation_id.as_deref() != Some(snapshot.operation_id.as_str())
                || event.workspace_id.as_deref() != Some(record.workspace_id.as_str())
                || !event.payload_json.contains(&snapshot.snapshot_id)
            {
                return Err(PongError::Integrity(
                    "workspace head snapshot event is inconsistent".into(),
                ));
            }
            let manifest = workspace.read_manifest(self.repository.cas(), digest)?;
            let file_count = manifest
                .entries
                .iter()
                .filter(|entry| entry.kind == "file")
                .count();
            let total_bytes = manifest
                .entries
                .iter()
                .filter(|entry| entry.kind == "file")
                .try_fold(0_u64, |total, entry| total.checked_add(entry.size))
                .ok_or_else(|| {
                    PongError::ResourceExhausted(
                        "workspace status snapshot size exceeds supported range".into(),
                    )
                })?;
            if snapshot.file_count != file_count || snapshot.total_bytes != total_bytes {
                return Err(PongError::Integrity(
                    "workspace head snapshot counters are inconsistent".into(),
                ));
            }
            changed =
                Some(!workspace.matches_manifest(self.repository.cas(), digest, &manifest)?);
            change_state = if changed == Some(true) {
                "changed"
            } else {
                "unchanged"
            }
            .into();
            head_digest = Some(head);
            head_snapshot_id = Some(snapshot_id);
        }

        let latest_operation = self
            .repository
            .metadata()
            .latest_operation_for_workspace(&record.workspace_id)?
            .map(|operation| WorkspaceOperationSummary {
                operation_id: operation.operation_id,
                action: operation.action,
                lifecycle_status: operation.lifecycle_status,
                recording_status: operation.recording_status,
                updated_at: operation.updated_at,
            });
        let recovery_required = self
            .repository
            .metadata()
            .has_unresolved_operation_for_workspace(&record.workspace_id)?;
        let healthy = !recovery_required && record.status != "reconciling";
        let execution_ready = matches!(record.status.as_str(), "ready" | "active")
            && head_snapshot_id.is_some()
            && environment.status == "bound"
            && !recovery_required;

        Ok(WorkspaceStatus {
            workspace_id: record.workspace_id,
            project_id: record.project_id,
            driver: record.driver,
            revision: record.revision,
            status: record.status,
            head_digest,
            head_snapshot_id,
            lease,
            environment,
            filesystem_accessible: true,
            changed,
            change_state,
            healthy,
            execution_ready,
            recovery_required,
            latest_operation,
        })
    }
}

fn collect_entries(
    root: &Path,
    relative: &Path,
    cas: &Cas,
    redactor: &Redactor,
    options: SnapshotOptions,
    entries: &mut Vec<TreeEntry>,
) -> Result<(), PongError> {
    let mut children = fs::read_dir(root)
        .map_err(PongError::from_protected_io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(PongError::from_protected_io)?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let metadata = fs::symlink_metadata(child.path()).map_err(PongError::from_protected_io)?;
        let child_relative = relative.join(child.file_name());
        let normalized = normalize_relative_path(&child_relative)?;
        redactor.assert_clean_bytes(normalized.as_bytes(), "workspace path")?;
        if is_reparse_point(&metadata) {
            return Err(PongError::Integrity(
                "workspace contains a symlink or reparse point".into(),
            ));
        }
        if metadata.is_dir() {
            entries.push(TreeEntry {
                path: normalized.clone(),
                kind: "directory".into(),
                size: 0,
                digest: None,
            });
            collect_entries(
                &child.path(),
                &child_relative,
                cas,
                redactor,
                options,
                entries,
            )?;
        } else if metadata.is_file() {
            if entries.iter().filter(|entry| entry.kind == "file").count() >= options.max_files {
                return Err(PongError::ResourceExhausted(
                    "workspace snapshot file limit exceeded".into(),
                ));
            }
            let size = metadata.len();
            if size > options.max_file_bytes {
                return Err(PongError::ResourceExhausted(
                    "workspace snapshot file size limit exceeded".into(),
                ));
            }
            let before_modified = metadata.modified().ok();
            let bytes = fs::read(child.path()).map_err(PongError::from_protected_io)?;
            let after = fs::symlink_metadata(child.path()).map_err(PongError::from_protected_io)?;
            if bytes.len() as u64 != size
                || after.len() != size
                || before_modified != after.modified().ok()
                || is_reparse_point(&after)
            {
                return Err(PongError::Integrity(
                    "workspace file changed while being read".into(),
                ));
            }
            let digest = cas.put(BLOB_DOMAIN, &bytes)?;
            entries.push(TreeEntry {
                path: normalized,
                kind: "file".into(),
                size,
                digest: Some(format!("sha256:{digest}")),
            });
        } else {
            return Err(PongError::Integrity(
                "workspace contains a non-regular filesystem entry".into(),
            ));
        }
    }
    Ok(())
}

fn verify_manifest_blobs(cas: &Cas, manifest: &TreeManifest) -> Result<(), PongError> {
    for entry in &manifest.entries {
        if entry.kind != "file" {
            continue;
        }
        let digest_text = entry
            .digest
            .as_deref()
            .ok_or_else(|| PongError::Integrity("snapshot file is missing its digest".into()))?;
        let blob_digest =
            Digest::from_hex(digest_text.strip_prefix("sha256:").unwrap_or(digest_text))?;
        let bytes = cas.get(BLOB_DOMAIN, blob_digest)?;
        if bytes.len() as u64 != entry.size || digest_for(BLOB_DOMAIN, &bytes) != blob_digest {
            return Err(PongError::Integrity(
                "snapshot file blob does not match its manifest".into(),
            ));
        }
    }
    Ok(())
}

fn collect_status_entries(
    root: &Path,
    relative: &Path,
    redactor: &Redactor,
    options: SnapshotOptions,
    entries: &mut Vec<TreeEntry>,
) -> Result<(), PongError> {
    let mut children = fs::read_dir(root)
        .map_err(PongError::from_protected_io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(PongError::from_protected_io)?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let metadata = fs::symlink_metadata(child.path()).map_err(PongError::from_protected_io)?;
        let child_relative = relative.join(child.file_name());
        let normalized = normalize_relative_path(&child_relative)?;
        redactor.assert_clean_bytes(normalized.as_bytes(), "workspace path")?;
        if is_reparse_point(&metadata) {
            return Err(PongError::Integrity(
                "workspace contains a symlink or reparse point".into(),
            ));
        }
        if metadata.is_dir() {
            entries.push(TreeEntry {
                path: normalized,
                kind: "directory".into(),
                size: 0,
                digest: None,
            });
            collect_status_entries(&child.path(), &child_relative, redactor, options, entries)?;
        } else if metadata.is_file() {
            if entries.iter().filter(|entry| entry.kind == "file").count() >= options.max_files {
                return Err(PongError::ResourceExhausted(
                    "workspace status file limit exceeded".into(),
                ));
            }
            let size = metadata.len();
            if size > options.max_file_bytes {
                return Err(PongError::ResourceExhausted(
                    "workspace status file size limit exceeded".into(),
                ));
            }
            let before_modified = metadata.modified().ok();
            let bytes = fs::read(child.path()).map_err(PongError::from_protected_io)?;
            let after = fs::symlink_metadata(child.path()).map_err(PongError::from_protected_io)?;
            if bytes.len() as u64 != size
                || after.len() != size
                || before_modified != after.modified().ok()
                || is_reparse_point(&after)
            {
                return Err(PongError::Integrity(
                    "workspace file changed while being read".into(),
                ));
            }
            redactor.assert_clean_bytes(&bytes, "workspace file")?;
            entries.push(TreeEntry {
                path: normalized,
                kind: "file".into(),
                size,
                digest: Some(format!("sha256:{}", digest_for(BLOB_DOMAIN, &bytes))),
            });
        } else {
            return Err(PongError::Integrity(
                "workspace contains a non-regular filesystem entry".into(),
            ));
        }
    }
    Ok(())
}

fn collect_materialized_paths(
    root: &Path,
    relative: &Path,
    paths: &mut Vec<String>,
) -> Result<(), PongError> {
    let mut children = fs::read_dir(root)
        .map_err(PongError::from_protected_io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(PongError::from_protected_io)?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let metadata = fs::symlink_metadata(child.path()).map_err(PongError::from_protected_io)?;
        if is_reparse_point(&metadata) {
            return Err(PongError::Integrity(
                "materialized destination contains a symlink or reparse point".into(),
            ));
        }
        let child_relative = relative.join(child.file_name());
        let normalized = normalize_relative_path(&child_relative)?;
        paths.push(normalized);
        if metadata.is_dir() {
            collect_materialized_paths(&child.path(), &child_relative, paths)?;
        } else if !metadata.is_file() {
            return Err(PongError::Integrity(
                "materialized destination contains a non-regular entry".into(),
            ));
        }
    }
    Ok(())
}

fn operation_error(error: &PongError) -> OperationError {
    OperationError {
        code: error.code().into(),
        message: error.to_string(),
        retryable: matches!(
            error,
            PongError::Io(_)
                | PongError::Sqlite(_)
                | PongError::ResourceExhausted(_)
                | PongError::PermissionDenied(_)
                | PongError::PermissionDeniedWithOsError { .. }
                | PongError::FaultInjected(_)
        ),
        details: Some(serde_json::json!({"error_code": error.code()})),
        safe_to_expose: true,
    }
}

fn diff_manifests(old: &TreeManifest, new: &TreeManifest) -> Vec<SnapshotDiffEntry> {
    let mut entries = Vec::new();
    let mut old_index = 0;
    let mut new_index = 0;

    while old_index < old.entries.len() || new_index < new.entries.len() {
        let old_entry = old.entries.get(old_index);
        let new_entry = new.entries.get(new_index);
        let ordering = match (old_entry, new_entry) {
            (Some(old_entry), Some(new_entry)) => {
                old_entry.path.as_bytes().cmp(new_entry.path.as_bytes())
            }
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => break,
        };

        let (old_entry, new_entry) = match ordering {
            std::cmp::Ordering::Less => {
                let old_entry = old_entry.expect("old entry exists for less ordering");
                old_index += 1;
                (Some(old_entry), None)
            }
            std::cmp::Ordering::Greater => {
                let new_entry = new_entry.expect("new entry exists for greater ordering");
                new_index += 1;
                (None, Some(new_entry))
            }
            std::cmp::Ordering::Equal => {
                let old_entry = old_entry.expect("old entry exists for equal ordering");
                let new_entry = new_entry.expect("new entry exists for equal ordering");
                old_index += 1;
                new_index += 1;
                (Some(old_entry), Some(new_entry))
            }
        };

        if let Some(entry) = classify_diff_entry(old_entry, new_entry) {
            entries.push(entry);
        }
    }
    entries
}

fn manifest_digest(manifest: &TreeManifest) -> Result<Digest, PongError> {
    let value = serde_json::to_value(manifest).map_err(|error| {
        PongError::Serialization(format!("cannot encode workspace tree manifest: {error}"))
    })?;
    let bytes = canonical_bytes(&value)?;
    Ok(digest_for(TREE_DOMAIN, &bytes))
}

fn classify_diff_entry(
    old: Option<&TreeEntry>,
    new: Option<&TreeEntry>,
) -> Option<SnapshotDiffEntry> {
    let (path, change_type) = match (old, new) {
        (Some(old), Some(new)) if old.kind != new.kind => {
            (old.path.clone(), SnapshotChangeType::TypeChanged)
        }
        (Some(old), Some(new))
            if old.kind == "file" && (old.digest != new.digest || old.size != new.size) =>
        {
            (old.path.clone(), SnapshotChangeType::Modified)
        }
        (Some(_), Some(_)) => return None,
        (Some(old), None) => (old.path.clone(), SnapshotChangeType::Removed),
        (None, Some(new)) => (new.path.clone(), SnapshotChangeType::Added),
        (None, None) => return None,
    };

    Some(SnapshotDiffEntry {
        path,
        change_type,
        old_digest: old.and_then(|entry| entry.digest.clone()),
        new_digest: new.and_then(|entry| entry.digest.clone()),
        old_size: old.map(|entry| entry.size),
        new_size: new.map(|entry| entry.size),
        old_type: old.map(|entry| entry.kind.clone()),
        new_type: new.map(|entry| entry.kind.clone()),
    })
}

fn validate_manifest(
    manifest: &TreeManifest,
    workspace_id: &str,
    project_id: &str,
    redactor: &Redactor,
) -> Result<(), PongError> {
    if manifest.manifest_version != TREE_MANIFEST_VERSION
        || manifest.workspace_id != workspace_id
        || manifest.project_id != project_id
        || manifest.redaction_profile_id != redactor.profile_id()
        || manifest.redaction_profile_version != redactor.version()
    {
        return Err(PongError::Integrity(
            "workspace tree manifest identity is incompatible".into(),
        ));
    }
    let mut previous: Option<&str> = None;
    for entry in &manifest.entries {
        let normalized = normalize_relative_path(Path::new(&entry.path))
            .map_err(|_| PongError::Integrity("workspace tree manifest path is unsafe".into()))?;
        if normalized != entry.path || previous.is_some_and(|value| value >= entry.path.as_str()) {
            return Err(PongError::Integrity(
                "workspace tree manifest paths are not canonical".into(),
            ));
        }
        redactor.assert_clean_bytes(entry.path.as_bytes(), "workspace manifest path")?;
        match entry.kind.as_str() {
            "directory" if entry.size == 0 && entry.digest.is_none() => {}
            "file" => {
                if entry.digest.is_none() {
                    return Err(PongError::Integrity(
                        "workspace tree manifest file lacks digest".into(),
                    ));
                }
                if !entry.digest.as_deref().is_some_and(is_prefixed_digest) {
                    return Err(PongError::Integrity(
                        "workspace tree manifest digest is invalid".into(),
                    ));
                }
            }
            _ => {
                return Err(PongError::Unsupported(
                    "workspace tree manifest entry kind is unsupported".into(),
                ))
            }
        }
        previous = Some(&entry.path);
    }
    Ok(())
}

fn is_prefixed_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn validate_identity(value: &str, label: &str) -> Result<(), PongError> {
    if value.trim().is_empty() || value.as_bytes().contains(&0) {
        return Err(PongError::InvalidInput(format!("{label} is invalid")));
    }
    Ok(())
}

fn normalize_relative_path(path: &Path) -> Result<String, PongError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str().ok_or_else(|| {
                    PongError::InvalidInput("workspace path is not valid UTF-8".into())
                })?;
                if value.is_empty() || value == "." || value == ".." || value.contains('\0') {
                    return Err(PongError::InvalidInput("workspace path is invalid".into()));
                }
                if value.contains('\\') {
                    return Err(PongError::InvalidInput(
                        "workspace path contains a non-portable separator".into(),
                    ));
                }
                validate_portable_component(value)?;
                parts.push(value.to_owned());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PongError::InvalidInput(
                    "workspace path escapes its root".into(),
                ))
            }
        }
    }
    if parts.is_empty() {
        return Err(PongError::InvalidInput("workspace path is empty".into()));
    }
    let value = parts.join("/");
    if value.starts_with('/') || value.contains("//") || value.contains(':') {
        return Err(PongError::InvalidInput(
            "workspace path is not portable".into(),
        ));
    }
    Ok(value)
}

fn safe_relative_path(path: &str) -> Result<PathBuf, PongError> {
    let normalized = normalize_relative_path(Path::new(path))?;
    let mut output = PathBuf::new();
    for component in normalized.split('/') {
        output.push(component);
    }
    Ok(output)
}

fn local_locator(repository_root: &Path, path: &Path) -> Result<String, PongError> {
    let repository_root = fs::canonicalize(repository_root).map_err(PongError::from)?;
    ensure_no_reparse_ancestors(path)?;
    let metadata = fs::symlink_metadata(path);
    let candidate = match metadata {
        Ok(metadata) => {
            if is_reparse_point(&metadata) || !metadata.is_dir() {
                return Err(PongError::Integrity(
                    "workspace path is not a safe directory".into(),
                ));
            }
            fs::canonicalize(path).map_err(PongError::from)?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| PongError::InvalidInput("workspace path has no parent".into()))?;
            ensure_no_reparse_ancestors(parent)?;
            let parent_metadata = fs::symlink_metadata(parent).map_err(PongError::from)?;
            if !parent_metadata.is_dir() || is_reparse_point(&parent_metadata) {
                return Err(PongError::Integrity(
                    "workspace path parent is not a safe directory".into(),
                ));
            }
            fs::canonicalize(parent).map_err(PongError::from)?.join(
                path.file_name().ok_or_else(|| {
                    PongError::InvalidInput("workspace path has no file name".into())
                })?,
            )
        }
        Err(error) => return Err(PongError::from(error)),
    };
    let control = repository_root.join(".pong");
    if candidate == repository_root || candidate == control || candidate.starts_with(&control) {
        return Err(PongError::InvalidInput(
            "workspace path must be outside the repository control directory".into(),
        ));
    }
    candidate
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| PongError::InvalidInput("workspace path is not valid UTF-8".into()))
}

/// Check every existing ancestor without following symlinks/reparse points.
/// Missing leaf components are allowed because callers may create them after
/// this check, but an existing redirect anywhere in the path is rejected.
fn ensure_no_reparse_ancestors(path: &Path) -> Result<(), PongError> {
    let ancestors = path.ancestors().collect::<Vec<_>>();
    for ancestor in ancestors.into_iter().rev() {
        let metadata = match fs::symlink_metadata(ancestor) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(PongError::from_protected_io(error)),
        };
        if is_reparse_point(&metadata) {
            return Err(PongError::Integrity(
                "workspace path contains a symlink or reparse point".into(),
            ));
        }
        if !metadata.is_dir() {
            return Err(PongError::InvalidInput(
                "workspace path ancestor is not a directory".into(),
            ));
        }
    }
    Ok(())
}

fn validate_portable_component(value: &str) -> Result<(), PongError> {
    if value.len() > 255
        || value.ends_with(' ')
        || value.ends_with('.')
        || value
            .chars()
            .any(|character| character.is_control() || "<>:\"|?*".contains(character))
    {
        return Err(PongError::InvalidInput(
            "workspace path component is not portable".into(),
        ));
    }
    let base = value
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    if matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(PongError::InvalidInput(
            "workspace path uses a reserved portable name".into(),
        ));
    }
    Ok(())
}

fn ensure_no_reparse_components(root: &Path, relative: &Path) -> Result<(), PongError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if let Component::Normal(value) = component {
            current.push(value);
            if let Ok(metadata) = fs::symlink_metadata(&current) {
                if is_reparse_point(&metadata) {
                    return Err(PongError::Integrity(
                        "materialization path contains a symlink or reparse point".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}-{sequence:x}")
}

fn workspace_fault_error(point: WorkspaceFailPoint, action: WorkspaceFaultAction) -> PongError {
    let label = point.label();
    match action {
        WorkspaceFaultAction::Continue
        | WorkspaceFaultAction::Fail
        | WorkspaceFaultAction::ShortWrite(_) => PongError::FaultInjected(label.into()),
        WorkspaceFaultAction::PermissionDenied => {
            PongError::PermissionDenied(format!("{label}:permission_denied"))
        }
        WorkspaceFaultAction::ResourceExhausted => {
            PongError::ResourceExhausted(format!("{label}:resource_exhausted"))
        }
    }
}

fn cleanup_rollback_backups(
    parent: &Path,
    workspace_name: Option<&std::ffi::OsStr>,
) -> Result<(), PongError> {
    let Some(workspace_name) = workspace_name.and_then(|name| name.to_str()) else {
        return Ok(());
    };
    let prefix = format!(".{workspace_name}.rollback-old-");
    for entry in fs::read_dir(parent).map_err(PongError::from_protected_io)? {
        let entry = entry.map_err(PongError::from_protected_io)?;
        let name = entry.file_name();
        if !name.to_str().is_some_and(|name| name.starts_with(&prefix)) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(PongError::from_protected_io)?;
        if is_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(PongError::Integrity(
                "rollback backup is not a regular directory".into(),
            ));
        }
        fs::remove_dir_all(entry.path()).map_err(PongError::from_protected_io)?;
    }
    sync_directory(parent)?;
    Ok(())
}

fn recover_rollback_layout(
    parent: &Path,
    workspace_name: Option<&std::ffi::OsStr>,
    workspace_root: &Path,
) -> Result<(), PongError> {
    let Some(workspace_name) = workspace_name.and_then(|name| name.to_str()) else {
        return Ok(());
    };
    if fs::symlink_metadata(workspace_root).is_ok() {
        return Ok(());
    }
    let prefix = format!(".{workspace_name}.rollback-old-");
    let mut backups = fs::read_dir(parent)
        .map_err(PongError::from_protected_io)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(&prefix))
        })
        .collect::<Vec<_>>();
    backups.sort_by_key(|entry| entry.file_name());
    let Some(backup) = backups.pop() else {
        return Err(PongError::RecoveryRequired(
            "rollback workspace tree is missing and no old complete backup exists".into(),
        ));
    };
    let metadata = fs::symlink_metadata(backup.path()).map_err(PongError::from_protected_io)?;
    if is_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(PongError::Integrity(
            "rollback backup is not a regular directory".into(),
        ));
    }
    fs::rename(backup.path(), workspace_root).map_err(PongError::from_protected_io)?;
    sync_directory(parent)?;
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
