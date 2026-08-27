mod property_support;

use pong_core::canonical::{canonical_bytes, canonical_digest};
use pong_core::cas::{digest_for, Cas};
use pong_core::metadata::{
    IdempotencyKey, IdempotencyResult, MetadataFailpoint, MetadataFailpoints, MetadataStore,
    NewEvent,
};
use pong_core::redaction::Redactor;
use property_support::{property_config, PURE_CASES, STORAGE_CASES};
use proptest::collection::{btree_map, btree_set, vec};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use serde_json::{Map, Number, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Display;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;

const PT01_SEED: u64 = 0x5054_0001_CAFE_0001;
const PT02_SEED: u64 = 0x5054_0002_CAFE_0002;
const PT03_SEED: u64 = 0x5054_0003_CAFE_0003;
const PT04_SEED: u64 = 0x5054_0004_CAFE_0004;
const PT05_SEED: u64 = 0x5054_0005_CAFE_0005;
const PT06_SEED: u64 = 0x5054_0006_CAFE_0006;
const PT07_SEED: u64 = 0x5054_0007_CAFE_0007;
const PT08_SEED: u64 = 0x5054_0008_CAFE_0008;
const PT11_SEED: u64 = 0x5054_0011_CAFE_0011;
const PT12_SEED: u64 = 0x5054_0012_CAFE_0012;

fn tc<T, E: Display>(result: Result<T, E>) -> Result<T, TestCaseError> {
    result.map_err(|error| TestCaseError::fail(error.to_string()))
}

fn identifier() -> impl Strategy<Value = String> {
    vec(b'a'..=b'z', 1..12).prop_map(|bytes| String::from_utf8(bytes).expect("ASCII generator"))
}

fn json_value() -> BoxedStrategy<Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i32>().prop_map(|value| Value::Number(Number::from(value))),
        vec(0x20u8..=0x7e, 0..24).prop_map(|bytes| {
            Value::String(String::from_utf8(bytes).expect("ASCII generator"))
        }),
    ];

    leaf.prop_recursive(3, 48, 8, |inner| {
        prop_oneof![
            vec(inner.clone(), 0..6).prop_map(Value::Array),
            btree_map(identifier(), inner, 0..6)
                .prop_map(|entries| { Value::Object(entries.into_iter().collect::<Map<_, _>>()) }),
        ]
    })
    .boxed()
}

fn reverse_object_insertion(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(reverse_object_insertion).collect()),
        Value::Object(object) => {
            let mut entries: Vec<_> = object.iter().collect();
            entries.reverse();
            let mut reordered = Map::new();
            for (key, value) in entries {
                reordered.insert(key.clone(), reverse_object_insertion(value));
            }
            Value::Object(reordered)
        }
        scalar => scalar.clone(),
    }
}

proptest! {
    #![proptest_config(property_config(PT01_SEED, PURE_CASES))]

    /// PT-01 (implemented subset): JSON map order is irrelevant recursively,
    /// canonical reparse is stable, and every exact canonical byte is hashed.
    #[test]
    fn pt01_canonical_json_and_covered_bytes_have_one_identity(
        value in json_value(),
        byte_index in any::<usize>(),
        delta in 1u8..=u8::MAX,
    ) {
        let reordered = reverse_object_insertion(&value);
        let first = tc(canonical_bytes(&value))?;
        let second = tc(canonical_bytes(&reordered))?;
        prop_assert_eq!(&first, &second);
        prop_assert_eq!(
            tc(canonical_digest("property/canonical", &value))?,
            tc(canonical_digest("property/canonical", &reordered))?,
        );

        let reparsed: Value = tc(serde_json::from_slice(&first))?;
        prop_assert_eq!(tc(canonical_bytes(&reparsed))?, first.clone());

        let mut changed = first.clone();
        let index = byte_index % changed.len();
        changed[index] ^= delta;
        prop_assert_ne!(
            digest_for("property/canonical", &first),
            digest_for("property/canonical", &changed),
        );
    }
}

proptest! {
    #![proptest_config(property_config(PT04_SEED, STORAGE_CASES))]

    /// PT-04 (ref subset): a generated writer ordering has exactly one winner
    /// for an expected-empty head, and every remaining writer is stale. Lease
    /// epochs are not implemented by the current durable-primitive port.
    #[test]
    fn pt04_generated_ref_writer_order_has_one_cas_winner(
        targets in btree_set(any::<u32>(), 2..10),
        order_keys in vec(any::<u64>(), 2..10),
    ) {
        let mut targets: Vec<_> = targets
            .into_iter()
            .map(|value| format!("commit-{value}"))
            .collect();
        prop_assume!(targets.len() >= 2);
        targets.sort_by_key(|target| {
            let index = target
                .bytes()
                .fold(0usize, |acc, byte| acc.wrapping_add(byte as usize));
            order_keys[index % order_keys.len()]
        });

        let mut metadata = tc(MetadataStore::in_memory())?;
        let mut winners = Vec::new();
        for target in &targets {
            match metadata.compare_and_swap_ref("refs/heads/main", None, target, "t") {
                Ok(()) => winners.push(target.clone()),
                Err(error) => prop_assert_eq!(error.code(), "CONFLICT"),
            }
        }
        prop_assert_eq!(winners.len(), 1);
        prop_assert_eq!(
            tc(metadata.get_ref("refs/heads/main"))?,
            Some(winners[0].clone()),
        );

        for target in &targets {
            let error = metadata
                .compare_and_swap_ref("refs/heads/main", Some("stale"), target, "t2")
                .expect_err("stale writer must fail");
            prop_assert_eq!(error.code(), "CONFLICT");
        }
        prop_assert_eq!(
            tc(metadata.get_ref("refs/heads/main"))?,
            Some(winners[0].clone()),
        );
    }
}

proptest! {
    #![proptest_config(property_config(PT05_SEED, STORAGE_CASES))]

    /// PT-05 (implemented stream subset): timestamps and inter-stream append
    /// order never affect contiguous per-stream sequence allocation. Causal
    /// links are not exercised by this legacy-port property; project cursors
    /// are covered by the dedicated projection integration tests.
    #[test]
    fn pt05_event_sequences_are_contiguous_per_stream_and_duplicates_converge(
        actions in vec((0u8..4, any::<i64>(), any::<bool>()), 1..48),
    ) {
        let mut metadata = tc(MetadataStore::in_memory())?;
        let mut expected_counts = BTreeMap::<String, usize>::new();
        for (index, (stream, timestamp, duplicate)) in actions.into_iter().enumerate() {
            let stream_id = format!("stream:{}", stream % 4);
            let event = NewEvent {
                event_id: format!("event-{index}"),
                project_id: "project-property".into(),
                stream_id: stream_id.clone(),
                schema_version: "0.1".into(),
                actor_id: Some("actor-property".into()),
                request_id: Some(format!("request-{index}")),
                occurred_at: timestamp.to_string(),
                payload: serde_json::json!({"index": index}),
            };
            let sequence = tc(metadata.append_event(event.clone()))?.sequence;
            let count = expected_counts.entry(stream_id).or_default();
            *count += 1;
            prop_assert_eq!(sequence, *count as i64);
            if duplicate {
                prop_assert_eq!(tc(metadata.append_event(event))?.sequence, sequence);
            }
        }

        for (stream_id, expected_count) in expected_counts {
            let events = tc(metadata.list_events(&stream_id))?;
            prop_assert_eq!(events.len(), expected_count);
            for (index, event) in events.iter().enumerate() {
                prop_assert_eq!(event.sequence, index as i64 + 1);
            }
        }
    }
}

proptest! {
    #![proptest_config(property_config(PT06_SEED, STORAGE_CASES))]

    /// PT-06 (event-store convergence subset): arbitrary permutations and
    /// repetitions of one valid event batch converge to exactly one immutable
    /// stored event per ID. Projection convergence is covered separately.
    #[test]
    fn pt06_duplicate_event_permutations_converge_in_the_event_store(
        event_count in 1usize..24,
        deliveries in vec(any::<usize>(), 1..96),
    ) {
        let batch: Vec<_> = (0..event_count)
            .map(|index| NewEvent {
                event_id: format!("event-{index}"),
                project_id: "project-property".into(),
                stream_id: "stream:duplicates".into(),
                schema_version: "0.1".into(),
                actor_id: Some("actor-property".into()),
                request_id: Some(format!("request-{index}")),
                occurred_at: format!("timestamp-{}", event_count - index),
                payload: serde_json::json!({"index": index}),
            })
            .collect();

        let mut order: Vec<_> = deliveries
            .into_iter()
            .map(|index| index % event_count)
            .collect();
        order.extend(0..event_count);

        let mut metadata = tc(MetadataStore::in_memory())?;
        let mut first_sequences = BTreeMap::new();
        for index in order {
            let stored = tc(metadata.append_event(batch[index].clone()))?;
            if let Some(first) = first_sequences.insert(index, stored.sequence) {
                prop_assert_eq!(stored.sequence, first);
            }
        }

        let events = tc(metadata.list_events("stream:duplicates"))?;
        prop_assert_eq!(events.len(), event_count);
        let observed: BTreeSet<_> =
            events.iter().map(|event| event.event_id.as_str()).collect();
        let expected: BTreeSet<_> =
            batch.iter().map(|event| event.event_id.as_str()).collect();
        prop_assert_eq!(observed, expected);
        for (index, event) in events.iter().enumerate() {
            prop_assert_eq!(event.sequence, index as i64 + 1);
        }
    }
}

proptest! {
    #![proptest_config(property_config(PT07_SEED, STORAGE_CASES))]

    /// PT-07: any positive number of canonical-equivalent retries returns the
    /// original result and never produces a second newly-recorded transition.
    #[test]
    fn pt07_same_key_and_digest_has_one_idempotency_transition(
        suffix in any::<u64>(),
        retries in 1usize..24,
    ) {
        let mut metadata = tc(MetadataStore::in_memory())?;
        let key = IdempotencyKey {
            project_id: format!("project-{suffix}"),
            actor_id: format!("actor-{suffix}"),
            request_id: format!("request-{suffix}"),
        };
        let original = serde_json::json!({"status": "accepted", "value": suffix});
        prop_assert_eq!(
            tc(metadata.record_idempotency(&key, "command:stable", &original, "t1"))?,
            IdempotencyResult::NewlyRecorded,
        );
        for _ in 0..retries {
            let reordered: Value = tc(serde_json::from_str(
                &format!(r#"{{"value":{suffix},"status":"accepted"}}"#),
            ))?;
            prop_assert_eq!(
                tc(metadata.record_idempotency(
                    &key,
                    "command:stable",
                    &reordered,
                    "later",
                ))?,
                IdempotencyResult::Existing(original.clone()),
            );
        }
    }
}

proptest! {
    #![proptest_config(property_config(PT08_SEED, STORAGE_CASES))]

    /// PT-08: changed command bytes fail closed under the same full key;
    /// actor/project changes create independent scoped records and never alias
    /// the original result.
    #[test]
    fn pt08_key_reuse_is_rejected_and_scopes_do_not_alias(
        project in identifier(),
        actor in identifier(),
        request in identifier(),
        command in identifier(),
    ) {
        let mut metadata = tc(MetadataStore::in_memory())?;
        let key = IdempotencyKey {
            project_id: format!("project-{project}"),
            actor_id: format!("actor-{actor}"),
            request_id: format!("request-{request}"),
        };
        let original = serde_json::json!({"scope": "original"});
        prop_assert_eq!(
            tc(metadata.record_idempotency(&key, &command, &original, "t1"))?,
            IdempotencyResult::NewlyRecorded,
        );
        let error = metadata
            .record_idempotency(&key, &format!("{command}:changed"), &original, "t2")
            .expect_err("same key with changed command must fail");
        prop_assert_eq!(error.code(), "IDEMPOTENCY_KEY_REUSE");

        for scoped in [
            IdempotencyKey {
                actor_id: format!("{}-other", key.actor_id),
                ..key.clone()
            },
            IdempotencyKey {
                project_id: format!("{}-other", key.project_id),
                ..key.clone()
            },
        ] {
            prop_assert_eq!(
                tc(metadata.record_idempotency(
                    &scoped,
                    &command,
                    &serde_json::json!({"scope": "independent"}),
                    "t3",
                ))?,
                IdempotencyResult::NewlyRecorded,
            );
        }
        prop_assert_eq!(
            tc(metadata.record_idempotency(&key, &command, &original, "t4"))?,
            IdempotencyResult::Existing(original),
        );
    }
}

proptest! {
    #![proptest_config(property_config(PT02_SEED, STORAGE_CASES))]

    /// PT-02: repeated and genuinely concurrent identical publications
    /// converge to one verified immutable object.
    #[test]
    fn pt02_concurrent_identical_cas_puts_converge(
        bytes in vec(any::<u8>(), 0..2048),
        writers in 1usize..6,
        repeats in 1usize..5,
    ) {
        let directory = tc(tempdir())?;
        let cas = Arc::new(tc(Cas::new(directory.path().join("cas")))?);
        let expected = digest_for("property/cas", &bytes);
        let handles: Vec<_> = (0..writers)
            .map(|_| {
                let cas = Arc::clone(&cas);
                let bytes = bytes.clone();
                thread::spawn(move || {
                    for _ in 0..repeats {
                        assert_eq!(cas.put("property/cas", &bytes).expect("CAS put"), expected);
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("CAS writer thread");
        }

        prop_assert!(cas.exists(expected));
        prop_assert_eq!(tc(cas.get("property/cas", expected))?, bytes);
        prop_assert!(cas.object_path(expected).is_file());
        prop_assert_eq!(tc(fs::read_dir(cas.root().join("staging")))?.count(), 0);
    }
}

#[derive(Clone, Copy, Debug)]
enum AtomicMutation {
    Ref,
    Idempotency,
    Event,
    Intent,
}

fn atomic_mutation() -> impl Strategy<Value = AtomicMutation> {
    prop_oneof![
        Just(AtomicMutation::Ref),
        Just(AtomicMutation::Idempotency),
        Just(AtomicMutation::Event),
        Just(AtomicMutation::Intent),
    ]
}

proptest! {
    #![proptest_config(property_config(PT03_SEED, STORAGE_CASES))]

    /// PT-03 (current port scope): each exposed metadata mutation is absent
    /// before COMMIT and complete after COMMIT, including failed responses.
    /// No public multi-row batch transaction exists yet.
    #[test]
    fn pt03_metadata_commit_boundary_is_pre_or_post_state(
        mutation in atomic_mutation(),
        after_commit in any::<bool>(),
        suffix in any::<u64>(),
    ) {
        let directory = tc(tempdir())?;
        let path = directory.path().join("metadata.sqlite");
        let point = if after_commit {
            MetadataFailpoint::AfterSqliteCommit
        } else {
            MetadataFailpoint::BeforeSqliteCommit
        };
        let mut metadata = tc(MetadataStore::open_with_failpoints(
            &path,
            MetadataFailpoints::once(point),
        ))?;
        let key = IdempotencyKey {
            project_id: format!("project-{suffix}"),
            actor_id: format!("actor-{suffix}"),
            request_id: format!("request-{suffix}"),
        };
        let result = match mutation {
            AtomicMutation::Ref => metadata.compare_and_swap_ref(
                "refs/heads/property",
                None,
                &format!("commit-{suffix}"),
                "t1",
            ),
            AtomicMutation::Idempotency => metadata
                .record_idempotency(&key, "command:a", &serde_json::json!({"n": suffix}), "t1")
                .map(|_| ()),
            AtomicMutation::Event => metadata
                .append_event(NewEvent {
                    event_id: format!("event-{suffix}"),
                    project_id: key.project_id.clone(),
                    stream_id: "stream:property".into(),
                    schema_version: "0.1".into(),
                    actor_id: Some(key.actor_id.clone()),
                    request_id: Some(key.request_id.clone()),
                    occurred_at: "t1".into(),
                    payload: serde_json::json!({"n": suffix, "nested": {"ok": true}}),
                })
                .map(|_| ()),
            AtomicMutation::Intent => metadata.record_intent(
                &key.project_id,
                &key.actor_id,
                &key.request_id,
                &format!("operation-{suffix}"),
                &serde_json::json!({"n": suffix}),
                "t1",
            ),
        };
        prop_assert_eq!(result.expect_err("failpoint must fire").code(), "FAULT_INJECTED");
        drop(metadata);

        let mut reopened = tc(MetadataStore::open(&path))?;
        match mutation {
            AtomicMutation::Ref => prop_assert_eq!(
                tc(reopened.get_ref("refs/heads/property"))?,
                after_commit.then(|| format!("commit-{suffix}")),
            ),
            AtomicMutation::Idempotency => {
                let observed = tc(reopened.record_idempotency(
                    &key,
                    "command:a",
                    &serde_json::json!({"n": suffix}),
                    "t2",
                ))?;
                if after_commit {
                    prop_assert!(matches!(observed, IdempotencyResult::Existing(_)));
                } else {
                    prop_assert_eq!(observed, IdempotencyResult::NewlyRecorded);
                }
            }
            AtomicMutation::Event => {
                let events = tc(reopened.list_events("stream:property"))?;
                prop_assert_eq!(events.len(), usize::from(after_commit));
                if let Some(event) = events.first() {
                    prop_assert_eq!(&event.event_id, &format!("event-{suffix}"));
                    prop_assert_eq!(
                        &event.payload_json,
                        &format!(r#"{{"n":{suffix},"nested":{{"ok":true}}}}"#),
                    );
                }
            }
            AtomicMutation::Intent => {
                let operation = tc(reopened.operation(&key))?;
                prop_assert_eq!(operation.is_some(), after_commit);
                if let Some(operation) = operation {
                    prop_assert_eq!(operation.phase, "intent_durable");
                    prop_assert_eq!(operation.payload_json, format!(r#"{{"n":{suffix}}}"#));
                }
            }
        }
    }
}

fn secret_payload(secret: &str, noise: &str) -> Value {
    serde_json::json!({
        "fields": {"message": format!("{noise}:{secret}")},
        "nested": [{"deep": secret}],
        "argv": ["tool", format!("--credential={secret}")],
        "environment": {"SERVICE_TOKEN": secret, "SAFE": noise},
        "logs": [format!("provider returned {secret}")],
        "artifacts": {"text": format!("artifact:{secret}")},
    })
}

proptest! {
    #![proptest_config(property_config(PT11_SEED, STORAGE_CASES / 2))]

    /// PT-11 (available durable surfaces): generated field/nested/argv/env/log
    /// and artifact-shaped placements are absent from SQLite/CAS bytes. CAS
    /// rejects raw secret bytes and accepts only the redacted representation.
    #[test]
    fn pt11_registered_secrets_are_closed_over_persisted_metadata_and_cas(
        secret_number in any::<u64>(),
        noise in identifier(),
    ) {
        let secret = format!("pong-secret-{secret_number:016x}");
        let payload = secret_payload(&secret, &noise);
        let mut redactor = tc(Redactor::new("property-profile", "0.1"))?;
        tc(redactor.register_secret(&secret))?;

        let directory = tc(tempdir())?;
        let pong = directory.path().join(".pong");
        tc(fs::create_dir_all(&pong))?;
        let metadata_path = pong.join("metadata.sqlite");
        let mut metadata = tc(MetadataStore::open_with_redactor(
            metadata_path,
            redactor.clone(),
        ))?;
        let key = IdempotencyKey {
            project_id: format!("project-{secret}"),
            actor_id: format!("actor-{secret}"),
            request_id: format!("request-{secret}"),
        };
        tc(metadata.append_event(NewEvent {
            event_id: format!("event-{secret}"),
            project_id: key.project_id.clone(),
            stream_id: format!("stream-{secret}"),
            schema_version: "0.1".into(),
            actor_id: Some(key.actor_id.clone()),
            request_id: Some(key.request_id.clone()),
            occurred_at: format!("time-{secret}"),
            payload: payload.clone(),
        }))?;
        tc(metadata.record_idempotency(
            &key,
            &format!("digest-{secret}"),
            &payload,
            &format!("created-{secret}"),
        ))?;
        tc(metadata.record_intent(
            &key.project_id,
            &key.actor_id,
            &format!("journal-{secret}"),
            &format!("operation-{secret}"),
            &payload,
            &format!("created-{secret}"),
        ))?;
        tc(metadata.compare_and_swap_ref(
            &format!("refs/{secret}"),
            None,
            &format!("commit-{secret}"),
            &format!("updated-{secret}"),
        ))?;
        drop(metadata);

        let cas = tc(Cas::new_with_redactor(
            pong.join("cas"),
            redactor.clone(),
        ))?;
        let raw = tc(canonical_bytes(&payload))?;
        let error = cas
            .put("property/secret", &raw)
            .expect_err("raw CAS secret must fail");
        prop_assert_eq!(error.code(), "INTEGRITY_ERROR");
        let redacted = tc(canonical_bytes(&redactor.redact_value(&payload)))?;
        let digest = tc(cas.put("property/secret", &redacted))?;
        prop_assert_eq!(tc(cas.get("property/secret", digest))?, redacted);
        drop(cas);

        tc(assert_tree_excludes(directory.path(), secret.as_bytes()))?;
    }
}

proptest! {
    #![proptest_config(property_config(PT12_SEED, PURE_CASES))]

    /// PT-12: equivalent input/profile produces exactly one recursive marker
    /// representation, with no raw secret or secret digest in the output.
    #[test]
    fn pt12_redaction_is_deterministic_and_uses_an_opaque_fixed_marker(
        secret_number in any::<u64>(),
        noise in identifier(),
    ) {
        let secret = format!("pong-secret-{secret_number:016x}");
        let payload = secret_payload(&secret, &noise);
        let mut first = tc(Redactor::new("property-profile", "0.1"))?;
        tc(first.register_secret(&secret))?;
        let mut second = tc(Redactor::new("property-profile", "0.1"))?;
        tc(second.register_secret(&secret))?;

        let left = first.redact_value(&payload);
        let right = second.redact_value(&reverse_object_insertion(&payload));
        prop_assert_eq!(tc(canonical_bytes(&left))?, tc(canonical_bytes(&right))?);
        prop_assert_eq!(first.profile(), second.profile());

        let persisted = tc(canonical_bytes(&left))?;
        prop_assert!(!persisted
            .windows(secret.len())
            .any(|window| window == secret.as_bytes()));
        let secret_digest = digest_for("property/redaction", secret.as_bytes()).to_hex();
        prop_assert!(!String::from_utf8_lossy(&persisted).contains(&secret_digest));
        prop_assert!(String::from_utf8_lossy(&persisted).contains("[REDACTED]"));
    }
}

fn assert_tree_excludes(root: &Path, needle: &[u8]) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            assert_tree_excludes(&path, needle)?;
        } else if metadata.is_file() {
            let bytes = fs::read(&path)?;
            if bytes.windows(needle.len()).any(|window| window == needle) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("configured secret appeared in {}", path.display()),
                ));
            }
        }
    }
    Ok(())
}
