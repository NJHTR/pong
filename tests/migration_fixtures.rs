use serde_json::Value;
use std::fs;

fn fixture(name: &str) -> Value {
    let path = format!("tests/fixtures/{name}");
    serde_json::from_str(&fs::read_to_string(path).expect("fixture file"))
        .expect("valid fixture JSON")
}

#[test]
fn previous_generation_fixture_preserves_identity_and_opaque_data() {
    let fixture = fixture("CT-08-previous-repository-generation.json");
    assert_eq!(fixture["expected"]["status"], "migration-required");
    assert_eq!(
        fixture["input"]["old_generation"]["generation_id"],
        "generation-0001"
    );
    assert_eq!(
        fixture["input"]["target_generation"]["generation_id"],
        "generation-0002"
    );
    assert_eq!(
        fixture["expected"]["projection"]["before_migration"]["schema_version"],
        "0.1"
    );
    assert_eq!(
        fixture["expected"]["projection"]["after_verified_migration"]["schema_version"],
        "0.2"
    );
    assert_eq!(
        fixture["expected"]["projection"]["event_identity_preserved"],
        true
    );
    assert_eq!(
        fixture["expected"]["projection"]["opaque_extension_preserved"],
        true
    );
}

#[test]
fn migration_interruption_fixture_requires_cold_reopen_and_forbids_mixed_state() {
    let fixture = fixture("FI-12-migration-selector-interruption.json");
    assert_eq!(fixture["expected"]["status"], "old-or-new-only");
    assert_eq!(fixture["fault"]["point"], "FI-12:selector_replace");
    assert_eq!(fixture["fault"]["restart_required"], true);
    assert_eq!(
        fixture["expected"]["projection"]["after_cold_reopen"]["selector_and_generation_match"],
        true
    );
    assert_eq!(
        fixture["expected"]["projection"]["after_cold_reopen"]["mixed_generation_visible"],
        false
    );
    assert_eq!(
        fixture["expected"]["projection"]["after_cold_reopen"]["retry_is_idempotent"],
        true
    );

    let phases = fixture["input"]["migration_phases"]
        .as_array()
        .expect("migration phases");
    assert_eq!(
        phases,
        &[
            Value::String("preflight".into()),
            Value::String("target_built".into()),
            Value::String("target_verified".into()),
            Value::String("selector_replace".into()),
            Value::String("selector_directory_sync".into()),
            Value::String("journal_finalized".into()),
        ]
    );
}
