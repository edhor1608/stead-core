use jsonschema::validator_for;
use serde_json::json;
use stead_session_model::{
    BackendKind, EventActor, EventKind, EventPayload, SessionLineage, SessionMetadata,
    SessionSource, SteadEvent, SteadSession, build_session_uid, canonical_sort_events,
    schema_version,
};

fn load_schema() -> serde_json::Value {
    let schema_path = format!(
        "{}/../../schemas/session.v0.1.0.schema.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let contents = std::fs::read_to_string(schema_path).expect("schema file should exist");
    serde_json::from_str(&contents).expect("schema must be valid JSON")
}

fn valid_session() -> SteadSession {
    let mut events = vec![SteadEvent {
        event_uid: "ev-1".to_string(),
        stream_id: "main".to_string(),
        line_number: 1,
        sequence: None,
        timestamp: chrono::Utc::now(),
        kind: EventKind::MessageUser,
        actor: Some(EventActor::user("user")),
        payload: EventPayload::text("hello"),
        raw_vendor_payload: json!({}),
        extensions: serde_json::Map::new(),
    }];
    canonical_sort_events(&mut events);

    SteadSession {
        schema_version: schema_version().to_string(),
        session_uid: build_session_uid(BackendKind::Codex, "s1"),
        source: SessionSource::new(BackendKind::Codex, "s1", vec!["/tmp/s1.jsonl".to_string()]),
        metadata: SessionMetadata::new(None, "/tmp".into(), chrono::Utc::now(), chrono::Utc::now()),
        events,
        artifacts: vec![],
        capabilities: serde_json::Map::new(),
        extensions: serde_json::Map::new(),
        shared_session_uid: Some("stead:shared:s1".to_string()),
        lineage: Some(SessionLineage {
            root_session_uid: Some(build_session_uid(BackendKind::Codex, "root")),
            parent_session_uid: Some(build_session_uid(BackendKind::Codex, "parent")),
            fork_origin_event_uid: Some("ev-1".to_string()),
            strategy: Some("rewind".to_string()),
        }),
        raw_vendor_payload: json!({}),
    }
}

#[test]
fn valid_session_conforms_to_json_schema() {
    let schema = load_schema();
    let validator = validator_for(&schema).expect("schema should compile");

    let session = valid_session();
    let value = serde_json::to_value(session).expect("serialize session");
    assert!(validator.is_valid(&value), "session should validate");
}

#[test]
fn invalid_session_missing_required_field_is_rejected() {
    let schema = load_schema();
    let validator = validator_for(&schema).expect("schema should compile");

    let invalid = json!({
        "schema_version": "0.1.0",
        "source": {},
        "metadata": {},
        "events": []
    });

    assert!(!validator.is_valid(&invalid), "invalid payload should fail");
}
