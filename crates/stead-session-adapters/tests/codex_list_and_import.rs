use stead_session_adapters::AdapterError;
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

#[test]
fn list_sessions_accepts_sessions_directory_as_base_dir() {
    let temp = TempDir::new().unwrap();
    support::copy_codex_fixture_tree(&temp);

    let adapter = CodexAdapter::from_base_dir(temp.path().join("sessions"));
    let sessions = adapter.list_sessions().expect("list codex sessions");

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].native_id, "s-new");
}

#[test]
fn import_session_returns_not_found_even_if_other_files_are_malformed() {
    let temp = TempDir::new().unwrap();
    support::copy_codex_fixture_tree(&temp);
    std::fs::write(
        temp.path()
            .join("sessions")
            .join("2026")
            .join("02")
            .join("17")
            .join("bad.jsonl"),
        "{not-json",
    )
    .unwrap();

    let adapter = CodexAdapter::from_base_dir(temp.path());
    let err = adapter
        .import_session("missing-session")
        .expect_err("should return not found");
    assert!(matches!(err, AdapterError::SessionNotFound(_)));
}
