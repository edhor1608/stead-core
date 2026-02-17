use insta::assert_snapshot;
use stead_session_adapters::codex::CodexAdapter;
use tempfile::TempDir;

#[test]
fn exported_codex_jsonl_snapshot_is_stable() {
    let temp = TempDir::new().unwrap();
    let fixture_root = format!(
        "{}/tests/fixtures/codex/sessions",
        env!("CARGO_MANIFEST_DIR")
    );
    let target_root = temp.path().join("sessions");
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

    let adapter = CodexAdapter::from_base_dir(temp.path());
    let session = adapter.import_session("s-new").expect("import");
    let out = temp.path().join("sessions/2026/02/19/rollout-2026-02-19T00-00-00-s-snapshot.jsonl");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    adapter.export_session(&session, &out).expect("export");

    let exported = std::fs::read_to_string(out).expect("read export");
    assert_snapshot!(exported);
}
