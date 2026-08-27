//! Deterministic JSON encoding used for protocol and identity digests.
//!
//! serde_json's normal object encoding follows the map implementation's
//! iteration order. That order is an implementation detail at the protocol
//! boundary, so Pong sorts object member names recursively before writing
//! compact JSON bytes.

use crate::cas::{digest_for, Digest};
use crate::error::PongError;
use serde_json::{Map, Value};

/// Encode a JSON value without insignificant whitespace and with recursively
/// sorted object keys.
pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>, PongError> {
    let mut output = Vec::new();
    write_value(value, &mut output)?;
    Ok(output)
}

/// Explicit alias for callers that use the longer protocol terminology.
pub fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, PongError> {
    canonical_bytes(value)
}

/// Hash a canonical JSON value under a declared domain.
pub fn canonical_digest(domain: &str, value: &Value) -> Result<Digest, PongError> {
    validate_domain(domain)?;
    Ok(digest_for(domain, &canonical_bytes(value)?))
}

fn validate_domain(domain: &str) -> Result<(), PongError> {
    if domain.is_empty() {
        return Err(PongError::InvalidInput("domain must not be empty".into()));
    }
    if domain.as_bytes().contains(&0) {
        return Err(PongError::InvalidInput(
            "domain must not contain NUL".into(),
        ));
    }
    Ok(())
}

fn write_value(value: &Value, output: &mut Vec<u8>) -> Result<(), PongError> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => {
            if *value {
                output.extend_from_slice(b"true");
            } else {
                output.extend_from_slice(b"false");
            }
        }
        Value::Number(number) => output.extend_from_slice(number.to_string().as_bytes()),
        Value::String(string) => write_json(string, output)?,
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_value(value, output)?;
            }
            output.push(b']');
        }
        Value::Object(object) => write_object(object, output)?,
    }
    Ok(())
}

fn write_object(object: &Map<String, Value>, output: &mut Vec<u8>) -> Result<(), PongError> {
    let mut entries: Vec<(&String, &Value)> = object.iter().collect();
    // UTF-8 byte ordering is stable across platforms and gives one unambiguous
    // order for the wire representation.
    entries.sort_by(|(left, _), (right, _)| left.as_bytes().cmp(right.as_bytes()));

    output.push(b'{');
    for (index, (key, value)) in entries.into_iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        write_json(key, output)?;
        output.push(b':');
        write_value(value, output)?;
    }
    output.push(b'}');
    Ok(())
}

fn write_json<T: serde::Serialize>(value: &T, output: &mut Vec<u8>) -> Result<(), PongError> {
    serde_json::to_writer(output, value)
        .map_err(|error| PongError::Serialization(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn object_keys_are_sorted_recursively() {
        let value = json!({"z": {"b": 2, "a": 1}, "a": [{"d": 4, "c": 3}]});
        assert_eq!(
            canonical_bytes(&value).unwrap(),
            br#"{"a":[{"c":3,"d":4}],"z":{"a":1,"b":2}}"#
        );
    }

    #[test]
    fn equivalent_object_order_has_identical_bytes_and_digest() {
        let first: Value = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
        let second: Value = serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap();
        assert_eq!(
            canonical_bytes(&first).unwrap(),
            canonical_bytes(&second).unwrap()
        );
        assert_eq!(
            canonical_digest("test/object", &first).unwrap(),
            canonical_digest("test/object", &second).unwrap()
        );
    }

    #[test]
    fn strings_use_json_escaping() {
        let value = json!({"line\n": "quote \" and slash \\"});
        assert_eq!(
            canonical_bytes(&value).unwrap(),
            br#"{"line\n":"quote \" and slash \\"}"#
        );
    }

    #[test]
    fn domain_is_unambiguous() {
        let value = json!({"ok": true});
        assert!(matches!(
            canonical_digest("", &value),
            Err(PongError::InvalidInput(_))
        ));
        assert!(matches!(
            canonical_digest("bad\0domain", &value),
            Err(PongError::InvalidInput(_))
        ));
    }
}
