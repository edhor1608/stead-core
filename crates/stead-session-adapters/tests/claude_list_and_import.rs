use stead_session_adapters::claude::ClaudeAdapter;
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
