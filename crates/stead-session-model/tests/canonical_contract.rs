use chrono::{TimeZone, Utc};
use stead_session_model::{
    BackendKind, EventActor, EventKind, EventPayload, SessionLineage, SessionMetadata,
    SessionSource, SteadEvent, SteadSession, SteadSessionError, build_session_uid,
    canonical_sort_events, schema_version,
};

fn sample_events() -> Vec<SteadEvent> {
    vec![
        SteadEvent {
            event_uid: "ev-2".to_string(),
            stream_id: "subagent-a".to_string(),
            line_number: 2,
            sequence: None,
            timestamp: Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 1).unwrap(),
            kind: EventKind::MessageAssistant,
            actor: Some(EventActor::assistant("assistant")),
            payload: EventPayload::text("Done."),
            raw_vendor_payload: serde_json::json!({ "raw": "assistant" }),
            extensions: serde_json::Map::new(),
        },
        SteadEvent {
            event_uid: "ev-1".to_string(),
            stream_id: "main".to_string(),
            line_number: 1,
            sequence: None,
            timestamp: Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 0).unwrap(),
            kind: EventKind::MessageUser,
            actor: Some(EventActor::user("user")),
            payload: EventPayload::text("Implement this."),
            raw_vendor_payload: serde_json::json!({ "raw": "user" }),
            extensions: serde_json::Map::new(),
        },
    ]
}

#[test]
fn schema_version_is_locked() {
    assert_eq!(schema_version(), "0.1.0");
}

#[test]
fn event_sort_is_deterministic_and_main_stream_first() {
    let mut events = sample_events();
    canonical_sort_events(&mut events);

    let ordered_ids: Vec<&str> = events.iter().map(|ev| ev.event_uid.as_str()).collect();
    assert_eq!(ordered_ids, vec!["ev-1", "ev-2"]);
    assert_eq!(events[0].sequence, Some(0));
    assert_eq!(events[1].sequence, Some(1));
}

#[test]
fn session_uid_builder_is_stable() {
    let a = build_session_uid(BackendKind::Codex, "abc");
    let b = build_session_uid(BackendKind::Codex, "abc");
    let c = build_session_uid(BackendKind::ClaudeCode, "abc");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert!(a.starts_with("stead:codex:"));
}

#[test]
fn session_validation_requires_event_sequence() {
    let session = SteadSession {
        schema_version: schema_version().to_string(),
        session_uid: build_session_uid(BackendKind::Codex, "abc"),
        source: SessionSource::new(BackendKind::Codex, "abc", vec!["/tmp/source.jsonl".into()]),
        metadata: SessionMetadata::new(
            Some("TDD session".into()),
            "/Users/jonas/repos/stead".into(),
            Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 1).unwrap(),
        ),
        events: sample_events(),
        artifacts: vec![],
        capabilities: serde_json::Map::new(),
        extensions: serde_json::Map::new(),
        lineage: None,
        raw_vendor_payload: serde_json::json!({}),
    };

    let err = session.validate().expect_err("missing sequence must fail");
    assert!(matches!(err, SteadSessionError::MissingSequence { .. }));
}

#[test]
fn session_can_store_optional_lineage_metadata() {
    let mut events = sample_events();
    canonical_sort_events(&mut events);

    let session = SteadSession {
        schema_version: schema_version().to_string(),
        session_uid: build_session_uid(BackendKind::Codex, "child-session"),
        source: SessionSource::new(
            BackendKind::Codex,
            "child-session",
            vec!["/tmp/source.jsonl".into()],
        ),
        metadata: SessionMetadata::new(
            Some("lineage".into()),
            "/Users/jonas/repos/stead".into(),
            Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 1).unwrap(),
        ),
        events,
        artifacts: vec![],
        capabilities: serde_json::Map::new(),
        extensions: serde_json::Map::new(),
        lineage: Some(SessionLineage {
            root_session_uid: Some(build_session_uid(BackendKind::Codex, "root-session")),
            parent_session_uid: Some(build_session_uid(BackendKind::Codex, "parent-session")),
            fork_origin_event_uid: Some("ev-1".into()),
            strategy: Some("rewind".into()),
        }),
        raw_vendor_payload: serde_json::json!({}),
    };

    let value = serde_json::to_value(session).expect("serialize with lineage");
    assert_eq!(value["lineage"]["strategy"], "rewind");
    assert_eq!(value["lineage"]["fork_origin_event_uid"], "ev-1");
}
