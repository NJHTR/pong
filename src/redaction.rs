use crate::error::PongError;
use serde_json::{Map, Value};
use std::fmt;

const REDACTED: &str = "[REDACTED]";

/// Public, non-secret identity for a redaction policy.
///
/// The profile deliberately contains only the policy identity and version;
/// registered secret values are never exposed through this type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedactionProfile {
    pub id: String,
    pub version: String,
}

/// Deterministic, pre-persistence redaction policy for structured payloads.
#[derive(Clone, PartialEq, Eq)]
pub struct Redactor {
    profile_id: String,
    version: String,
    secrets: Vec<String>,
}

impl fmt::Debug for Redactor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Redactor")
            .field("profile_id", &self.redact_text(&self.profile_id))
            .field("version", &self.redact_text(&self.version))
            .field("secret_count", &self.secrets.len())
            .finish()
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Self {
            profile_id: "default".into(),
            version: "0.1".into(),
            secrets: Vec::new(),
        }
    }
}

impl Redactor {
    pub fn new(
        profile_id: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, PongError> {
        let profile_id = profile_id.into();
        let version = version.into();
        if profile_id.is_empty() || version.is_empty() {
            return Err(PongError::InvalidInput(
                "redaction profile id and version must not be empty".into(),
            ));
        }
        Ok(Self {
            profile_id,
            version,
            secrets: Vec::new(),
        })
    }

    pub fn register_secret(&mut self, secret: impl Into<String>) -> Result<(), PongError> {
        let secret = secret.into();
        if secret.len() < 4 {
            return Err(PongError::InvalidInput(
                "registered secrets must contain at least four bytes".into(),
            ));
        }
        if !self.secrets.contains(&secret) {
            self.secrets.push(secret);
            self.secrets
                .sort_by_key(|value| std::cmp::Reverse(value.len()));
        }
        Ok(())
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// Return the policy identity without exposing registered secret values.
    pub fn profile(&self) -> RedactionProfile {
        RedactionProfile {
            id: self.profile_id.clone(),
            version: self.version.clone(),
        }
    }

    pub fn redact_text(&self, value: &str) -> String {
        let mut redacted = value.to_owned();
        for secret in &self.secrets {
            redacted = redacted.replace(secret, REDACTED);
        }
        redacted
    }

    /// Return whether any registered secret occurs as an exact UTF-8 byte
    /// sequence. This is used by startup and test scanners; it never exposes
    /// the matching secret in an error message.
    pub fn contains_secret(&self, bytes: &[u8]) -> bool {
        self.secrets.iter().any(|secret| {
            let secret_bytes = secret.as_bytes();
            bytes
                .windows(secret_bytes.len())
                .any(|window| window == secret_bytes)
        })
    }

    /// Fail closed when a durable byte region still contains a configured
    /// secret. The caller supplies only a safe location label for diagnostics.
    pub fn assert_clean_bytes(&self, bytes: &[u8], location: &str) -> Result<(), PongError> {
        if self.contains_secret(bytes) {
            return Err(PongError::Integrity(format!(
                "configured secret detected in persisted bytes at {location}"
            )));
        }
        Ok(())
    }

    pub fn redact_value(&self, value: &Value) -> Value {
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
            Value::String(value) => Value::String(self.redact_text(value)),
            Value::Array(values) => Value::Array(
                values
                    .iter()
                    .map(|value| self.redact_value(value))
                    .collect(),
            ),
            Value::Object(object) => {
                let mut redacted = Map::new();
                for (key, value) in object {
                    let mut redacted_key = self.redact_text(key);
                    if redacted.contains_key(&redacted_key) {
                        // Secret-bearing keys can collapse to the same marker.
                        // Keep the result deterministic while preserving both
                        // values without retaining the original secret.
                        let base = redacted_key.clone();
                        let mut suffix = 1usize;
                        while redacted.contains_key(&redacted_key) {
                            redacted_key = format!("{base}#{suffix}");
                            suffix += 1;
                        }
                    }
                    let value = if is_sensitive_key(key) {
                        Value::String(REDACTED.into())
                    } else {
                        self.redact_value(value)
                    };
                    redacted.insert(redacted_key, value);
                }
                Value::Object(redacted)
            }
        }
    }
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
    ]
    .iter()
    .any(|part| normalized == *part || normalized.ends_with(&format!("_{part}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redaction_is_deterministic_and_recursive() {
        let mut redactor = Redactor::new("test", "0.1").unwrap();
        redactor.register_secret("sk-test-secret").unwrap();
        let value = json!({
            "nested": ["sk-test-secret", {"password": "not persisted"}],
            "safe": "ok"
        });
        let expected = json!({
            "nested": ["[REDACTED]", {"password": "[REDACTED]"}],
            "safe": "ok"
        });
        assert_eq!(redactor.redact_value(&value), expected);
        assert_eq!(redactor.redact_value(&value), expected);
    }

    #[test]
    fn short_registered_secrets_are_rejected() {
        let mut redactor = Redactor::default();
        assert!(matches!(
            redactor.register_secret("abc"),
            Err(PongError::InvalidInput(_))
        ));
    }

    #[test]
    fn secret_bearing_object_keys_are_replaced_without_collisions() {
        let mut redactor = Redactor::default();
        redactor.register_secret("key-secret").unwrap();
        let value = json!({"key-secret": 1, "[REDACTED]": 2});
        let redacted = redactor.redact_value(&value);
        assert_eq!(redacted, json!({"[REDACTED]": 2, "[REDACTED]#1": 1}));
        assert!(!redactor.contains_secret(serde_json::to_string(&redacted).unwrap().as_bytes()));
    }

    #[test]
    fn byte_scan_fails_closed_without_disclosing_secret() {
        let mut redactor = Redactor::default();
        redactor.register_secret("scan-secret").unwrap();
        let error = redactor
            .assert_clean_bytes(b"prefix scan-secret suffix", "fixture")
            .expect_err("secret must be rejected");
        assert_eq!(error.code(), "INTEGRITY_ERROR");
        assert!(!error.to_string().contains("scan-secret"));
    }

    #[test]
    fn debug_representation_does_not_expose_registered_secrets() {
        let mut redactor = Redactor::default();
        redactor.register_secret("debug-secret").unwrap();
        let debug = format!("{redactor:?}");
        assert!(!debug.contains("debug-secret"));
        assert!(debug.contains("secret_count"));
    }
}
