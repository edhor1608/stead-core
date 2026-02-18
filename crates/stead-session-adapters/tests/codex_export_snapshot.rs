use insta::assert_snapshot;
use stead_session_adapters::codex::CodexAdapter;
use tempfile::TempDir;

mod support;

#[test]
fn exported_codex_jsonl_snapshot_is_stable() {
    let temp = TempDir::new().unwrap();
    support::copy_codex_fixture_tree(&temp);

    let adapter = CodexAdapter::from_base_dir(temp.path());
    let session = adapter.import_session("s-new").expect("import");
    let out = temp
        .path()
        .join("sessions/2026/02/19/rollout-2026-02-19T00-00-00-s-snapshot.jsonl");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    adapter.export_session(&session, &out).expect("export");

    let exported = std::fs::read_to_string(out).expect("read export");
    assert_snapshot!(exported);
}
