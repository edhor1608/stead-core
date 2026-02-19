use chrono::{TimeZone, Utc};
use insta::assert_json_snapshot;
use stead_session_model::{
    BackendKind, EventActor, EventKind, EventPayload, SessionMetadata, SessionSource, SteadEvent,
    SteadSession, build_session_uid, canonical_sort_events, schema_version,
};

#[test]
fn canonical_json_snapshot_is_stable() {
    let mut events = vec![
        SteadEvent {
            event_uid: "ev-tool".to_string(),
            stream_id: "main".to_string(),
            line_number: 2,
            sequence: None,
            timestamp: Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 2).unwrap(),
            kind: EventKind::ToolCall,
            actor: Some(EventActor::assistant("assistant")),
            payload: EventPayload::tool_call("exec_command", serde_json::json!({ "cmd": "ls" })),
            raw_vendor_payload: serde_json::json!({ "vendor": "codex", "type": "function_call" }),
            extensions: serde_json::Map::new(),
        },
        SteadEvent {
            event_uid: "ev-user".to_string(),
            stream_id: "main".to_string(),
            line_number: 1,
            sequence: None,
            timestamp: Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 1).unwrap(),
            kind: EventKind::MessageUser,
            actor: Some(EventActor::user("user")),
            payload: EventPayload::text("List files"),
            raw_vendor_payload: serde_json::json!({ "vendor": "codex", "type": "message" }),
            extensions: serde_json::Map::new(),
        },
    ];
    canonical_sort_events(&mut events);

    let mut source =
        SessionSource::new(BackendKind::Codex, "abc", vec!["/tmp/source.jsonl".into()]);
    source.imported_at = Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 0).unwrap();

    let session = SteadSession {
        schema_version: schema_version().to_string(),
        session_uid: build_session_uid(BackendKind::Codex, "abc"),
        source,
        metadata: SessionMetadata::new(
            Some("Sample Session".into()),
            "/Users/jonas/repos/stead".into(),
            Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 2, 17, 12, 0, 2).unwrap(),
        ),
        events,
        artifacts: vec![],
        capabilities: serde_json::Map::new(),
        extensions: serde_json::Map::new(),
        shared_session_uid: Some("stead:shared:abc".to_string()),
        lineage: None,
        raw_vendor_payload: serde_json::json!({ "session_meta": { "id": "abc" } }),
    };

    assert_json_snapshot!(session);
}
