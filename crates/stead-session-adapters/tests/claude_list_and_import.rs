use stead_session_adapters::claude::ClaudeAdapter;
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
    let session = adapter.import_session("claude-main").expect("import session");

    assert_eq!(session.source.original_session_id, "claude-main");
    assert_eq!(session.metadata.project_root, "/path/to/repo");
    assert_eq!(session.events.len(), 7);
    assert!(session.events.iter().any(|e| e.stream_id == "main"));
    assert!(session
        .events
        .iter()
        .any(|e| e.stream_id.starts_with("subagent:agent-a123")));
}
