use stead_session_adapters::codex::CodexAdapter;
use tempfile::TempDir;

mod support;

fn setup_codex_home() -> TempDir {
    let temp = TempDir::new().unwrap();
    support::copy_codex_fixture_tree(&temp);
    temp
}

#[test]
fn codex_import_export_import_roundtrip_preserves_core_semantics() {
    let temp = setup_codex_home();
    let adapter = CodexAdapter::from_base_dir(temp.path());
    let session = adapter.import_session("s-new").expect("first import");

    let out = temp
        .path()
        .join("sessions/2026/02/18/rollout-2026-02-18T00-00-00-s-new-roundtrip.jsonl");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let report = adapter
        .export_session(&session, &out)
        .expect("export codex session");

    assert_eq!(report.events_exported, session.events.len());
    assert!(report.losses.is_empty());

    let imported_again = adapter
        .import_from_file(&out)
        .expect("import exported codex session");

    assert_eq!(
        imported_again.source.original_session_id,
        session.source.original_session_id
    );
    assert_eq!(imported_again.events.len(), session.events.len());
}
