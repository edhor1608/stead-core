use serde_json::{Value, json};
use std::path::Path;
use stead_session_adapters::claude::ClaudeAdapter;
use stead_session_adapters::codex::CodexAdapter;
use stead_session_model::{EventKind, EventPayload};
use tempfile::TempDir;

fn parse_jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect()
}

#[test]
fn codex_roundtrip_preserves_unknown_fields() {
    let temp = TempDir::new().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/compat/codex/unknown-fields.jsonl");
    let input = temp.path().join("codex-input.jsonl");
    std::fs::copy(fixture, &input).unwrap();

    let adapter = CodexAdapter::from_base_dir(temp.path());
    let session = adapter.import_from_file(&input).unwrap();
    let output = temp.path().join("codex-output.jsonl");
    adapter.export_session(&session, &output).unwrap();
    let lines = parse_jsonl(&output);

    assert_eq!(lines[0]["source_version"], json!("2026.2"));
    assert_eq!(lines[0]["payload"]["workspace_id"], json!("ws-123"));

    let message_line = lines
        .iter()
        .find(|line| {
            line.get("type") == Some(&json!("response_item"))
                && line.get("payload").and_then(|p| p.get("type")) == Some(&json!("message"))
        })
        .unwrap();
    assert_eq!(message_line["trace_id"], json!("trace-abc"));
    assert_eq!(
        message_line["payload"]["vendor_extras"]["flag"],
        json!(true)
    );
}

#[test]
fn codex_roundtrip_keeps_event_ids_timestamps_and_tool_links() {
    let temp = TempDir::new().unwrap();
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex");
    copy_tree(&fixture_root, temp.path());

    let adapter = CodexAdapter::from_base_dir(temp.path());
    let imported = adapter.import_session("s-new").unwrap();
    let output = temp
        .path()
        .join("sessions/2026/02/18/rollout-2026-02-18T00-00-00-s-new-compat.jsonl");
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    adapter.export_session(&imported, &output).unwrap();
    let reimported = adapter.import_from_file(&output).unwrap();

    assert_eq!(imported.events.len(), reimported.events.len());
    for (before, after) in imported.events.iter().zip(reimported.events.iter()) {
        assert_eq!(before.kind, after.kind);
        assert_eq!(before.event_uid, after.event_uid);
        assert_eq!(before.timestamp, after.timestamp);
    }

    let tool_calls: Vec<_> = reimported
        .events
        .iter()
        .filter(|event| event.kind == EventKind::ToolCall)
        .map(|event| event.event_uid.clone())
        .collect();
    for result in reimported
        .events
        .iter()
        .filter(|event| event.kind == EventKind::ToolResult)
    {
        let call_id = match &result.payload {
            EventPayload::ToolResult { call_id, .. } => call_id.clone(),
            _ => String::new(),
        };
        assert!(tool_calls.iter().any(|id| id == &call_id));
    }
}

#[test]
fn claude_roundtrip_preserves_unknown_fields() {
    let temp = TempDir::new().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/compat/claude/unknown-fields.jsonl");
    let input = temp.path().join("claude-input.jsonl");
    std::fs::copy(fixture, &input).unwrap();

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let session = adapter.import_from_file(&input, "main").unwrap();
    let output = temp.path().join("claude-output.jsonl");
    adapter.export_session(&session, &output).unwrap();
    let lines = parse_jsonl(&output);

    assert_eq!(lines[0]["trace"]["id"], json!("trace-1"));
    assert_eq!(lines[0]["message"]["extra"]["lang"], json!("en"));

    let tool_use_line = lines
        .iter()
        .find(|line| {
            line.get("message")
                .and_then(|m| m.get("content"))
                .and_then(|content| content.as_array())
                .and_then(|items| items.first())
                .and_then(|item| item.get("type"))
                == Some(&json!("tool_use"))
        })
        .unwrap();
    assert_eq!(tool_use_line["trace"]["id"], json!("trace-2"));
    assert_eq!(
        tool_use_line["message"]["content"][0]["latency_ms"],
        json!(12)
    );
}

#[test]
fn claude_roundtrip_keeps_event_ids_timestamps_and_tool_links() {
    let temp = TempDir::new().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/compat/claude/unknown-fields.jsonl");
    let input = temp.path().join("claude-input.jsonl");
    std::fs::copy(fixture, &input).unwrap();

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let imported = adapter.import_from_file(&input, "main").unwrap();
    let output = temp.path().join("claude-output.jsonl");
    adapter.export_session(&imported, &output).unwrap();
    let reimported = adapter.import_from_file(&output, "main").unwrap();

    assert_eq!(imported.events.len(), reimported.events.len());
    for (before, after) in imported.events.iter().zip(reimported.events.iter()) {
        assert_eq!(before.kind, after.kind);
        assert_eq!(before.timestamp, after.timestamp);
    }

    let tool_calls: Vec<_> = reimported
        .events
        .iter()
        .filter(|event| event.kind == EventKind::ToolCall)
        .map(|event| event.event_uid.clone())
        .collect();
    for result in reimported
        .events
        .iter()
        .filter(|event| event.kind == EventKind::ToolResult)
    {
        let call_id = match &result.payload {
            EventPayload::ToolResult { call_id, .. } => call_id.clone(),
            _ => String::new(),
        };
        assert!(tool_calls.iter().any(|id| id == &call_id));
    }

    let imported_progress_ids: Vec<_> = imported
        .events
        .iter()
        .filter(|event| event.kind == EventKind::SystemProgress)
        .map(|event| event.event_uid.clone())
        .collect();
    let reimported_progress_ids: Vec<_> = reimported
        .events
        .iter()
        .filter(|event| event.kind == EventKind::SystemProgress)
        .map(|event| event.event_uid.clone())
        .collect();
    assert_eq!(imported_progress_ids, reimported_progress_ids);
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let rel = path.strip_prefix(from).unwrap();
        let target = to.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::copy(path, target).unwrap();
    }
}
