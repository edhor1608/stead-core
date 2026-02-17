use stead_session_adapters::codex::CodexAdapter;
use tempfile::TempDir;

fn copy_fixture_tree(temp: &TempDir) {
    let fixture_root = format!(
        "{}/tests/fixtures/codex/sessions",
        env!("CARGO_MANIFEST_DIR")
    );
    let target_root = temp.path().join("sessions");
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
fn list_sessions_discovers_and_sorts_by_recency() {
    let temp = TempDir::new().unwrap();
    copy_fixture_tree(&temp);

    let adapter = CodexAdapter::from_base_dir(temp.path());
    let sessions = adapter.list_sessions().expect("list codex sessions");

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].native_id, "s-new");
    assert_eq!(sessions[1].native_id, "s-old");
}

#[test]
fn import_session_maps_messages_and_tools() {
    let temp = TempDir::new().unwrap();
    copy_fixture_tree(&temp);

    let adapter = CodexAdapter::from_base_dir(temp.path());
    let session = adapter.import_session("s-new").expect("import session");

    assert_eq!(session.source.original_session_id, "s-new");
    assert_eq!(session.metadata.project_root, "/Users/jonas/repos/stead-core");
    assert_eq!(session.events.len(), 5);
    assert_eq!(session.events[0].sequence, Some(0));
    assert_eq!(session.events[0].stream_id, "main");
}
