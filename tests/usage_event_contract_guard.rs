use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn septa_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("mycelium should be inside basidiocarp workspace")
        .join("septa")
}

#[test]
fn usage_event_contract_exposes_fields_needed_for_deterministic_summaries() {
    let schema_path = septa_dir().join("usage-event-v1.schema.json");
    let fixture_path = septa_dir().join("fixtures/usage-event-v1.example.json");

    if !schema_path.exists() || !fixture_path.exists() {
        eprintln!("Skipping: Septa usage-event contract files not found");
        return;
    }

    let schema_text = fs::read_to_string(&schema_path).expect("read usage-event schema");
    let fixture_text = fs::read_to_string(&fixture_path).expect("read usage-event fixture");

    let schema: Value = serde_json::from_str(&schema_text).expect("parse usage-event schema");
    let fixture: Value = serde_json::from_str(&fixture_text).expect("parse usage-event fixture");

    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .expect("usage-event schema should have required array");
    let required_fields: Vec<&str> = required.iter().filter_map(Value::as_str).collect();

    for field in &[
        "schema_version",
        "event_kind",
        "captured_at_unix",
        "tool_name",
        "runtime",
        "scope",
        "usage",
        "origin",
    ] {
        assert!(
            required_fields.contains(field),
            "usage-event-v1 schema missing required field '{field}'"
        );
    }

    let usage = fixture
        .get("usage")
        .and_then(Value::as_object)
        .expect("usage fixture should include usage object");

    let input = usage
        .get("input_tokens")
        .and_then(Value::as_i64)
        .expect("usage fixture should include input_tokens");
    let output = usage
        .get("output_tokens")
        .and_then(Value::as_i64)
        .expect("usage fixture should include output_tokens");
    let cache_create = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_i64)
        .expect("usage fixture should include cache_creation_input_tokens");
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_i64)
        .expect("usage fixture should include cache_read_input_tokens");
    let total = usage
        .get("total_tokens")
        .and_then(Value::as_i64)
        .expect("usage fixture should include total_tokens");

    assert_eq!(
        total,
        input + output + cache_create + cache_read,
        "usage-event fixture total_tokens should match the summed counters"
    );

    let origin = fixture
        .get("origin")
        .and_then(Value::as_object)
        .expect("usage fixture should include origin");
    assert_eq!(
        origin
            .get("producer")
            .and_then(Value::as_str)
            .expect("usage fixture origin should include producer"),
        "cortina"
    );
}
