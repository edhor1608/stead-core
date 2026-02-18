use insta::assert_snapshot;
use stead_session_adapters::claude::ClaudeAdapter;
use tempfile::TempDir;

#[test]
fn exported_claude_jsonl_snapshot_is_stable() {
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

    let adapter = ClaudeAdapter::from_base_dir(temp.path());
    let session = adapter.import_session("claude-main").expect("import");
    let out = temp
        .path()
        .join("projects/-Users-jonas-repos-stead-core/claude-main-export.jsonl");
    adapter.export_session(&session, &out).expect("export");

    let exported = std::fs::read_to_string(out).expect("read export");
    assert_snapshot!(exported);
}
