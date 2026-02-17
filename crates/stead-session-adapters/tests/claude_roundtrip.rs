use stead_session_adapters::claude::ClaudeAdapter;
use tempfile::TempDir;

fn setup_claude_home() -> TempDir {
    let temp = TempDir::new().unwrap();
    let fixture_root = format!(
        "{}/tests/fixtures/claude/projects",
        env!("CARGO_MANIFEST_DIR")
    );
    let target_root = temp.path().join("projects");
    std::fs::create_dir_all(&target_root).unwrap();

    for entry in walkdir::WalkDir::new(&fixture_root) {
        let entry = entry.unwrap();
        if entry.path().is_dir() {
            continue;
        }
        let rel = entry.path().strip_prefix(&fixture_root).unwrap();
        let target = target_root.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::copy(entry.path(), target).unwrap();
    }
    temp
}

#[test]
fn claude_import_export_import_roundtrip_preserves_core_semantics() {
    let temp = setup_claude_home();
    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let session = adapter.import_session("claude-main").expect("first import");

    let out = temp
        .path()
        .join("projects/-Users-jonas-repos-stead-core/claude-main-roundtrip.jsonl");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let report = adapter
        .export_session(&session, &out)
        .expect("export claude session");

    assert_eq!(report.events_exported, session.events.len());
    assert!(report.losses.is_empty());

    let imported_again = adapter
        .import_from_file(&out, "main")
        .expect("import exported claude session");
    assert_eq!(imported_again.events.len(), session.events.len());
    assert_eq!(
        imported_again.source.original_session_id,
        session.source.original_session_id
    );
}
