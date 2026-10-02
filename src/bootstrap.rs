//! Local, non-secret bootstrap metadata for starting a Pong adapter.
//!
//! Bootstrap metadata is deliberately separate from the repository selector
//! and from the durable SQLite/CAS state. It only tells a launch layer where
//! to open an existing repository and workspace binding root.

use crate::Repository;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const BOOTSTRAP_FILENAME: &str = "bootstrap.json";
pub const BOOTSTRAP_SCHEMA_VERSION: u32 = 1;
pub const BOOTSTRAP_PROTOCOL_VERSION: &str = "1.0";
pub const PONG_ENDPOINT_ENV: &str = "PONG_ENDPOINT";

#[derive(Debug)]
pub enum BootstrapError {
    ProjectRootMissing(PathBuf),
    PongDirectoryMissing(PathBuf),
    MetadataMissing(PathBuf),
    InvalidMetadata(String),
    RepositoryMissing(PathBuf),
    WorkspaceRootMissing(PathBuf),
    ProtocolVersionMismatch { expected: String, found: String },
    EndpointInvalid(String),
    RepositoryInitialization(String),
    Io(io::Error),
    Serialization(serde_json::Error),
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRootMissing(path) => {
                write!(formatter, "project root is unavailable: {}", path.display())
            }
            Self::PongDirectoryMissing(path) => {
                write!(formatter, ".pong directory is missing: {}", path.display())
            }
            Self::MetadataMissing(path) => {
                write!(
                    formatter,
                    "bootstrap metadata is missing: {}",
                    path.display()
                )
            }
            Self::InvalidMetadata(message) => {
                write!(formatter, "bootstrap metadata is invalid: {message}")
            }
            Self::RepositoryMissing(path) => {
                write!(
                    formatter,
                    "bootstrap repository is unavailable: {}",
                    path.display()
                )
            }
            Self::WorkspaceRootMissing(path) => {
                write!(
                    formatter,
                    "bootstrap workspace root is unavailable: {}",
                    path.display()
                )
            }
            Self::ProtocolVersionMismatch { expected, found } => write!(
                formatter,
                "bootstrap protocol version is incompatible (expected {expected}, found {found})"
            ),
            Self::EndpointInvalid(message) => {
                write!(formatter, "bootstrap endpoint is invalid: {message}")
            }
            Self::RepositoryInitialization(message) => {
                write!(formatter, "repository initialization failed: {message}")
            }
            Self::Io(error) => write!(formatter, "bootstrap I/O error: {error}"),
            Self::Serialization(error) => {
                write!(formatter, "bootstrap serialization error: {error}")
            }
        }
    }
}

impl std::error::Error for BootstrapError {}

impl From<io::Error> for BootstrapError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for BootstrapError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

impl BootstrapError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProjectRootMissing(_) => "BOOTSTRAP_PROJECT_ROOT_MISSING",
            Self::PongDirectoryMissing(_) => "BOOTSTRAP_PONG_DIRECTORY_MISSING",
            Self::MetadataMissing(_) => "BOOTSTRAP_METADATA_MISSING",
            Self::InvalidMetadata(_) => "BOOTSTRAP_METADATA_INVALID",
            Self::RepositoryMissing(_) => "BOOTSTRAP_REPOSITORY_MISSING",
            Self::WorkspaceRootMissing(_) => "BOOTSTRAP_WORKSPACE_ROOT_MISSING",
            Self::ProtocolVersionMismatch { .. } => "BOOTSTRAP_PROTOCOL_INCOMPATIBLE",
            Self::EndpointInvalid(_) => "BOOTSTRAP_ENDPOINT_INVALID",
            Self::RepositoryInitialization(_) => "BOOTSTRAP_REPOSITORY_INIT_FAILED",
            Self::Io(_) => "BOOTSTRAP_IO_ERROR",
            Self::Serialization(_) => "BOOTSTRAP_SERIALIZATION_ERROR",
        }
    }
}

/// The small, non-secret descriptor consumed before a Runtime connects to
/// Pong. Paths are resolved relative to the project root when they are not
/// absolute. `core_endpoint` is an optional transport hint; stdio transport
/// intentionally leaves it unset.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapMetadata {
    pub schema_version: u32,
    pub protocol_version: String,
    pub repository_root: String,
    pub workspace_root: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub core_endpoint: Option<String>,
}

impl BootstrapMetadata {
    pub fn new(
        repository_root: impl Into<String>,
        workspace_root: impl Into<String>,
        workspace_id: Option<String>,
        core_endpoint: Option<String>,
    ) -> Self {
        Self {
            schema_version: BOOTSTRAP_SCHEMA_VERSION,
            protocol_version: BOOTSTRAP_PROTOCOL_VERSION.into(),
            repository_root: repository_root.into(),
            workspace_root: workspace_root.into(),
            workspace_id,
            core_endpoint,
        }
    }

    pub fn path(project_root: impl AsRef<Path>) -> PathBuf {
        project_root.as_ref().join(".pong").join(BOOTSTRAP_FILENAME)
    }

    pub fn write(&self, project_root: impl AsRef<Path>) -> Result<PathBuf, BootstrapError> {
        let project_root = project_root.as_ref();
        if !project_root.is_dir() {
            return Err(BootstrapError::ProjectRootMissing(
                project_root.to_path_buf(),
            ));
        }
        let pong = project_root.join(".pong");
        fs::create_dir_all(&pong)?;
        let path = pong.join(BOOTSTRAP_FILENAME);
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(&path, bytes)?;
        Ok(path)
    }
}

/// Initialize the local Repository and its project-discovered bootstrap.
///
/// The stdio Core has no network endpoint to advertise, so initialization
/// writes `core_endpoint: null`. The binding root is owned by Pong under
/// `.pong/workspaces`, keeping initialization away from user business files.
/// An existing bootstrap is validated and preserved byte-for-byte.
pub fn initialize(project_root: impl AsRef<Path>) -> Result<BootstrapResolution, BootstrapError> {
    let project_root = project_root.as_ref();
    let repository = Repository::init(project_root)
        .map_err(|error| BootstrapError::RepositoryInitialization(error.to_string()))?;
    drop(repository);

    let project_root = canonical_directory(project_root)?;
    let workspace_root = project_root.join(".pong").join("workspaces");
    fs::create_dir_all(&workspace_root)?;
    let metadata_path = BootstrapMetadata::path(&project_root);
    if metadata_path.is_file() {
        return discover(&project_root);
    }

    BootstrapMetadata::new(".", ".pong/workspaces", None, None).write(&project_root)?;
    discover(&project_root)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapResolution {
    pub project_root: PathBuf,
    pub repository_root: PathBuf,
    pub workspace_root: PathBuf,
    pub workspace_id: Option<String>,
    pub core_endpoint: Option<String>,
    pub protocol_version: String,
}

pub fn discover(project_root: impl AsRef<Path>) -> Result<BootstrapResolution, BootstrapError> {
    let project_root = canonical_directory(project_root.as_ref())?;
    let pong = project_root.join(".pong");
    if !pong.is_dir() {
        return Err(BootstrapError::PongDirectoryMissing(project_root));
    }
    resolve_at_project_root(&project_root)
}

/// Search the current directory and its ancestors for the nearest `.pong`
/// directory. An existing `.pong` with missing metadata is reported at that
/// boundary instead of silently selecting a parent repository.
pub fn discover_from_cwd() -> Result<BootstrapResolution, BootstrapError> {
    let mut current = env::current_dir().map_err(BootstrapError::Io)?;
    loop {
        if current.join(".pong").is_dir() {
            return discover(&current);
        }
        if !current.pop() {
            return Err(BootstrapError::PongDirectoryMissing(current));
        }
    }
}

/// Resolve an endpoint with deterministic precedence: explicit argument,
/// `PONG_ENDPOINT`, then the local descriptor. No localhost fallback is
/// selected because the stdio adapter has no implicit network listener.
pub fn resolve_endpoint(
    explicit: Option<&str>,
    environment: Option<&str>,
    metadata: Option<&str>,
) -> Result<Option<String>, BootstrapError> {
    let candidate = explicit.or(environment).or(metadata);
    let Some(candidate) = candidate else {
        return Ok(None);
    };
    let candidate = candidate.trim();
    if candidate.is_empty() || candidate.chars().any(char::is_control) {
        return Err(BootstrapError::EndpointInvalid(
            "endpoint must be non-empty and free of control characters".into(),
        ));
    }
    Ok(Some(candidate.to_owned()))
}

fn resolve_at_project_root(project_root: &Path) -> Result<BootstrapResolution, BootstrapError> {
    let metadata_path = BootstrapMetadata::path(project_root);
    if !metadata_path.is_file() {
        return Err(BootstrapError::MetadataMissing(metadata_path));
    }
    let bytes = fs::read(&metadata_path)?;
    let metadata: BootstrapMetadata = serde_json::from_slice(&bytes)
        .map_err(|error| BootstrapError::InvalidMetadata(error.to_string()))?;
    validate_metadata(&metadata)?;

    let repository_root = resolve_path(project_root, &metadata.repository_root);
    if !repository_root.is_dir()
        || !repository_root
            .join(".pong")
            .join("repository.json")
            .is_file()
    {
        return Err(BootstrapError::RepositoryMissing(repository_root));
    }
    let workspace_root = resolve_path(project_root, &metadata.workspace_root);
    if !workspace_root.is_dir() {
        return Err(BootstrapError::WorkspaceRootMissing(workspace_root));
    }
    let environment_endpoint = env::var(PONG_ENDPOINT_ENV).ok();
    let core_endpoint = resolve_endpoint(
        None,
        environment_endpoint.as_deref(),
        metadata.core_endpoint.as_deref(),
    )?;
    Ok(BootstrapResolution {
        project_root: project_root.to_path_buf(),
        repository_root,
        workspace_root,
        workspace_id: metadata.workspace_id,
        core_endpoint,
        protocol_version: metadata.protocol_version,
    })
}

fn validate_metadata(metadata: &BootstrapMetadata) -> Result<(), BootstrapError> {
    if metadata.schema_version != BOOTSTRAP_SCHEMA_VERSION {
        return Err(BootstrapError::InvalidMetadata(format!(
            "unsupported schema version {}",
            metadata.schema_version
        )));
    }
    if metadata.protocol_version != BOOTSTRAP_PROTOCOL_VERSION {
        return Err(BootstrapError::ProtocolVersionMismatch {
            expected: BOOTSTRAP_PROTOCOL_VERSION.into(),
            found: metadata.protocol_version.clone(),
        });
    }
    if metadata.repository_root.trim().is_empty() {
        return Err(BootstrapError::InvalidMetadata(
            "repository_root must not be empty".into(),
        ));
    }
    if metadata.workspace_root.trim().is_empty() {
        return Err(BootstrapError::InvalidMetadata(
            "workspace_root must not be empty".into(),
        ));
    }
    if metadata
        .workspace_id
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(BootstrapError::InvalidMetadata(
            "workspace_id must not be empty".into(),
        ));
    }
    Ok(())
}

fn canonical_directory(path: &Path) -> Result<PathBuf, BootstrapError> {
    let canonical = path
        .canonicalize()
        .map_err(|_| BootstrapError::ProjectRootMissing(path.to_path_buf()))?;
    if !canonical.is_dir() {
        return Err(BootstrapError::ProjectRootMissing(canonical));
    }
    Ok(canonical)
}

fn resolve_path(project_root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}
