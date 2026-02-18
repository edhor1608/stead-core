use stead_session_adapters::codex::CodexAdapter;
use tempfile::TempDir;

mod support;

#[test]
fn list_sessions_discovers_and_sorts_by_recency() {
    let temp = TempDir::new().unwrap();
    support::copy_codex_fixture_tree(&temp);

    let adapter = CodexAdapter::from_base_dir(temp.path());
    let sessions = adapter.list_sessions().expect("list codex sessions");

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].native_id, "s-new");
    assert_eq!(sessions[1].native_id, "s-old");
}

#[test]
fn import_session_maps_messages_and_tools() {
    let temp = TempDir::new().unwrap();
    support::copy_codex_fixture_tree(&temp);

    let adapter = CodexAdapter::from_base_dir(temp.path());
    let session = adapter.import_session("s-new").expect("import session");

    assert_eq!(session.source.original_session_id, "s-new");
    assert_eq!(session.metadata.project_root, "/path/to/repo");
    assert_eq!(session.events.len(), 5);
    assert_eq!(session.events[0].sequence, Some(0));
    assert_eq!(session.events[0].stream_id, "main");
}
