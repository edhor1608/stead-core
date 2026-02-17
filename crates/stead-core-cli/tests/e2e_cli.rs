use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[allow(deprecated)]
fn stead_core() -> Command {
    Command::cargo_bin("stead-core").unwrap()
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let rel = path.strip_prefix(from).unwrap();
        let target = to.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::copy(path, target).unwrap();
    }
}

#[test]
fn list_sessions_from_codex_backend_as_json() {
    let temp = TempDir::new().unwrap();
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    copy_tree(&fixture_root, temp.path());

    stead_core()
        .args([
            "sessions",
            "list",
            "--backend",
            "codex",
            "--base-dir",
            temp.path().to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"native_id\":\"s-new\""));
}

#[test]
fn import_codex_session_writes_canonical_json() {
    let temp = TempDir::new().unwrap();
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    copy_tree(&fixture_root, temp.path());

    let out = temp.path().join("canonical.json");
    stead_core()
        .args([
            "import",
            "--from",
            "codex",
            "--base-dir",
            temp.path().to_str().unwrap(),
            "--session",
            "s-new",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    let canonical = std::fs::read_to_string(out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    assert_eq!(parsed["schema_version"], "0.1.0");
    assert_eq!(parsed["source"]["backend"], "codex");
}

#[test]
fn convert_codex_to_claude_is_e2e_runnable() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    copy_tree(&fixture_root, source.path());

    let out = target
        .path()
        .join("projects/-Users-jonas-repos-stead-core/converted.jsonl");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    stead_core()
        .args([
            "convert",
            "--from",
            "codex",
            "--to",
            "claude",
            "--source-base",
            source.path().to_str().unwrap(),
            "--target-base",
            target.path().to_str().unwrap(),
            "--session",
            "s-new",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(out.exists());
    let exported = std::fs::read_to_string(out).unwrap();
    assert!(exported.contains("\"type\":\"assistant\""));
}

#[test]
fn convert_claude_to_codex_is_e2e_runnable() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&fixture_root, source.path());

    let out = target
        .path()
        .join("sessions/2026/02/20/rollout-2026-02-20T00-00-00-claude-main.jsonl");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    stead_core()
        .args([
            "convert",
            "--from",
            "claude",
            "--to",
            "codex",
            "--source-base",
            source.path().to_str().unwrap(),
            "--target-base",
            target.path().to_str().unwrap(),
            "--session",
            "claude-main",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(out.exists());
    let exported = std::fs::read_to_string(out).unwrap();
    assert!(exported.contains("\"type\":\"session_meta\""));
}

#[test]
fn export_canonical_to_codex_is_e2e_runnable() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    copy_tree(&fixture_root, source.path());

    let canonical = source.path().join("canonical-for-export.json");
    stead_core()
        .args([
            "import",
            "--from",
            "codex",
            "--base-dir",
            source.path().to_str().unwrap(),
            "--session",
            "s-new",
            "--out",
            canonical.to_str().unwrap(),
        ])
        .assert()
        .success();

    let out = target
        .path()
        .join("sessions/2026/02/21/rollout-2026-02-21T00-00-00-export.jsonl");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    stead_core()
        .args([
            "export",
            "--to",
            "codex",
            "--base-dir",
            target.path().to_str().unwrap(),
            "--input",
            canonical.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(out.exists());
    let exported = std::fs::read_to_string(out).unwrap();
    assert!(exported.contains("\"type\":\"response_item\""));
}

#[test]
fn export_canonical_to_claude_is_e2e_runnable() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&fixture_root, source.path());

    let canonical = source.path().join("canonical-for-export.json");
    stead_core()
        .args([
            "import",
            "--from",
            "claude",
            "--base-dir",
            source.path().to_str().unwrap(),
            "--session",
            "claude-main",
            "--out",
            canonical.to_str().unwrap(),
        ])
        .assert()
        .success();

    let out = target
        .path()
        .join("projects/-Users-jonas-repos-stead-core/claude-export.jsonl");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    stead_core()
        .args([
            "export",
            "--to",
            "claude",
            "--base-dir",
            target.path().to_str().unwrap(),
            "--input",
            canonical.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(out.exists());
    let exported = std::fs::read_to_string(out).unwrap();
    assert!(exported.contains("\"type\":\"assistant\""));
}
