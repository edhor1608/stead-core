use stead_session_adapters::claude::ClaudeAdapter;
use stead_session_adapters::AdapterError;
use stead_session_model::EventKind;
use tempfile::TempDir;

fn copy_fixture_tree(temp: &TempDir) {
    let fixture_root = format!(
        "{}/tests/fixtures/claude/projects",
        env!("CARGO_MANIFEST_DIR")
    );
    let target_root = temp.path().join("projects");
    std::fs::create_dir_all(&target_root).unwrap();

    for entry in walkdir::WalkDir::new(&fixture_root) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let rel = path.strip_prefix(&fixture_root).unwrap();
        let target_path = target_root.join(rel);
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::copy(path, target_path).unwrap();
    }
}

#[test]
fn list_sessions_discovers_main_claude_sessions() {
    let temp = TempDir::new().unwrap();
    copy_fixture_tree(&temp);

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let sessions = adapter.list_sessions().expect("list claude sessions");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].native_id, "claude-main");
}

#[test]
fn import_session_merges_main_and_subagent_streams() {
    let temp = TempDir::new().unwrap();
    copy_fixture_tree(&temp);

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let session = adapter.import_session("claude-main").expect("import session");

    assert_eq!(session.source.original_session_id, "claude-main");
    assert_eq!(session.metadata.project_root, "/Users/jonas/repos/stead-core");
    assert_eq!(session.events.len(), 7);
    assert!(session.events.iter().any(|e| e.stream_id == "main"));
    assert!(session
        .events
        .iter()
        .any(|e| e.stream_id.starts_with("subagent:agent-a123")));
}

#[test]
fn list_sessions_accepts_projects_directory_as_base_dir() {
    let temp = TempDir::new().unwrap();
    copy_fixture_tree(&temp);

    let adapter = ClaudeAdapter::from_base_dir(temp.path().join("projects"));
    let sessions = adapter.list_sessions().expect("list claude sessions");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].native_id, "claude-main");
}

#[test]
fn list_sessions_skips_malformed_files_instead_of_failing() {
    let temp = TempDir::new().unwrap();
    copy_fixture_tree(&temp);

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
    copy_fixture_tree(&temp);

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
    let session = adapter.import_session("raw-content").expect("import session");

    assert!(session.events.iter().any(|e| e.kind == EventKind::ToolCall));
    assert!(session.events.iter().any(|e| e.kind == EventKind::ToolResult));
}
