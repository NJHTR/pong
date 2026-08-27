//! Provider-neutral workspace handles and the bounded local filesystem driver.
//!
//! A workspace ID is logical metadata. `LocalWorkspace` owns only a checked
//! physical materialization and never treats its path as identity. Snapshots
//! are immutable CAS blobs plus a canonical tree manifest; materialization
//! always builds a new directory so a partial write cannot masquerade as a
//! complete snapshot.

use crate::atomic_replace::{rename_new_with_retry, sync_directory};
use crate::canonical::canonical_bytes;
use crate::cas::{Cas, Digest};
use crate::error::PongError;
use crate::metadata::{LeaseToken, WorkspaceRecord, WorkspaceUpdate};
use crate::redaction::Redactor;
use crate::repository::Repository;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    pub digest: Digest,
    pub workspace_id: String,
    pub project_id: String,
    pub file_count: usize,
    pub total_bytes: u64,
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
            digest,
            workspace_id: self.workspace_id.clone(),
            project_id: self.project_id.clone(),
            file_count,
            total_bytes,
        })
    }

    pub fn read_manifest(&self, cas: &Cas, digest: Digest) -> Result<TreeManifest, PongError> {
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
        validate_manifest(
            &manifest,
            &self.workspace_id,
            &self.project_id,
            &self.redactor,
        )?;
        Ok(manifest)
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
        self.repository
            .metadata_mut()
            .update_workspace(WorkspaceUpdate {
                workspace_id,
                expected_revision: record.revision,
                lease,
                branch_ref: record.branch_ref.as_deref(),
                head: Some(&head),
                environment_id: record.environment_id.as_deref(),
                status: "ready",
                updated_at: now,
                now_ms,
            })?;
        Ok(snapshot)
    }

    pub fn workspace(&self, workspace_id: &str) -> Result<Option<WorkspaceRecord>, PongError> {
        self.repository.metadata().workspace(workspace_id)
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
