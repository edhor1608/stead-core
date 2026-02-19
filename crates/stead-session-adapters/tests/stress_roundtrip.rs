use serde_json::json;
use stead_session_adapters::claude::ClaudeAdapter;
use stead_session_adapters::codex::CodexAdapter;
use tempfile::TempDir;

#[test]
fn codex_import_export_import_handles_large_mixed_timeline() {
    let temp = TempDir::new().unwrap();
    let file = temp
        .path()
        .join("sessions/2026/02/19/rollout-2026-02-19T00-00-00-stress-codex.jsonl");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();

    let mut lines = Vec::new();
    lines.push(json!({
        "timestamp": "2026-02-19T00:00:00Z",
        "type": "session_meta",
        "payload": { "id": "stress-codex", "cwd": "/stress/repo", "model_provider": "test" }
    }));
    for i in 0..120u32 {
        let ts_user = format!("2026-02-19T00:{:02}:{:02}Z", (i / 60), (i % 60));
        let ts_assistant = format!("2026-02-19T01:{:02}:{:02}Z", (i / 60), (i % 60));
        lines.push(json!({
            "timestamp": ts_user,
            "type": "response_item",
            "payload": { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": format!("user-{i}") }] }
        }));
        lines.push(json!({
            "timestamp": ts_assistant,
            "type": "response_item",
            "payload": { "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": format!("assistant-{i}") }] }
        }));
        if i % 10 == 0 {
            lines.push(json!({
                "timestamp": format!("2026-02-19T02:{:02}:{:02}Z", (i / 60), (i % 60)),
                "type": "response_item",
                "payload": {
                    "type": "function_call",
                    "name": "Read",
                    "call_id": format!("call-{i}"),
                    "arguments": "{\"file_path\":\"README.md\"}"
                }
            }));
            lines.push(json!({
                "timestamp": format!("2026-02-19T03:{:02}:{:02}Z", (i / 60), (i % 60)),
                "type": "response_item",
                "payload": {
                    "type": "function_call_output",
                    "call_id": format!("call-{i}"),
                    "output": "ok"
                }
            }));
        }
    }
    let raw = lines
        .into_iter()
        .map(|line| serde_json::to_string(&line).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&file, format!("{raw}\n")).unwrap();

    let adapter = CodexAdapter::from_base_dir(temp.path());
    let imported = adapter.import_session("stress-codex").unwrap();
    assert!(imported.events.len() > 240);
    assert_eq!(imported.events[0].sequence, Some(0));
    assert_eq!(
        imported.events.last().unwrap().sequence,
        Some(imported.events.len() as u64 - 1)
    );

    let out = temp
        .path()
        .join("sessions/2026/02/19/stress-codex-out.jsonl");
    adapter.export_session(&imported, &out).unwrap();
    let reimported = adapter.import_from_file(&out).unwrap();
    assert_eq!(imported.events.len(), reimported.events.len());
}

#[test]
fn claude_import_export_import_handles_large_mixed_timeline() {
    let temp = TempDir::new().unwrap();
    let file = temp
        .path()
        .join("projects/-stress-repo/stress-claude.jsonl");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();

    let mut lines = Vec::new();
    lines.push(json!({
        "type": "queue-operation",
        "operation": "enqueue",
        "timestamp": "2026-02-19T10:00:00Z",
        "sessionId": "stress-claude",
        "content": "start"
    }));
    for i in 0..100u32 {
        lines.push(json!({
            "type": "user",
            "timestamp": format!("2026-02-19T10:{:02}:{:02}Z", (i / 60), (i % 60)),
            "sessionId": "stress-claude",
            "cwd": "/stress/repo",
            "uuid": format!("u-{i}"),
            "isSidechain": false,
            "userType": "external",
            "message": { "role": "user", "content": format!("u-{i}") }
        }));
        lines.push(json!({
            "type": "assistant",
            "timestamp": format!("2026-02-19T11:{:02}:{:02}Z", (i / 60), (i % 60)),
            "sessionId": "stress-claude",
            "cwd": "/stress/repo",
            "uuid": format!("a-{i}"),
            "isSidechain": i % 7 == 0,
            "userType": if i % 7 == 0 { "agent" } else { "assistant" },
            "message": { "role": "assistant", "content": [{ "type": "text", "text": format!("a-{i}") }] }
        }));
        if i % 12 == 0 {
            lines.push(json!({
                "type": "progress",
                "timestamp": format!("2026-02-19T12:{:02}:{:02}Z", (i / 60), (i % 60)),
                "sessionId": "stress-claude",
                "cwd": "/stress/repo",
                "uuid": format!("p-{i}"),
                "data": { "type": "hook", "phase": "loop" }
            }));
        }
    }
    let raw = lines
        .into_iter()
        .map(|line| serde_json::to_string(&line).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&file, format!("{raw}\n")).unwrap();

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let imported = adapter.import_session("stress-claude").unwrap();
    assert!(imported.events.len() > 200);
    assert_eq!(imported.events[0].sequence, Some(0));
    assert_eq!(
        imported.events.last().unwrap().sequence,
        Some(imported.events.len() as u64 - 1)
    );

    let out = temp
        .path()
        .join("projects/-stress-repo/stress-claude-out.jsonl");
    adapter.export_session(&imported, &out).unwrap();
    let reimported = adapter.import_from_file(&out, "main").unwrap();
    assert_eq!(imported.events.len(), reimported.events.len());
}
