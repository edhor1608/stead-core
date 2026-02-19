use stead_session_adapters::AdapterError;
use stead_session_adapters::claude::ClaudeAdapter;
use stead_session_model::EventKind;
use tempfile::TempDir;

mod common;

#[test]
fn list_sessions_discovers_main_claude_sessions() {
    let temp = TempDir::new().unwrap();
    common::copy_claude_fixture_tree(&temp);

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let sessions = adapter.list_sessions().expect("list claude sessions");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].native_id, "claude-main");
}

#[test]
fn import_session_merges_main_and_subagent_streams() {
    let temp = TempDir::new().unwrap();
    common::copy_claude_fixture_tree(&temp);

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let session = adapter
        .import_session("claude-main")
        .expect("import session");

    assert_eq!(session.source.original_session_id, "claude-main");
    assert_eq!(session.metadata.project_root, "/path/to/repo");
    assert_eq!(session.events.len(), 7);
    assert!(session.events.iter().any(|e| e.stream_id == "main"));
    assert!(
        session
            .events
            .iter()
            .any(|e| e.stream_id.starts_with("subagent:agent-a123"))
    );
}

#[test]
fn list_sessions_accepts_projects_directory_as_base_dir() {
    let temp = TempDir::new().unwrap();
    common::copy_claude_fixture_tree(&temp);

    let adapter = ClaudeAdapter::from_base_dir(temp.path().join("projects"));
    let sessions = adapter.list_sessions().expect("list claude sessions");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].native_id, "claude-main");
}

#[test]
fn list_sessions_skips_malformed_files_instead_of_failing() {
    let temp = TempDir::new().unwrap();
    common::copy_claude_fixture_tree(&temp);

    std::fs::write(
        temp.path()
            .join("projects")
            .join("-Users-jonas-repos-stead-core")
            .join("broken.jsonl"),
        "{not-json",
    )
    .unwrap();

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let sessions = adapter
        .list_sessions()
        .expect("listing should skip malformed session files");

    assert!(!sessions.is_empty());
    assert!(sessions.iter().any(|s| s.native_id == "claude-main"));
}

#[test]
fn import_session_returns_not_found_even_if_other_files_are_malformed() {
    let temp = TempDir::new().unwrap();
    common::copy_claude_fixture_tree(&temp);

    std::fs::write(
        temp.path()
            .join("projects")
            .join("-Users-jonas-repos-stead-core")
            .join("broken.jsonl"),
        "{not-json",
    )
    .unwrap();

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let err = adapter
        .import_session("missing-session")
        .expect_err("should return not found");
    assert!(matches!(err, AdapterError::SessionNotFound(_)));
}

#[test]
fn import_session_parses_non_string_tool_result_content() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path().join("projects").join("-Users-test-repo");
    std::fs::create_dir_all(&project_dir).unwrap();
    let file = project_dir.join("raw-content.jsonl");
    std::fs::write(
        &file,
        concat!(
            "{\"type\":\"user\",\"timestamp\":\"2026-02-17T00:00:00Z\",\"sessionId\":\"raw-content\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"u1\",\"message\":{\"role\":\"user\",\"content\":\"start\"}}\n",
            "{\"type\":\"assistant\",\"timestamp\":\"2026-02-17T00:00:01Z\",\"sessionId\":\"raw-content\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"a1\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"tool_use\",\"id\":\"tool-1\",\"name\":\"Bash\",\"input\":{\"command\":\"echo ok\"}}]}}\n",
            "{\"type\":\"user\",\"timestamp\":\"2026-02-17T00:00:02Z\",\"sessionId\":\"raw-content\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"u2\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"tool-1\",\"content\":[{\"type\":\"text\",\"text\":\"ok\"}],\"is_error\":false}]}}\n"
        ),
    )
    .unwrap();

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let session = adapter
        .import_session("raw-content")
        .expect("import session");

    assert!(session.events.iter().any(|e| e.kind == EventKind::ToolCall));
    assert!(
        session
            .events
            .iter()
            .any(|e| e.kind == EventKind::ToolResult)
    );
}

#[test]
fn list_sessions_dedupes_split_files_by_latest_update() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path().join("projects").join("-Users-test-repo");
    std::fs::create_dir_all(&project_dir).unwrap();

    std::fs::write(
        project_dir.join("split-a.jsonl"),
        concat!(
            "{\"type\":\"user\",\"timestamp\":\"2026-02-17T00:00:00Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"u1\",\"message\":{\"role\":\"user\",\"content\":\"start\"}}\n",
            "{\"type\":\"assistant\",\"timestamp\":\"2026-02-17T00:00:01Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"a1\",\"parentUuid\":\"u1\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"step one\"}]}}\n"
        ),
    )
    .unwrap();

    std::fs::write(
        project_dir.join("split-b.jsonl"),
        concat!(
            "{\"type\":\"user\",\"timestamp\":\"2026-02-17T00:00:00Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"u1\",\"message\":{\"role\":\"user\",\"content\":\"start\"}}\n",
            "{\"type\":\"assistant\",\"timestamp\":\"2026-02-17T00:00:01Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"a1\",\"parentUuid\":\"u1\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"step one\"}]}}\n",
            "{\"type\":\"user\",\"timestamp\":\"2026-02-17T00:00:02Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"u2\",\"parentUuid\":\"a1\",\"message\":{\"role\":\"user\",\"content\":\"rewind branch\"}}\n"
        ),
    )
    .unwrap();

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let sessions = adapter.list_sessions().expect("list split sessions");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].native_id, "claude-rewind");
    assert!(sessions[0].file_path.ends_with("split-b.jsonl"));
}

#[test]
fn import_session_merges_split_files_and_preserves_lineage_payload() {
    let temp = TempDir::new().unwrap();
    let project_dir = temp.path().join("projects").join("-Users-test-repo");
    std::fs::create_dir_all(&project_dir).unwrap();

    std::fs::write(
        project_dir.join("split-a.jsonl"),
        concat!(
            "{\"type\":\"user\",\"timestamp\":\"2026-02-17T00:00:00Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"u1\",\"message\":{\"role\":\"user\",\"content\":\"start\"}}\n",
            "{\"type\":\"assistant\",\"timestamp\":\"2026-02-17T00:00:01Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"a1\",\"parentUuid\":\"u1\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"step one\"}]}}\n"
        ),
    )
    .unwrap();

    std::fs::write(
        project_dir.join("split-b.jsonl"),
        concat!(
            "{\"type\":\"user\",\"timestamp\":\"2026-02-17T00:00:00Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"u1\",\"message\":{\"role\":\"user\",\"content\":\"start\"}}\n",
            "{\"type\":\"assistant\",\"timestamp\":\"2026-02-17T00:00:01Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"a1\",\"parentUuid\":\"u1\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"step one\"}]}}\n",
            "{\"type\":\"user\",\"timestamp\":\"2026-02-17T00:00:02Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"u2\",\"parentUuid\":\"a1\",\"message\":{\"role\":\"user\",\"content\":\"rewind branch\"}}\n",
            "{\"type\":\"assistant\",\"timestamp\":\"2026-02-17T00:00:03Z\",\"sessionId\":\"claude-rewind\",\"cwd\":\"/Users/test/repo\",\"uuid\":\"a2\",\"parentUuid\":\"u2\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"branch response\"}]}}\n"
        ),
    )
    .unwrap();

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let session = adapter
        .import_session("claude-rewind")
        .expect("import split session");

    assert_eq!(session.source.original_session_id, "claude-rewind");
    assert_eq!(session.events.len(), 4);
    assert_eq!(session.source.source_files.len(), 2);
    assert!(
        session
            .source
            .source_files
            .iter()
            .any(|path| path.ends_with("split-a.jsonl"))
    );
    assert!(
        session
            .source
            .source_files
            .iter()
            .any(|path| path.ends_with("split-b.jsonl"))
    );

    assert!(session.events.iter().all(|event| {
        event
            .extensions
            .get("source_file")
            .and_then(|value| value.as_str())
            .is_some()
    }));

    assert!(session.events.iter().any(|event| {
        event
            .raw_vendor_payload
            .get("parentUuid")
            .and_then(|value| value.as_str())
            == Some("u2")
    }));
}

#[test]
fn import_session_handles_queue_and_sidechain_variants() {
    let temp = TempDir::new().unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("compat")
        .join("claude")
        .join("queue-sidechain-mixed.jsonl");
    let path = temp
        .path()
        .join("projects")
        .join("-Users-test-repo")
        .join("queue-sidechain-mixed.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::copy(fixture, &path).unwrap();

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let listed = adapter.list_sessions().expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].native_id, "claude-queue-sidechain");
    assert_eq!(listed[0].title.as_deref(), Some("start from queue"));

    let session = adapter
        .import_session("claude-queue-sidechain")
        .expect("import");
    assert_eq!(session.source.original_session_id, "claude-queue-sidechain");
    assert_eq!(session.events.len(), 5);
    assert!(
        session
            .events
            .iter()
            .any(|event| event.kind == EventKind::ToolCall)
    );
    assert!(
        session
            .events
            .iter()
            .any(|event| event.kind == EventKind::ToolResult)
    );
    assert!(
        session
            .events
            .iter()
            .any(|event| event.kind == EventKind::SystemProgress)
    );
}
