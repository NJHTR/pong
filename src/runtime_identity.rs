//! Minimal provider-runtime identity adapter.
//!
//! The adapter owns a small, local identity file for one logical Runtime
//! profile. It deliberately keeps provider run IDs separate from the durable
//! Pong Agent identity and does not add lifecycle fields to Protocol v1.0.

use crate::atomic_replace::rename_new_with_retry;
use crate::PongError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const RUNTIME_IDENTITY_SCHEMA_VERSION: u32 = 1;
pub const RUNTIME_IDENTITY_DIRECTORY: &str = "runtime-identities";

static ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIdentityMetadata {
    pub schema_version: u32,
    pub provider: String,
    pub agent_id: String,
    pub created_at: String,
}

/// One provider adapter instance. `session_id` is intentionally ephemeral;
/// `metadata.agent_id` is the stable logical identity recovered on restart.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIdentityAdapter {
    path: PathBuf,
    metadata: RuntimeIdentityMetadata,
    session_id: String,
}

impl RuntimeIdentityAdapter {
    /// Open or create the identity for one logical Runtime profile.
    pub fn open(path: impl AsRef<Path>, provider: &str) -> Result<Self, PongError> {
        let path = path.as_ref().to_path_buf();
        validate_provider(provider)?;
        let metadata = if path.is_file() {
            read_metadata(&path, provider)?
        } else {
            if path.exists() {
                return Err(PongError::InvalidInput(
                    "runtime identity path is not a regular file".into(),
                ));
            }
            let metadata = RuntimeIdentityMetadata {
                schema_version: RUNTIME_IDENTITY_SCHEMA_VERSION,
                provider: provider.to_owned(),
                agent_id: identity_token("agent", &path),
                created_at: format!("unix-ms:{}", unix_millis()),
            };
            write_new_metadata(&path, &metadata)?;
            read_metadata(&path, provider)?
        };

        Ok(Self {
            session_id: identity_token("session", &path),
            path,
            metadata,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn metadata(&self) -> &RuntimeIdentityMetadata {
        &self.metadata
    }

    pub fn agent_id(&self) -> &str {
        &self.metadata.agent_id
    }

    /// The adapter's connection/runtime context. Protocol v1.0 does not
    /// carry this value in `register_agent`; transports may correlate it
    /// outside the frozen domain request.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Stable provider metadata sent to `register_agent`. It must not include
    /// a per-run thread ID because Core treats registered Agent metadata as
    /// durable and rejects changed metadata on replay.
    pub fn provider_metadata(&self) -> &str {
        &self.metadata.provider
    }

    /// Preserve a provider run identity in an adapter-owned context without
    /// conflating it with the durable Pong Agent identity.
    pub fn provider_thread_metadata(&self, provider_thread_id: &str) -> Result<String, PongError> {
        if provider_thread_id.trim().is_empty() {
            return Err(PongError::InvalidInput(
                "provider thread identity must be non-empty".into(),
            ));
        }
        serde_json::to_string(&serde_json::json!({
            "provider": self.metadata.provider,
            "thread_id": provider_thread_id,
        }))
        .map_err(|error| PongError::Serialization(error.to_string()))
    }
}

fn validate_provider(provider: &str) -> Result<(), PongError> {
    if provider.trim().is_empty() {
        return Err(PongError::InvalidInput(
            "runtime provider must be non-empty".into(),
        ));
    }
    Ok(())
}

fn read_metadata(path: &Path, provider: &str) -> Result<RuntimeIdentityMetadata, PongError> {
    let bytes = fs::read(path)?;
    let metadata: RuntimeIdentityMetadata = serde_json::from_slice(&bytes)
        .map_err(|error| PongError::Serialization(error.to_string()))?;
    validate_metadata(&metadata, provider)?;
    Ok(metadata)
}

fn validate_metadata(metadata: &RuntimeIdentityMetadata, provider: &str) -> Result<(), PongError> {
    if metadata.schema_version != RUNTIME_IDENTITY_SCHEMA_VERSION {
        return Err(PongError::Unsupported(format!(
            "runtime identity schema version {} is not supported",
            metadata.schema_version
        )));
    }
    if metadata.provider != provider {
        return Err(PongError::Conflict(format!(
            "runtime identity belongs to provider {}",
            metadata.provider
        )));
    }
    if metadata.agent_id.trim().is_empty() || metadata.created_at.trim().is_empty() {
        return Err(PongError::Integrity(
            "runtime identity metadata is incomplete".into(),
        ));
    }
    Ok(())
}

fn write_new_metadata(path: &Path, metadata: &RuntimeIdentityMetadata) -> Result<(), PongError> {
    let parent = path.parent().ok_or_else(|| {
        PongError::InvalidInput("runtime identity path has no parent directory".into())
    })?;
    fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|error| PongError::Serialization(error.to_string()))?;
    let temp = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("runtime-identity"),
        std::process::id(),
        ID_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    match rename_new_with_retry(&temp, path) {
        Ok(()) => Ok(()),
        Err(_error) if path.is_file() => {
            let _ = fs::remove_file(&temp);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error)
        }
    }
}

fn identity_token(prefix: &str, path: &Path) -> String {
    let sequence = ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let material = format!(
        "{prefix}\0{}\0{}\0{}\0{}",
        path.display(),
        std::process::id(),
        sequence,
        now
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn identity_is_stable_but_session_is_per_adapter_instance() {
        let root = tempdir().unwrap();
        let path = root
            .path()
            .join(RUNTIME_IDENTITY_DIRECTORY)
            .join("codex.json");
        let first = RuntimeIdentityAdapter::open(&path, "codex").unwrap();
        let second = RuntimeIdentityAdapter::open(&path, "codex").unwrap();
        assert_eq!(first.agent_id(), second.agent_id());
        assert_ne!(first.session_id(), second.session_id());
    }
}
