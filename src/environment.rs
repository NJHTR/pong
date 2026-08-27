//! Safe, deterministic environment facts for workspace/snapshot metadata.
//!
//! Capture is deliberately allowlist-based. Host paths, usernames, complete
//! process environments, and secret-shaped keys are excluded before the
//! value reaches canonical encoding or SQLite.

use crate::canonical::{canonical_bytes, canonical_digest};
use crate::cas::Digest;
use crate::error::PongError;
use crate::metadata::{EnvironmentRecord, MetadataStore};
use crate::redaction::Redactor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const ENVIRONMENT_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentFacts {
    pub schema_version: String,
    pub os: String,
    pub architecture: String,
    pub family: String,
    pub variables: BTreeMap<String, String>,
}

impl EnvironmentFacts {
    pub fn capture(redactor: &Redactor, allowlist: &[&str]) -> Result<Self, PongError> {
        let mut variables = BTreeMap::new();
        for key in allowlist {
            validate_env_key(key)?;
            if is_sensitive_key(key) || is_host_identity_key(key) {
                continue;
            }
            if let Some(value) = std::env::var_os(key) {
                let value = value.to_str().ok_or_else(|| {
                    PongError::InvalidInput("allowlisted environment value is not UTF-8".into())
                })?;
                let value = redactor.redact_text(value);
                // A redactor may intentionally replace a configured secret,
                // but an unregistered secret-shaped value must never enter
                // ordinary environment history.
                if looks_secret_like(&value) {
                    continue;
                }
                variables.insert((*key).to_owned(), value);
            }
        }
        Ok(Self {
            schema_version: ENVIRONMENT_SCHEMA_VERSION.into(),
            os: std::env::consts::OS.into(),
            architecture: std::env::consts::ARCH.into(),
            family: std::env::consts::FAMILY.into(),
            variables,
        })
    }

    pub fn to_value(&self) -> Result<Value, PongError> {
        serde_json::to_value(self).map_err(|error| {
            PongError::Serialization(format!("cannot encode environment facts: {error}"))
        })
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PongError> {
        canonical_bytes(&self.to_value()?)
    }

    pub fn fingerprint(&self) -> Result<Digest, PongError> {
        canonical_digest("environment/v1", &self.to_value()?)
    }

    pub fn persist(
        &self,
        metadata: &mut MetadataStore,
        environment_id: &str,
        project_id: &str,
        created_at: &str,
    ) -> Result<EnvironmentRecord, PongError> {
        metadata.record_environment(environment_id, project_id, &self.to_value()?, created_at)
    }
}

fn validate_env_key(key: &str) -> Result<(), PongError> {
    if key.is_empty()
        || key.len() > 128
        || key.as_bytes().contains(&0)
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(PongError::InvalidInput(
            "environment allowlist key is invalid".into(),
        ));
    }
    Ok(())
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    [
        "secret",
        "password",
        "passwd",
        "token",
        "api_key",
        "apikey",
        "authorization",
        "cookie",
        "private_key",
        "credential",
    ]
    .iter()
    .any(|part| normalized == *part || normalized.contains(part))
}

fn is_host_identity_key(key: &str) -> bool {
    let normalized = key.to_ascii_uppercase();
    normalized.contains("PATH")
        || matches!(
            normalized.as_str(),
            "HOME"
                | "USER"
                | "USERNAME"
                | "USERPROFILE"
                | "PWD"
                | "OLDPWD"
                | "TMP"
                | "TEMP"
                | "TMPDIR"
                | "HOST"
                | "HOSTNAME"
                | "COMPUTERNAME"
        )
        || normalized.starts_with("SSH_")
        || normalized.starts_with("XDG_")
}

fn looks_secret_like(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("sk-")
        || lower.starts_with("ghp_")
        || lower.starts_with("xoxb-")
        || lower.starts_with("bearer ")
        || (value.len() >= 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_./+=".contains(&byte)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::MetadataStore;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn capture_is_allowlisted_and_deterministic() {
        let mut redactor = Redactor::default();
        redactor.register_secret("environment-secret").unwrap();
        let first =
            EnvironmentFacts::capture(&redactor, &["PATH", "PONG_TEST_SECRET"]).expect("capture");
        let second =
            EnvironmentFacts::capture(&redactor, &["PATH", "PONG_TEST_SECRET"]).expect("capture");
        assert_eq!(first, second);
        assert!(!first.variables.contains_key("PONG_TEST_SECRET"));
        assert_eq!(first.fingerprint().unwrap(), second.fingerprint().unwrap());
    }

    #[test]
    fn persisted_environment_identity_is_immutable() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("metadata.sqlite");
        let mut metadata = MetadataStore::open(path).unwrap();
        let facts = EnvironmentFacts {
            schema_version: ENVIRONMENT_SCHEMA_VERSION.into(),
            os: "test-os".into(),
            architecture: "test-arch".into(),
            family: "test-family".into(),
            variables: BTreeMap::from([("SAFE".into(), "value".into())]),
        };
        let first = facts
            .persist(&mut metadata, "env-1", "project-1", "t1")
            .unwrap();
        assert_eq!(metadata.environment("env-1").unwrap(), Some(first.clone()));
        let changed = json!({"schema_version":"0.1","os":"other"});
        assert!(matches!(
            metadata.record_environment("env-1", "project-1", &changed, "t2"),
            Err(PongError::Integrity(_))
        ));
    }
}
