//! Repository ownership and startup compatibility checks.
//!
//! A [`Repository`] is the durable boundary for the local core.  It owns the
//! `.pong` layout and opens the two storage primitives together: the
//! filesystem CAS and the SQLite metadata store.  Higher-level adapters should
//! receive this handle rather than opening either store directly.

use crate::atomic_replace::{atomic_replace_file, rename_with_retry, sync_directory};
use crate::canonical::canonical_bytes;
use crate::cas::digest_hex;
use crate::cas::{Cas, CasFailpoints};
use crate::error::PongError;
use crate::metadata::{JournalOperation, MetadataFailpoints, MetadataStore};
use crate::redaction::Redactor;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Repository format written by this crate.
///
/// The format is deliberately independent of the Rust crate version.  A
/// reader checks it before opening or mutating the metadata store.
pub const REPOSITORY_FORMAT: &str = "0.1";

/// Version of the on-disk repository marker schema.
pub const REPOSITORY_MARKER_VERSION: u32 = 1;

/// Version for the first metadata/object schema family.
pub const REPOSITORY_SCHEMA_VERSION: &str = "0.1";

/// Repository format written to generation-backed metadata stores.
pub const GENERATION_REPOSITORY_FORMAT: &str = "0.2";

/// Schema version for repository generation metadata.
pub const GENERATION_SCHEMA_VERSION: &str = "0.2";

/// Version of the repository selector schema stored at `.pong/repository.json`.
pub const REPOSITORY_SELECTOR_VERSION: u32 = 2;

const STORAGE_DRIVER: &str = "cas+sqlite";
const MARKER_FILENAME: &str = "repository.json";
const METADATA_FILENAME: &str = "metadata.sqlite";
const OBJECTS_DIRNAME: &str = "objects";
const STAGING_DIRNAME: &str = "staging";
const QUARANTINE_DIRNAME: &str = "quarantine";
const GENERATIONS_DIRNAME: &str = "generations";
const MIGRATIONS_DIRNAME: &str = "migrations";
const LOCKS_DIRNAME: &str = "locks";
const GENERATION_MANIFEST_FILENAME: &str = "generation.json";
const REPOSITORY_LOCK_FILENAME: &str = "repository.lock";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepositorySelector {
    /// Kept under the legacy marker field name so a v0.1 reader parses the
    /// incompatible version and returns `UNSUPPORTED` before opening SQLite.
    pub marker_version: u32,
    pub repository_format: String,
    pub schema_version: String,
    pub storage_driver: String,
    pub active_generation: String,
    pub migration_id: String,
    pub generation_manifest_digest: String,
}

impl RepositorySelector {
    fn as_marker_view(&self) -> RepositoryMarker {
        RepositoryMarker {
            marker_version: self.marker_version,
            repository_format: self.repository_format.clone(),
            schema_version: self.schema_version.clone(),
            storage_driver: self.storage_driver.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepositoryGenerationManifest {
    pub manifest_version: u32,
    pub generation_id: String,
    pub migration_id: String,
    pub repository_format: String,
    pub schema_version: String,
    pub storage_driver: String,
    pub source_marker_digest: String,
    pub metadata_digest: String,
    pub metadata_size: u64,
    pub redaction_profile_id: String,
    pub redaction_profile_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationSpec {
    pub migration_id: String,
    pub target_generation_id: String,
}

impl MigrationSpec {
    pub fn new(migration_id: impl Into<String>, target_generation_id: impl Into<String>) -> Self {
        Self {
            migration_id: migration_id.into(),
            target_generation_id: target_generation_id.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationOutcome {
    pub migration_id: String,
    pub source_generation_id: String,
    pub target_generation_id: String,
    pub plan_digest: String,
    pub selector_digest: String,
    pub generation_manifest_digest: String,
    pub already_active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationFailpoint {
    Preflight,
    TargetAllocated,
    AfterBackup,
    AfterCheckpoint,
    AfterVerify,
    BeforeSelectorReplace,
    AfterSelectorReplace,
    AfterSelectorDirectorySync,
    AfterJournalFinalize,
}

impl MigrationFailpoint {
    fn label(self) -> &'static str {
        match self {
            Self::Preflight => "preflight",
            Self::TargetAllocated => "target_allocated",
            Self::AfterBackup => "after_backup",
            Self::AfterCheckpoint => "after_checkpoint",
            Self::AfterVerify => "after_verify",
            Self::BeforeSelectorReplace => "before_selector_replace",
            Self::AfterSelectorReplace => "after_selector_replace",
            Self::AfterSelectorDirectorySync => "after_selector_directory_sync",
            Self::AfterJournalFinalize => "after_journal_finalize",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct MigrationFailpoints {
    armed: Option<MigrationFailpoint>,
}

impl MigrationFailpoints {
    pub const fn disabled() -> Self {
        Self { armed: None }
    }

    pub const fn once(point: MigrationFailpoint) -> Self {
        Self { armed: Some(point) }
    }

    fn take_if(&mut self, point: MigrationFailpoint) -> Option<MigrationFailpoint> {
        if self.armed == Some(point) {
            let armed = self.armed;
            self.armed = None;
            armed
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MigrationJournal {
    journal_version: u32,
    migration_id: String,
    source_generation_id: String,
    target_generation_id: String,
    plan_digest: String,
    source_root_digest: String,
    source_metadata_digest: String,
    target_metadata_digest: Option<String>,
    selector_digest: Option<String>,
    status: String,
    finalized_at: Option<String>,
}

enum RepositoryRootDocument {
    Legacy(RepositoryMarker),
    Selector(RepositorySelector),
}

/// Immutable compatibility marker stored at `.pong/repository.json`.
///
/// Unknown JSON fields are intentionally ignored when reading, so additive
/// marker metadata can be introduced without making an older reader reject a
/// repository before it has checked the fields it understands.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepositoryMarker {
    pub marker_version: u32,
    pub repository_format: String,
    pub schema_version: String,
    pub storage_driver: String,
}

impl Default for RepositoryMarker {
    fn default() -> Self {
        Self {
            marker_version: REPOSITORY_MARKER_VERSION,
            repository_format: REPOSITORY_FORMAT.into(),
            schema_version: REPOSITORY_SCHEMA_VERSION.into(),
            storage_driver: STORAGE_DRIVER.into(),
        }
    }
}

/// The stable paths owned by a local repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryLayout {
    root: PathBuf,
    pong: PathBuf,
    marker: PathBuf,
    metadata: PathBuf,
    cas: PathBuf,
    objects: PathBuf,
    staging: PathBuf,
    quarantine: PathBuf,
    generations: PathBuf,
    migrations: PathBuf,
    locks: PathBuf,
}

impl RepositoryLayout {
    /// Build paths from a project root without touching the filesystem.
    pub fn from_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let pong = root.join(".pong");
        let marker = pong.join(MARKER_FILENAME);
        let metadata = pong.join(METADATA_FILENAME);
        let cas = pong.clone();
        let objects = cas.join(OBJECTS_DIRNAME);
        let staging = cas.join(STAGING_DIRNAME);
        let quarantine = cas.join(QUARANTINE_DIRNAME);
        let generations = pong.join(GENERATIONS_DIRNAME);
        let migrations = pong.join(MIGRATIONS_DIRNAME);
        let locks = pong.join(LOCKS_DIRNAME);
        Self {
            root,
            pong,
            marker,
            metadata,
            cas,
            objects,
            staging,
            quarantine,
            generations,
            migrations,
            locks,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn pong_dir(&self) -> &Path {
        &self.pong
    }

    pub fn marker_path(&self) -> &Path {
        &self.marker
    }

    pub fn metadata_path(&self) -> &Path {
        &self.metadata
    }

    /// Root passed to [`Cas::new`].  The CAS keeps its `objects`, `staging`,
    /// and `quarantine` directories directly beneath this path.
    pub fn cas_root(&self) -> &Path {
        &self.cas
    }

    pub fn objects_dir(&self) -> &Path {
        &self.objects
    }

    pub fn staging_dir(&self) -> &Path {
        &self.staging
    }

    pub fn quarantine_dir(&self) -> &Path {
        &self.quarantine
    }

    pub fn generations_dir(&self) -> &Path {
        &self.generations
    }

    pub fn migrations_dir(&self) -> &Path {
        &self.migrations
    }

    pub fn locks_dir(&self) -> &Path {
        &self.locks
    }

    pub fn repository_lock_path(&self) -> PathBuf {
        self.locks.join(REPOSITORY_LOCK_FILENAME)
    }

    pub fn generation_dir(&self, generation_id: &str) -> PathBuf {
        self.generations.join(generation_id)
    }

    pub fn generation_metadata_path(&self, generation_id: &str) -> PathBuf {
        self.generation_dir(generation_id).join(METADATA_FILENAME)
    }

    pub fn generation_manifest_path(&self, generation_id: &str) -> PathBuf {
        self.generation_dir(generation_id)
            .join(GENERATION_MANIFEST_FILENAME)
    }

    pub fn migration_journal_path(&self, migration_id: &str) -> PathBuf {
        self.migrations.join(format!("{migration_id}.json"))
    }
}

/// Process-wide advisory fence for the repository namespace.
///
/// Ordinary repository handles retain a shared lock for their lifetime.
/// Migration obtains an exclusive lock before it reads the legacy source, so
/// current Pong processes cannot mutate the source while a new generation is
/// being built. Older binaries do not know this lock; migration therefore
/// remains explicitly offline and callers must close those processes first.
struct RepositoryLock {
    file: File,
}

impl RepositoryLock {
    fn shared(layout: &RepositoryLayout) -> Result<Self, PongError> {
        ensure_directory(&layout.locks)?;
        let file = open_lock_file(&layout.repository_lock_path())?;
        FileExt::try_lock_shared(&file).map_err(lock_error)?;
        Ok(Self { file })
    }

    fn exclusive(layout: &RepositoryLayout) -> Result<Self, PongError> {
        ensure_directory(&layout.locks)?;
        let file = open_lock_file(&layout.repository_lock_path())?;
        FileExt::try_lock_exclusive(&file).map_err(lock_error)?;
        Ok(Self { file })
    }
}

impl Drop for RepositoryLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

/// A local Pong repository with its CAS and metadata store already opened.
pub struct Repository {
    layout: RepositoryLayout,
    marker: RepositoryMarker,
    selector: Option<RepositorySelector>,
    generation_manifest: Option<RepositoryGenerationManifest>,
    _repository_lock: RepositoryLock,
    cas: Cas,
    metadata: MetadataStore,
}

impl fmt::Debug for Repository {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Repository")
            .field("layout", &self.layout)
            .field("marker", &self.marker)
            .finish_non_exhaustive()
    }
}

impl Repository {
    /// Initialize a repository with the default redaction profile.
    ///
    /// Initialization is idempotent when the existing marker is compatible;
    /// this lets a process safely retry after a crash during first startup.
    pub fn init(root: impl AsRef<Path>) -> Result<Self, PongError> {
        Self::init_with_redactor(root, Redactor::default())
    }

    /// Initialize a repository using an explicit redaction profile.
    pub fn init_with_redactor(
        root: impl AsRef<Path>,
        redactor: Redactor,
    ) -> Result<Self, PongError> {
        let root = create_project_root(root.as_ref())?;
        let layout = RepositoryLayout::from_root(root);
        ensure_directory(&layout.pong)?;

        // A marker is the commit point for initialization.  If it is already
        // visible, use the normal startup path so all compatibility checks are
        // applied and no second marker can race with this call.
        if layout.marker.exists() {
            return Self::open_layout(
                layout,
                redactor,
                MetadataFailpoints::disabled(),
                CasFailpoints::disabled(),
            );
        }

        // Prepare directories before taking the byte-range fence. Windows
        // may reject a directory flush while a sibling lock file is held.
        create_cas_directories(&layout)?;
        ensure_directory(&layout.generations)?;
        ensure_directory(&layout.migrations)?;

        // Initialization modifies the root namespace, so serialize the
        // marker/database commit with migration. Once the marker is
        // published, reopen normally under a shared lease.
        let initialization_lock = RepositoryLock::exclusive(&layout)?;
        if layout.marker.exists() {
            drop(initialization_lock);
            return Self::open_layout(
                layout,
                redactor,
                MetadataFailpoints::disabled(),
                CasFailpoints::disabled(),
            );
        }

        let metadata = MetadataStore::open_with_redactor(&layout.metadata, redactor.clone())?;
        let cas = Cas::new_with_redactor(&layout.cas, redactor.clone())?;
        let marker = RepositoryMarker::default();
        publish_marker(&layout.marker, &marker)?;
        drop(cas);
        drop(metadata);
        scan_owned_bytes(&layout.pong, &redactor)?;
        drop(initialization_lock);
        Self::open_layout(
            layout,
            redactor,
            MetadataFailpoints::disabled(),
            CasFailpoints::disabled(),
        )
    }

    /// Open an existing repository with the default redaction profile.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, PongError> {
        Self::open_with_redactor(root, Redactor::default())
    }

    /// Open an existing repository with an explicit redaction profile.
    pub fn open_with_redactor(
        root: impl AsRef<Path>,
        redactor: Redactor,
    ) -> Result<Self, PongError> {
        let root = existing_project_root(root.as_ref())?;
        Self::open_layout(
            RepositoryLayout::from_root(root),
            redactor,
            MetadataFailpoints::disabled(),
            CasFailpoints::disabled(),
        )
    }

    /// Open an existing repository with explicit one-shot fault schedules.
    ///
    /// This entrance is for deterministic cold-restart tests. The ordinary
    /// [`Repository::open`] and [`Repository::open_with_redactor`] paths always
    /// pass disabled schedules and therefore retain production behavior.
    pub fn open_with_failpoints(
        root: impl AsRef<Path>,
        metadata_failpoints: MetadataFailpoints,
        cas_failpoints: CasFailpoints,
    ) -> Result<Self, PongError> {
        Self::open_with_redactor_and_failpoints(
            root,
            Redactor::default(),
            metadata_failpoints,
            cas_failpoints,
        )
    }

    /// Open an existing repository with an explicit redaction policy and
    /// deterministic one-shot fault schedules for both storage primitives.
    pub fn open_with_redactor_and_failpoints(
        root: impl AsRef<Path>,
        redactor: Redactor,
        metadata_failpoints: MetadataFailpoints,
        cas_failpoints: CasFailpoints,
    ) -> Result<Self, PongError> {
        let root = existing_project_root(root.as_ref())?;
        Self::open_layout(
            RepositoryLayout::from_root(root),
            redactor,
            metadata_failpoints,
            cas_failpoints,
        )
    }

    fn open_layout(
        layout: RepositoryLayout,
        redactor: Redactor,
        metadata_failpoints: MetadataFailpoints,
        cas_failpoints: CasFailpoints,
    ) -> Result<Self, PongError> {
        let repository_lock = RepositoryLock::shared(&layout)?;
        let root_document = read_repository_root(&layout.marker)?;
        let (marker, selector, generation_manifest, metadata_path) = match root_document {
            RepositoryRootDocument::Legacy(marker) => (marker, None, None, layout.metadata.clone()),
            RepositoryRootDocument::Selector(selector) => {
                let generation_root = layout.generation_dir(&selector.active_generation);
                let generation_manifest = read_and_validate_generation_manifest(
                    &layout,
                    &selector.active_generation,
                    &selector.generation_manifest_digest,
                )?;
                let marker = selector.as_marker_view();
                let metadata_path = generation_root.join(METADATA_FILENAME);
                (
                    marker,
                    Some(selector),
                    Some(generation_manifest),
                    metadata_path,
                )
            }
        };
        if let (Some(selector), Some(manifest)) = (&selector, &generation_manifest) {
            finalize_selector_journal(&layout, selector, manifest)?;
        }
        // The marker check is deliberately first: an empty project is a
        // normal "not initialized" result, not a storage I/O failure. Once a
        // compatible marker proves that `.pong` is a repository, scan every
        // owned byte before opening mutable stores.
        scan_owned_bytes(&layout.pong, &redactor)?;
        verify_cas_directories(&layout)?;
        if !is_regular_file(&metadata_path)? {
            return Err(PongError::Integrity(format!(
                "repository metadata file is missing or not a regular file: {}",
                metadata_path.display()
            )));
        }
        let mut metadata = match MetadataStore::open_with_redactor_and_failpoints(
            &metadata_path,
            redactor.clone(),
            metadata_failpoints,
        ) {
            Ok(metadata) => metadata,
            Err(error) if generation_manifest.is_some() => {
                return Err(classify_generation_error(error))
            }
            Err(error) => return Err(error),
        };
        let metadata_format = metadata.repository_format().map_err(|error| {
            if generation_manifest.is_some() {
                classify_generation_error(error)
            } else {
                error
            }
        })?;
        if metadata_format != marker.repository_format {
            return Err(PongError::Integrity(format!(
                "repository marker format {} disagrees with metadata format {}",
                marker.repository_format, metadata_format
            )));
        }
        if let Some(manifest) = &generation_manifest {
            metadata
                .verify_integrity(&marker.repository_format)
                .map_err(classify_generation_error)?;
            let identity = metadata
                .generation_identity()
                .map_err(classify_generation_error)?
                .ok_or_else(|| {
                    PongError::Integrity(
                        "active generation metadata has no durable generation identity".into(),
                    )
                })?;
            if identity.0 != manifest.generation_id || identity.1 != manifest.migration_id {
                return Err(PongError::Integrity(
                    "active generation metadata identity does not match its manifest".into(),
                ));
            }
            let profile = metadata.redaction_profile();
            if profile.id != manifest.redaction_profile_id
                || profile.version != manifest.redaction_profile_version
            {
                return Err(PongError::Integrity(
                    "active generation redaction profile does not match its manifest".into(),
                ));
            }
            metadata
                .validate_projection_bindings(&manifest.generation_id, &manifest.migration_id)
                .map_err(classify_generation_error)?;
        }
        let cas = Cas::new_with_redactor_and_failpoints(&layout.cas, redactor, cas_failpoints)?;
        // SQLite may create WAL/SHM sidecars while opening. Scan again after
        // initialization so those newly-created durable bytes are covered.
        scan_owned_bytes(&layout.pong, metadata.redactor())?;
        // Startup recovery is part of the repository health boundary. It is
        // idempotent and leaves outcome-durable rows available for republish.
        metadata.recover_unfinished_operations().map_err(|error| {
            if generation_manifest.is_some() {
                classify_generation_error(error)
            } else {
                error
            }
        })?;
        Ok(Self {
            layout,
            marker,
            selector,
            generation_manifest,
            _repository_lock: repository_lock,
            cas,
            metadata,
        })
    }

    pub fn layout(&self) -> &RepositoryLayout {
        &self.layout
    }

    pub fn root(&self) -> &Path {
        self.layout.root()
    }

    pub fn pong_dir(&self) -> &Path {
        self.layout.pong_dir()
    }

    pub fn marker(&self) -> &RepositoryMarker {
        &self.marker
    }

    pub fn selector(&self) -> Option<&RepositorySelector> {
        self.selector.as_ref()
    }

    pub fn generation_manifest(&self) -> Option<&RepositoryGenerationManifest> {
        self.generation_manifest.as_ref()
    }

    pub fn active_generation_id(&self) -> Option<&str> {
        self.selector
            .as_ref()
            .map(|selector| selector.active_generation.as_str())
    }

    /// Path of the metadata file actually opened by this handle.  For a
    /// legacy v0.1 repository this is `.pong/metadata.sqlite`; after a
    /// generation migration it is the selector's active generation file.
    pub fn active_metadata_path(&self) -> PathBuf {
        self.selector.as_ref().map_or_else(
            || self.layout.metadata.clone(),
            |selector| {
                self.layout
                    .generation_metadata_path(&selector.active_generation)
            },
        )
    }

    pub fn cas(&self) -> &Cas {
        &self.cas
    }

    pub fn metadata(&self) -> &MetadataStore {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut MetadataStore {
        &mut self.metadata
    }

    /// Re-run the bounded startup recovery action explicitly. Repeated calls
    /// converge and return only intents newly classified as unknown.
    pub fn recover_unfinished_operations(&mut self) -> Result<Vec<JournalOperation>, PongError> {
        self.metadata.recover_unfinished_operations()
    }

    /// Migrate a legacy v0.1 repository to a new, generation-backed metadata
    /// file. The operation is deliberately a static constructor: callers must
    /// close existing repository handles before requesting the offline
    /// exclusive fence.
    pub fn migrate(
        root: impl AsRef<Path>,
        spec: MigrationSpec,
    ) -> Result<MigrationOutcome, PongError> {
        Self::migrate_with_redactor_and_failpoints(
            root,
            spec,
            Redactor::default(),
            MigrationFailpoints::disabled(),
        )
    }

    pub fn migrate_with_redactor(
        root: impl AsRef<Path>,
        spec: MigrationSpec,
        redactor: Redactor,
    ) -> Result<MigrationOutcome, PongError> {
        Self::migrate_with_redactor_and_failpoints(
            root,
            spec,
            redactor,
            MigrationFailpoints::disabled(),
        )
    }

    /// Test-facing migration entry with one-shot crash boundaries. Production
    /// callers should use [`Repository::migrate`] so no injection is possible
    /// through the normal API.
    pub fn migrate_with_failpoints(
        root: impl AsRef<Path>,
        spec: MigrationSpec,
        failpoints: MigrationFailpoints,
    ) -> Result<MigrationOutcome, PongError> {
        Self::migrate_with_redactor_and_failpoints(root, spec, Redactor::default(), failpoints)
    }

    pub fn migrate_with_redactor_and_failpoints(
        root: impl AsRef<Path>,
        spec: MigrationSpec,
        redactor: Redactor,
        mut failpoints: MigrationFailpoints,
    ) -> Result<MigrationOutcome, PongError> {
        validate_migration_spec(&spec)?;
        let root = existing_project_root(root.as_ref())?;
        let layout = RepositoryLayout::from_root(root);
        if !is_regular_file(&layout.marker)? {
            return Err(PongError::NotFound(
                "repository marker does not exist".into(),
            ));
        }
        let _lock = RepositoryLock::exclusive(&layout)?;
        ensure_directory(&layout.generations)?;
        ensure_directory(&layout.migrations)?;
        scan_owned_bytes(&layout.pong, &redactor)?;

        match migrate_locked(&layout, &spec, &redactor, &mut failpoints) {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                // A failed pre-selector attempt must never leave a directory
                // that a later cold start could mistake for active state.
                if matches!(
                    read_repository_root(&layout.marker),
                    Ok(RepositoryRootDocument::Legacy(_))
                ) && cleanup_unpublished_generation(&layout, &spec).is_err()
                {
                    return Err(PongError::RecoveryRequired(
                        "unpublished migration target could not be safely cleaned".into(),
                    ));
                }
                Err(error)
            }
        }
    }
}

fn open_lock_file(path: &Path) -> Result<File, PongError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(PongError::from)
}

fn lock_error(error: std::io::Error) -> PongError {
    if error.kind() == std::io::ErrorKind::WouldBlock {
        PongError::Conflict(
            "repository lock is busy; migration requires an offline repository".into(),
        )
    } else {
        PongError::from(error)
    }
}

fn validate_migration_spec(spec: &MigrationSpec) -> Result<(), PongError> {
    validate_path_component(&spec.migration_id, "migration id")?;
    validate_path_component(&spec.target_generation_id, "generation id")?;
    if spec.migration_id == spec.target_generation_id {
        return Err(PongError::InvalidInput(
            "migration id and generation id must be distinct".into(),
        ));
    }
    Ok(())
}

fn validate_path_component(value: &str, label: &str) -> Result<(), PongError> {
    if value.is_empty() || value == "." || value == ".." || value.contains('\0') {
        return Err(PongError::InvalidInput(format!("{label} is invalid")));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(PongError::InvalidInput(format!(
            "{label} contains unsafe path characters"
        )));
    }
    Ok(())
}

fn migrate_locked(
    layout: &RepositoryLayout,
    spec: &MigrationSpec,
    redactor: &Redactor,
    failpoints: &mut MigrationFailpoints,
) -> Result<MigrationOutcome, PongError> {
    let root_document = read_repository_root(&layout.marker)?;
    if let RepositoryRootDocument::Selector(selector) = root_document {
        validate_selector(&selector)?;
        if selector.active_generation != spec.target_generation_id
            || selector.migration_id != spec.migration_id
        {
            return Err(PongError::Conflict(
                "repository already has a different active generation".into(),
            ));
        }
        let manifest = read_and_validate_generation_manifest(
            layout,
            &selector.active_generation,
            &selector.generation_manifest_digest,
        )?;
        let journal = finalize_selector_journal(layout, &selector, &manifest)?;
        let selector_digest = byte_digest(&fs::read(&layout.marker)?)?;
        if journal.plan_digest
            != plan_digest(
                spec,
                &journal.source_root_digest,
                &journal.source_metadata_digest,
            )?
        {
            return Err(PongError::Conflict(
                "migration journal plan digest does not match the requested plan".into(),
            ));
        }
        return Ok(MigrationOutcome {
            migration_id: spec.migration_id.clone(),
            source_generation_id: journal.source_generation_id,
            target_generation_id: selector.active_generation,
            plan_digest: journal.plan_digest,
            selector_digest,
            generation_manifest_digest: selector.generation_manifest_digest,
            already_active: true,
        });
    }

    fault_if(failpoints, MigrationFailpoint::Preflight)?;
    let source_marker_bytes = fs::read(&layout.marker)?;
    let source_root_digest = byte_digest(&source_marker_bytes)?;
    let source_path = layout.metadata_path().to_path_buf();
    if !is_regular_file(&source_path)? {
        return Err(PongError::Integrity(
            "legacy metadata file is missing or not regular".into(),
        ));
    }
    // A legacy source must be captured before any additive schema initializer
    // can touch it.  The read-only backup entrance validates the existing
    // v0.1 identities but does not create M2 tables or metadata rows, so the
    // source digest remains an attestation of the actual legacy generation.
    let source = MetadataStore::open_for_backup(&source_path, redactor.clone())?;
    if source.repository_format()? != REPOSITORY_FORMAT {
        return Err(PongError::Unsupported(
            "migration source is not a legacy v0.1 repository".into(),
        ));
    }
    // The source handle is intentionally read-only. Do not checkpoint or
    // otherwise mutate the legacy database here: SQLite Online Backup reads
    // the committed state including WAL pages without requiring a source
    // write, and a checkpoint would fail under the read-only migration path
    // (especially on Windows) while changing legacy-sidecar bytes. Include
    // durable WAL/journal bytes in the source plan digest so a committed
    // change that has not reached the main file cannot evade retry checks;
    // SQLite's non-semantic `-shm` index is deliberately excluded.
    let source_metadata_digest = source_database_digest(&source_path)?;
    let plan_digest = plan_digest(spec, &source_root_digest, &source_metadata_digest)?;
    let journal_path = layout.migration_journal_path(&spec.migration_id);
    let mut journal = match read_migration_journal_optional(&journal_path)? {
        Some(existing) => {
            if existing.plan_digest != plan_digest {
                return Err(PongError::Conflict(
                    "migration id is already bound to a different plan".into(),
                ));
            }
            if existing.source_root_digest != source_root_digest
                || existing.source_metadata_digest != source_metadata_digest
            {
                return Err(PongError::Conflict(
                    "legacy source changed since the migration was prepared".into(),
                ));
            }
            existing
        }
        None => {
            let initial = MigrationJournal {
                journal_version: 1,
                migration_id: spec.migration_id.clone(),
                source_generation_id: "legacy-v0.1".into(),
                target_generation_id: spec.target_generation_id.clone(),
                plan_digest: plan_digest.clone(),
                source_root_digest: source_root_digest.clone(),
                source_metadata_digest: source_metadata_digest.clone(),
                target_metadata_digest: None,
                selector_digest: None,
                status: "preflight".into(),
                finalized_at: None,
            };
            write_migration_journal(layout, &initial)?;
            initial
        }
    };
    if journal.status == "selector_committed" {
        return Err(PongError::RecoveryRequired(
            "migration journal records a selector commit but the selector is legacy".into(),
        ));
    }

    let temporary_generation = layout.generations.join(format!(
        ".{}.{}.tmp",
        spec.target_generation_id, spec.migration_id
    ));
    let final_generation = layout.generation_dir(&spec.target_generation_id);
    remove_generation_tree(&temporary_generation)?;
    let final_exists = match fs::symlink_metadata(&final_generation) {
        Ok(metadata) if metadata.is_dir() && !is_reparse_point(&metadata) => true,
        Ok(_) => {
            return Err(PongError::Integrity(
                "existing target generation is not a safe directory".into(),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(PongError::from(error)),
    };
    if final_exists {
        let manifest_path = layout.generation_manifest_path(&spec.target_generation_id);
        let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|_| {
            PongError::RecoveryRequired(
                "an existing target generation has no readable manifest".into(),
            )
        })?;
        if !manifest_metadata.is_file() || is_reparse_point(&manifest_metadata) {
            return Err(PongError::RecoveryRequired(
                "an existing target generation manifest is not safe".into(),
            ));
        }
        let manifest = fs::read(&manifest_path).map_err(|_| {
            PongError::RecoveryRequired(
                "an existing target generation has no readable manifest".into(),
            )
        })?;
        let manifest: RepositoryGenerationManifest =
            serde_json::from_slice(&manifest).map_err(|_| {
                PongError::RecoveryRequired("existing target manifest is invalid".into())
            })?;
        if manifest.migration_id != spec.migration_id {
            return Err(PongError::Conflict(
                "target generation is already owned by another migration".into(),
            ));
        }
        remove_generation_tree(&final_generation)?;
    }
    fs::create_dir_all(&temporary_generation)?;
    sync_directory(&layout.generations)?;
    fault_if(failpoints, MigrationFailpoint::TargetAllocated)?;

    let target_path = temporary_generation.join(METADATA_FILENAME);
    // The repository lock is the offline writer fence. Keep the source
    // connection outside an SQLite write transaction: SQLite's Online
    // Backup API treats an active writer transaction as a transient BUSY/LOCK
    // source and cannot produce a complete snapshot from it.
    source.backup_to(&target_path)?;
    fault_if(failpoints, MigrationFailpoint::AfterBackup)?;

    let mut target = MetadataStore::open_with_redactor(&target_path, redactor.clone())?;
    target.set_repository_format(GENERATION_REPOSITORY_FORMAT)?;
    target.set_generation_identity(&spec.target_generation_id, &spec.migration_id)?;
    target.rebind_projections(&spec.target_generation_id, &spec.migration_id)?;
    target.checkpoint_truncate()?;
    drop(target);
    sync_file(&target_path)?;
    fault_if(failpoints, MigrationFailpoint::AfterCheckpoint)?;

    let target = MetadataStore::open_with_redactor(&target_path, redactor.clone())?;
    target.verify_integrity(GENERATION_REPOSITORY_FORMAT)?;
    let profile = target.redaction_profile();
    drop(target);
    sync_file(&target_path)?;
    let target_metadata_bytes = fs::read(&target_path)?;
    let target_metadata_digest = byte_digest(&target_metadata_bytes)?;
    let target_metadata_size = target_metadata_bytes.len() as u64;
    let manifest = RepositoryGenerationManifest {
        manifest_version: 1,
        generation_id: spec.target_generation_id.clone(),
        migration_id: spec.migration_id.clone(),
        repository_format: GENERATION_REPOSITORY_FORMAT.into(),
        schema_version: GENERATION_SCHEMA_VERSION.into(),
        storage_driver: STORAGE_DRIVER.into(),
        source_marker_digest: source_root_digest.clone(),
        metadata_digest: target_metadata_digest.clone(),
        metadata_size: target_metadata_size,
        redaction_profile_id: profile.id,
        redaction_profile_version: profile.version,
    };
    write_json_file(
        &temporary_generation.join(GENERATION_MANIFEST_FILENAME),
        &manifest,
    )?;
    scan_owned_bytes(&layout.pong, redactor)?;
    rename_with_retry(&temporary_generation, &final_generation)?;
    sync_directory(&layout.generations)?;

    journal.target_metadata_digest = Some(target_metadata_digest);
    journal.status = "target_verified".into();
    write_migration_journal(layout, &journal)?;
    fault_if(failpoints, MigrationFailpoint::AfterVerify)?;

    let selector = RepositorySelector {
        marker_version: REPOSITORY_SELECTOR_VERSION,
        repository_format: GENERATION_REPOSITORY_FORMAT.into(),
        schema_version: GENERATION_SCHEMA_VERSION.into(),
        storage_driver: STORAGE_DRIVER.into(),
        active_generation: spec.target_generation_id.clone(),
        migration_id: spec.migration_id.clone(),
        generation_manifest_digest: byte_digest(&fs::read(
            layout.generation_manifest_path(&spec.target_generation_id),
        )?)?,
    };
    let selector_bytes = canonical_json_bytes(&selector)?;
    let selector_digest = byte_digest(&selector_bytes)?;
    fault_if(failpoints, MigrationFailpoint::BeforeSelectorReplace)?;
    write_replaced_file(&layout.marker, &selector_bytes)?;
    fault_if(failpoints, MigrationFailpoint::AfterSelectorReplace)?;
    sync_directory(&layout.pong)?;
    fault_if(failpoints, MigrationFailpoint::AfterSelectorDirectorySync)?;

    journal.selector_digest = Some(selector_digest.clone());
    journal.status = "selector_committed".into();
    write_migration_journal(layout, &journal)?;
    journal.status = "finalized".into();
    journal.finalized_at = Some("migration-complete".into());
    write_migration_journal(layout, &journal)?;
    fault_if(failpoints, MigrationFailpoint::AfterJournalFinalize)?;

    Ok(MigrationOutcome {
        migration_id: spec.migration_id.clone(),
        source_generation_id: journal.source_generation_id,
        target_generation_id: spec.target_generation_id.clone(),
        plan_digest,
        selector_digest,
        generation_manifest_digest: selector.generation_manifest_digest,
        already_active: false,
    })
}

fn fault_if(
    failpoints: &mut MigrationFailpoints,
    point: MigrationFailpoint,
) -> Result<(), PongError> {
    if failpoints.take_if(point).is_some() {
        return Err(PongError::FaultInjected(format!(
            "migration_{}",
            point.label()
        )));
    }
    Ok(())
}

fn cleanup_unpublished_generation(
    layout: &RepositoryLayout,
    spec: &MigrationSpec,
) -> Result<(), PongError> {
    let temporary = layout.generations.join(format!(
        ".{}.{}.tmp",
        spec.target_generation_id, spec.migration_id
    ));
    remove_generation_tree(&temporary)?;
    let root = read_repository_root(&layout.marker)?;
    if matches!(root, RepositoryRootDocument::Legacy(_)) {
        let final_generation = layout.generation_dir(&spec.target_generation_id);
        let manifest_path = layout.generation_manifest_path(&spec.target_generation_id);
        let owned_by_plan = match fs::symlink_metadata(&manifest_path) {
            Ok(metadata) if metadata.is_file() && !is_reparse_point(&metadata) => {
                match fs::read(&manifest_path) {
                    Ok(bytes) => serde_json::from_slice::<RepositoryGenerationManifest>(&bytes)
                        .map(|manifest| {
                            manifest.generation_id == spec.target_generation_id
                                && manifest.migration_id == spec.migration_id
                        })
                        .unwrap_or(false),
                    Err(_) => false,
                }
            }
            Ok(_) | Err(_) => false,
        };
        if owned_by_plan {
            remove_generation_tree(&final_generation)?;
        }
    }
    sync_directory(&layout.generations)?;
    Ok(())
}

fn remove_generation_tree(path: &Path) -> Result<(), PongError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !is_reparse_point(&metadata) => {
            reject_reparse_tree(path)?;
            fs::remove_dir_all(path).map_err(PongError::from)
        }
        Ok(_) => Err(PongError::Integrity(
            "generation path is not a safe directory".into(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(PongError::from(error)),
    }
}

fn reject_reparse_tree(path: &Path) -> Result<(), PongError> {
    for entry in fs::read_dir(path)? {
        let entry = entry.map_err(PongError::from)?;
        let child = entry.path();
        let metadata = fs::symlink_metadata(&child)?;
        if is_reparse_point(&metadata) {
            return Err(PongError::Integrity(
                "repository generation contains a symlink or reparse point".into(),
            ));
        }
        if metadata.is_dir() {
            reject_reparse_tree(&child)?;
        }
    }
    Ok(())
}

fn write_migration_journal(
    layout: &RepositoryLayout,
    journal: &MigrationJournal,
) -> Result<(), PongError> {
    write_json_file(
        &layout.migration_journal_path(&journal.migration_id),
        journal,
    )
}

fn read_migration_journal(path: &Path) -> Result<MigrationJournal, PongError> {
    let bytes = fs::read(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PongError::RecoveryRequired("migration journal is missing".into())
        } else {
            PongError::from(error)
        }
    })?;
    let journal: MigrationJournal = serde_json::from_slice(&bytes)
        .map_err(|_| PongError::Integrity("migration journal is invalid".into()))?;
    if journal.journal_version != 1
        || journal.migration_id.is_empty()
        || journal.target_generation_id.is_empty()
    {
        return Err(PongError::Integrity(
            "migration journal fields are invalid".into(),
        ));
    }
    Ok(journal)
}

fn read_migration_journal_optional(path: &Path) -> Result<Option<MigrationJournal>, PongError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !is_reparse_point(&metadata) => {
            read_migration_journal(path).map(Some)
        }
        Ok(_) => Err(PongError::Integrity(
            "migration journal is not a regular file".into(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(PongError::from(error)),
    }
}

/// Complete only the bookkeeping after a selector has become authoritative.
/// The selector is deliberately the commit point: a failure to rewrite this
/// journal must never make startup roll back to the legacy source.
fn finalize_selector_journal(
    layout: &RepositoryLayout,
    selector: &RepositorySelector,
    manifest: &RepositoryGenerationManifest,
) -> Result<MigrationJournal, PongError> {
    if selector.migration_id != manifest.migration_id
        || selector.active_generation != manifest.generation_id
    {
        return Err(PongError::RecoveryRequired(
            "active selector and generation manifest disagree".into(),
        ));
    }
    let selector_digest = byte_digest(&fs::read(&layout.marker)?)?;
    let mut journal =
        read_migration_journal(&layout.migration_journal_path(&selector.migration_id))?;
    if journal.migration_id != selector.migration_id
        || journal.target_generation_id != selector.active_generation
        || journal.target_metadata_digest.as_deref() != Some(manifest.metadata_digest.as_str())
        || journal.source_root_digest != manifest.source_marker_digest
        || journal.plan_digest
            != plan_digest(
                &MigrationSpec::new(&journal.migration_id, &journal.target_generation_id),
                &journal.source_root_digest,
                &journal.source_metadata_digest,
            )?
    {
        return Err(PongError::RecoveryRequired(
            "active selector and migration journal disagree".into(),
        ));
    }
    if journal.status != "finalized" || journal.selector_digest.as_deref() != Some(&selector_digest)
    {
        journal.status = "finalized".into();
        journal.selector_digest = Some(selector_digest);
        journal.finalized_at = Some("selector-recovery".into());
        write_migration_journal(layout, &journal)?;
    }
    Ok(journal)
}

fn plan_digest(
    spec: &MigrationSpec,
    source_root_digest: &str,
    source_metadata_digest: &str,
) -> Result<String, PongError> {
    let value = serde_json::json!({
        "migration_id": spec.migration_id,
        "target_generation_id": spec.target_generation_id,
        "source_format": REPOSITORY_FORMAT,
        "target_format": GENERATION_REPOSITORY_FORMAT,
        "source_root_digest": source_root_digest,
        "source_metadata_digest": source_metadata_digest,
    });
    byte_digest(&canonical_bytes(&value)?)
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, PongError> {
    let value = serde_json::to_value(value).map_err(|error| {
        PongError::Serialization(format!("cannot encode repository JSON: {error}"))
    })?;
    canonical_bytes(&value)
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), PongError> {
    let bytes = canonical_json_bytes(value)?;
    write_replaced_file(path, &bytes)
}

fn write_replaced_file(path: &Path, bytes: &[u8]) -> Result<(), PongError> {
    let parent = path
        .parent()
        .ok_or_else(|| PongError::InvalidInput("repository file has no parent".into()))?;
    ensure_directory(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        unique_suffix()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(PongError::from_protected_io)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(PongError::from_protected_io(error));
    }
    drop(file);
    if let Err(error) = atomic_replace_file(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

fn sync_file(path: &Path) -> Result<(), PongError> {
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    file.sync_all()?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn byte_digest(bytes: &[u8]) -> Result<String, PongError> {
    Ok(format!(
        "sha256:{}",
        digest_hex("pong.repository.bytes", bytes)
    ))
}

/// Digest the durable bytes that make up a legacy SQLite generation without
/// opening it in a writable mode or forcing a WAL checkpoint. The suffix
/// labels are length-delimited so concatenated files cannot become
/// ambiguous; `-shm` is excluded because it is a rebuildable SQLite index.
fn source_database_digest(path: &Path) -> Result<String, PongError> {
    let mut bytes = Vec::new();
    for suffix in ["", "-wal", "-journal"] {
        let mut candidate = path.as_os_str().to_os_string();
        candidate.push(suffix);
        let candidate = PathBuf::from(candidate);
        let contents = match fs::symlink_metadata(&candidate) {
            Ok(metadata) => {
                if is_reparse_point(&metadata) || !metadata.is_file() {
                    return Err(PongError::Integrity(
                        "legacy metadata sidecar is not a regular file".into(),
                    ));
                }
                fs::read(&candidate).map_err(PongError::from_protected_io)?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(PongError::from_protected_io(error)),
        };
        let suffix_bytes = suffix.as_bytes();
        bytes.extend_from_slice(&(suffix_bytes.len() as u64).to_le_bytes());
        bytes.extend_from_slice(suffix_bytes);
        bytes.extend_from_slice(&(contents.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&contents);
    }
    byte_digest(&bytes)
}

fn create_project_root(root: &Path) -> Result<PathBuf, PongError> {
    fs::create_dir_all(root)?;
    fs::canonicalize(root).map_err(PongError::from)
}

fn existing_project_root(root: &Path) -> Result<PathBuf, PongError> {
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PongError::NotFound(format!("project root does not exist: {}", root.display()))
        } else {
            PongError::from(error)
        }
    })?;
    if !metadata.is_dir() {
        return Err(PongError::InvalidInput(format!(
            "project root is not a directory: {}",
            root.display()
        )));
    }
    fs::canonicalize(root).map_err(PongError::from)
}

fn create_cas_directories(layout: &RepositoryLayout) -> Result<(), PongError> {
    ensure_directory(&layout.pong)?;
    for path in [&layout.objects, &layout.staging, &layout.quarantine] {
        if path.exists() {
            ensure_directory(path)?;
        } else {
            fs::create_dir(path)?;
            sync_directory(path.parent().expect("CAS directory has parent"))?;
        }
    }
    Ok(())
}

fn verify_cas_directories(layout: &RepositoryLayout) -> Result<(), PongError> {
    ensure_directory(&layout.pong)?;
    for path in [&layout.objects, &layout.staging, &layout.quarantine] {
        if !path.exists() {
            return Err(PongError::Integrity(format!(
                "repository CAS directory is missing: {}",
                path.display()
            )));
        }
        ensure_directory(path)?;
    }
    Ok(())
}

fn ensure_directory(path: &Path) -> Result<(), PongError> {
    let metadata = fs::symlink_metadata(path);
    match metadata {
        Ok(metadata) if metadata.is_dir() && !is_reparse_point(&metadata) => Ok(()),
        Ok(metadata) if is_reparse_point(&metadata) => Err(PongError::Integrity(
            "repository directory is a symlink or reparse point".into(),
        )),
        Ok(_) => Err(PongError::Integrity(format!(
            "repository path is not a directory: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path)?;
            sync_directory(path.parent().expect("repository directory has parent"))
        }
        Err(error) => Err(PongError::from(error)),
    }
}

fn is_regular_file(path: &Path) -> Result<bool, PongError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.is_file() && !is_reparse_point(&metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(PongError::from(error)),
    }
}

fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn classify_generation_error(error: PongError) -> PongError {
    match error {
        PongError::Sqlite(_) => PongError::Integrity(
            "active generation SQLite database failed to open or verify".into(),
        ),
        other @ (PongError::PermissionDenied(_)
        | PongError::PermissionDeniedWithOsError { .. }
        | PongError::ResourceExhausted(_)) => other,
        other => other,
    }
}

fn read_repository_root(path: &Path) -> Result<RepositoryRootDocument, PongError> {
    if !is_regular_file(path)? {
        return Err(PongError::NotFound(
            "repository selector does not exist".into(),
        ));
    }
    let bytes = fs::read(path)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| PongError::Integrity("repository selector is invalid JSON".into()))?;
    if value.get("active_generation").is_some() || value.get("generation_manifest_digest").is_some()
    {
        let selector: RepositorySelector = serde_json::from_value(value)
            .map_err(|_| PongError::Integrity("repository selector fields are invalid".into()))?;
        validate_selector(&selector)?;
        Ok(RepositoryRootDocument::Selector(selector))
    } else {
        let marker: RepositoryMarker = serde_json::from_value(value)
            .map_err(|_| PongError::Integrity("repository marker fields are invalid".into()))?;
        validate_marker(&marker)?;
        Ok(RepositoryRootDocument::Legacy(marker))
    }
}

fn validate_selector(selector: &RepositorySelector) -> Result<(), PongError> {
    if selector.marker_version != REPOSITORY_SELECTOR_VERSION {
        return Err(PongError::Unsupported(format!(
            "repository selector version {} is unsupported",
            selector.marker_version
        )));
    }
    if selector.repository_format != GENERATION_REPOSITORY_FORMAT
        || selector.schema_version != GENERATION_SCHEMA_VERSION
        || selector.storage_driver != STORAGE_DRIVER
    {
        return Err(PongError::Unsupported(
            "repository selector compatibility identity is unsupported".into(),
        ));
    }
    validate_path_component(&selector.active_generation, "active generation")?;
    validate_path_component(&selector.migration_id, "migration id")?;
    if !is_sha256_digest(&selector.generation_manifest_digest) {
        return Err(PongError::Integrity(
            "repository selector manifest digest is invalid".into(),
        ));
    }
    Ok(())
}

fn read_and_validate_generation_manifest(
    layout: &RepositoryLayout,
    generation_id: &str,
    expected_digest: &str,
) -> Result<RepositoryGenerationManifest, PongError> {
    validate_path_component(generation_id, "generation id")?;
    let generation = layout.generation_dir(generation_id);
    let generation_metadata = fs::symlink_metadata(&generation).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PongError::RecoveryRequired("active generation directory is missing".into())
        } else {
            PongError::from(error)
        }
    })?;
    if !generation_metadata.is_dir() || is_reparse_point(&generation_metadata) {
        return Err(PongError::Integrity(
            "active generation directory is not safe".into(),
        ));
    }
    let manifest_path = layout.generation_manifest_path(generation_id);
    let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PongError::RecoveryRequired("active generation manifest is missing".into())
        } else {
            PongError::from(error)
        }
    })?;
    if !manifest_metadata.is_file() || is_reparse_point(&manifest_metadata) {
        return Err(PongError::Integrity(
            "active generation manifest is not a regular file".into(),
        ));
    }
    let manifest_bytes = fs::read(&manifest_path)?;
    if byte_digest(&manifest_bytes)? != expected_digest {
        return Err(PongError::Integrity(
            "active generation manifest digest mismatch".into(),
        ));
    }
    let manifest: RepositoryGenerationManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| PongError::Integrity("active generation manifest is invalid".into()))?;
    if manifest.manifest_version != 1
        || manifest.generation_id != generation_id
        || manifest.repository_format != GENERATION_REPOSITORY_FORMAT
        || manifest.schema_version != GENERATION_SCHEMA_VERSION
        || manifest.storage_driver != STORAGE_DRIVER
    {
        return Err(PongError::Unsupported(
            "active generation manifest compatibility identity is unsupported".into(),
        ));
    }
    validate_path_component(&manifest.migration_id, "migration id")?;
    let metadata_path = generation.join(METADATA_FILENAME);
    let metadata = fs::symlink_metadata(metadata_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PongError::RecoveryRequired("active generation metadata is missing".into())
        } else {
            PongError::from(error)
        }
    })?;
    if !metadata.is_file() || is_reparse_point(&metadata) {
        return Err(PongError::Integrity(
            "active generation metadata is not a regular file".into(),
        ));
    }
    // `metadata_digest` and `metadata_size` attest to the immutable backup
    // image that was verified before selector publication. The active SQLite
    // database legitimately changes as events and refs are appended, so its
    // live bytes cannot be compared to that snapshot on every open. Startup
    // verifies the current file through SQLite integrity/FK checks instead.
    if manifest.metadata_size == 0 || !is_sha256_digest(&manifest.metadata_digest) {
        return Err(PongError::Integrity(
            "active generation metadata attestation is invalid".into(),
        ));
    }
    if !is_sha256_digest(&manifest.source_marker_digest) {
        return Err(PongError::Integrity(
            "active generation source marker attestation is invalid".into(),
        ));
    }
    Ok(manifest)
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

/// Scan every ordinary file owned by `.pong`, including unknown future
/// subdirectories and SQLite sidecars. Location labels are deliberately
/// stable and path-free so a secret embedded in a user path cannot leak into
/// an error message.
fn scan_owned_bytes(root: &Path, redactor: &Redactor) -> Result<(), PongError> {
    let metadata = fs::symlink_metadata(root).map_err(PongError::from_protected_io)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(PongError::Integrity(
            "repository-owned storage root is not a directory".into(),
        ));
    }
    scan_owned_directory(root, redactor)
}

fn scan_owned_directory(path: &Path, redactor: &Redactor) -> Result<(), PongError> {
    for entry in fs::read_dir(path).map_err(PongError::from_protected_io)? {
        let entry = entry.map_err(PongError::from_protected_io)?;
        let child = entry.path();
        // The active byte-range lock is intentionally unreadable on Windows
        // while held. It contains no repository data; excluding this one
        // control file keeps the complete data scan fail-closed without
        // confusing an OS lock violation with corruption.
        if child.file_name().and_then(|name| name.to_str()) == Some(REPOSITORY_LOCK_FILENAME)
            && path.file_name().and_then(|name| name.to_str()) == Some(LOCKS_DIRNAME)
        {
            continue;
        }
        let metadata = fs::symlink_metadata(&child).map_err(PongError::from_protected_io)?;
        if is_reparse_point(&metadata) {
            return Err(PongError::Integrity(
                "repository-owned storage contains a symlink or reparse point".into(),
            ));
        }
        if metadata.is_dir() {
            scan_owned_directory(&child, redactor)?;
        } else if metadata.is_file() {
            let bytes = fs::read(&child).map_err(PongError::from_protected_io)?;
            redactor.assert_clean_bytes(&bytes, "repository-owned bytes")?;
        } else {
            return Err(PongError::Integrity(
                "repository-owned storage contains a non-regular entry".into(),
            ));
        }
    }
    Ok(())
}

fn read_and_validate_marker(path: &Path) -> Result<RepositoryMarker, PongError> {
    if !is_regular_file(path)? {
        return Err(PongError::NotFound(format!(
            "repository marker does not exist: {}",
            path.display()
        )));
    }
    let bytes = fs::read(path)?;
    let marker: RepositoryMarker = serde_json::from_slice(&bytes).map_err(|error| {
        PongError::Integrity(format!(
            "invalid repository marker {}: {error}",
            path.display()
        ))
    })?;
    validate_marker(&marker)?;
    Ok(marker)
}

fn validate_marker(marker: &RepositoryMarker) -> Result<(), PongError> {
    if marker.marker_version != REPOSITORY_MARKER_VERSION {
        return Err(PongError::Unsupported(format!(
            "repository marker version {} is unsupported; expected {}",
            marker.marker_version, REPOSITORY_MARKER_VERSION
        )));
    }
    if marker.repository_format != REPOSITORY_FORMAT {
        return Err(PongError::Unsupported(format!(
            "repository format {} is unsupported; expected {}",
            marker.repository_format, REPOSITORY_FORMAT
        )));
    }
    if marker.schema_version != REPOSITORY_SCHEMA_VERSION {
        return Err(PongError::Unsupported(format!(
            "repository schema {} is unsupported; expected {}",
            marker.schema_version, REPOSITORY_SCHEMA_VERSION
        )));
    }
    if marker.storage_driver != STORAGE_DRIVER {
        return Err(PongError::Unsupported(format!(
            "repository storage driver {} is unsupported; expected {}",
            marker.storage_driver, STORAGE_DRIVER
        )));
    }
    Ok(())
}

fn publish_marker(path: &Path, marker: &RepositoryMarker) -> Result<(), PongError> {
    let marker_value = serde_json::to_value(marker).map_err(|error| {
        PongError::Serialization(format!("cannot encode repository marker: {error}"))
    })?;
    let bytes = canonical_bytes(&marker_value).map_err(|error| {
        PongError::Serialization(format!("cannot encode repository marker: {error}"))
    })?;
    let parent = path.parent().expect("repository marker has parent");
    ensure_directory(parent)?;
    let staging = parent.join(format!(".{}.{}.tmp", MARKER_FILENAME, unique_suffix()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&staging);
        return Err(PongError::from(error));
    }
    drop(file);
    match fs::hard_link(&staging, path) {
        Ok(()) => {
            fs::remove_file(&staging)?;
            sync_directory(parent)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&staging);
            let existing = read_and_validate_marker(path)?;
            if existing == *marker {
                Ok(())
            } else {
                Err(PongError::Conflict(
                    "repository marker was concurrently initialized with incompatible values"
                        .into(),
                ))
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&staging);
            Err(PongError::from(error))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PongError;
    use tempfile::tempdir;

    #[test]
    fn init_creates_durable_layout_and_marker() {
        let project = tempdir().expect("project");
        let repository = Repository::init(project.path()).expect("init");
        let layout = repository.layout();
        assert!(layout.pong_dir().is_dir());
        assert!(layout.objects_dir().is_dir());
        assert!(layout.staging_dir().is_dir());
        assert!(layout.quarantine_dir().is_dir());
        assert!(layout.metadata_path().is_file());
        assert!(layout.marker_path().is_file());
        assert_eq!(repository.marker(), &RepositoryMarker::default());
    }

    #[test]
    fn init_is_idempotent_and_open_reuses_the_same_format() {
        let project = tempdir().expect("project");
        let first = Repository::init(project.path()).expect("first init");
        let first_marker = first.marker().clone();
        drop(first);
        let second = Repository::init(project.path()).expect("retry init");
        assert_eq!(second.marker(), &first_marker);
        let opened = Repository::open(project.path()).expect("open");
        assert_eq!(opened.root(), second.root());
        assert_eq!(
            opened.metadata().repository_format().unwrap(),
            REPOSITORY_FORMAT
        );
    }

    #[test]
    fn unsupported_marker_fails_before_metadata_open() {
        let project = tempdir().expect("project");
        let repository = Repository::init(project.path()).expect("init");
        let marker_path = repository.layout().marker_path().to_path_buf();
        drop(repository);
        let mut marker: serde_json::Value =
            serde_json::from_slice(&fs::read(&marker_path).unwrap()).unwrap();
        marker["repository_format"] = serde_json::Value::String("9.0".into());
        fs::write(&marker_path, serde_json::to_vec(&marker).unwrap()).unwrap();
        let error = Repository::open(project.path()).expect_err("format mismatch");
        assert!(matches!(error, PongError::Unsupported(_)));
    }

    #[test]
    fn missing_marker_is_not_treated_as_an_initialized_repository() {
        let project = tempdir().expect("project");
        let error = Repository::open(project.path()).expect_err("missing marker");
        assert!(matches!(error, PongError::NotFound(_)));
    }

    #[test]
    fn startup_does_not_recreate_missing_cas_directories() {
        let project = tempdir().expect("project");
        let repository = Repository::init(project.path()).expect("init");
        let objects = repository.layout().objects_dir().to_path_buf();
        drop(repository);
        fs::remove_dir_all(&objects).expect("remove objects");
        let error = Repository::open(project.path()).expect_err("missing CAS directory");
        assert!(matches!(error, PongError::Integrity(_)));
        assert!(!objects.exists());
    }

    #[test]
    fn repository_wires_one_redaction_policy_into_metadata_and_cas() {
        let project = tempdir().expect("project");
        let mut redactor = Redactor::new("test-profile", "0.1").expect("profile");
        redactor.register_secret("repo-secret").expect("secret");
        let repository = Repository::init_with_redactor(project.path(), redactor).expect("init");
        let error = match repository.cas().put("snapshot/v1", b"contains repo-secret") {
            Ok(_) => panic!("repository CAS must reject configured secrets"),
            Err(error) => error,
        };
        assert_eq!(error.code(), "INTEGRITY_ERROR");
        assert_eq!(repository.metadata().redaction_profile().id, "test-profile");
    }

    #[test]
    fn opening_repository_classifies_orphan_intents_as_unknown() {
        let project = tempdir().expect("project");
        let mut repository = Repository::init(project.path()).expect("init");
        repository
            .metadata_mut()
            .record_intent(
                "prj_startup",
                "agt_startup",
                "req_startup",
                "op_startup",
                &serde_json::json!({"tool":"filesystem.write"}),
                "2026-08-19T00:00:00Z",
            )
            .expect("intent");
        drop(repository);

        let repository = Repository::open(project.path()).expect("startup recovery");
        let unknown = repository
            .metadata()
            .unknown_operations()
            .expect("unknown operations");
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].request_id, "req_startup");
    }
}
