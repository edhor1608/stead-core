use assert_cmd::prelude::*;
use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use uuid::Uuid;

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

fn rewrite_in_file(path: &Path, from: &str, to: &str) {
    let raw = std::fs::read_to_string(path).unwrap();
    std::fs::write(path, raw.replace(from, to)).unwrap();
}

fn list_canonical_sessions(repo_root: &Path) -> Vec<serde_json::Value> {
    let sessions_dir = repo_root.join(".stead-core").join("sessions");
    if !sessions_dir.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(sessions_dir).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(path).unwrap();
        out.push(serde_json::from_str(&raw).unwrap());
    }
    out
}

fn parse_jsonl_lines(raw: &str) -> Vec<Value> {
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect()
}

fn assert_jsonl_has_type(lines: &[Value], expected_type: &str) {
    assert!(
        lines
            .iter()
            .any(|line| { line.get("type") == Some(&Value::String(expected_type.to_string())) })
    );
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
    let lines = parse_jsonl_lines(&exported);
    assert_jsonl_has_type(&lines, "assistant");
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
    let lines = parse_jsonl_lines(&exported);
    assert_jsonl_has_type(&lines, "session_meta");
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
    let lines = parse_jsonl_lines(&exported);
    assert_jsonl_has_type(&lines, "response_item");
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
    let lines = parse_jsonl_lines(&exported);
    assert_jsonl_has_type(&lines, "assistant");
}

#[test]
fn sync_imports_codex_and_claude_sessions_into_repo_store() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    assert!(!sessions.is_empty());
    assert!(sessions.iter().any(|s| s["source"]["backend"] == "codex"));
    assert!(
        sessions
            .iter()
            .any(|s| s["source"]["backend"] == "claude_code")
    );
}

#[test]
fn sync_accepts_leaf_backend_directories() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().join("sessions").to_str().unwrap(),
            "--claude-base",
            claude_home.path().join("projects").to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    assert!(!sessions.is_empty());
    assert!(sessions.iter().any(|s| s["source"]["backend"] == "codex"));
    assert!(
        sessions
            .iter()
            .any(|s| s["source"]["backend"] == "claude_code")
    );
}

#[test]
fn sync_scopes_to_repo_when_matching_sessions_exist() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    let repo_path = repo.path().to_str().unwrap();
    let codex_new = codex_home
        .path()
        .join("sessions/2026/02/17/rollout-2026-02-17T20-00-00-s-new.jsonl");
    let codex_old = codex_home
        .path()
        .join("sessions/2026/02/16/rollout-2026-02-16T20-00-00-s-old.jsonl");
    rewrite_in_file(&codex_new, "/path/to/repo", repo_path);
    rewrite_in_file(&codex_old, "/path/to/repo", "/tmp/other-repo");

    let claude_main = claude_home
        .path()
        .join("projects/-Users-jonas-repos-stead-core/claude-main.jsonl");
    let claude_sub = claude_home
        .path()
        .join("projects/-Users-jonas-repos-stead-core/subagents/agent-a123.jsonl");
    rewrite_in_file(&claude_main, "/path/to/repo", repo_path);
    rewrite_in_file(&claude_sub, "/path/to/repo", repo_path);

    stead_core()
        .args([
            "sync",
            "--repo",
            repo_path,
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    assert_eq!(sessions.len(), 2);
    assert!(
        sessions
            .iter()
            .any(|s| s["source"]["original_session_id"] == "s-new")
    );
    assert!(
        !sessions
            .iter()
            .any(|s| s["source"]["original_session_id"] == "s-old")
    );
    assert!(
        sessions
            .iter()
            .any(|s| s["source"]["original_session_id"] == "claude-main")
    );
}

#[test]
fn materialize_updates_canonical_native_refs_and_writes_target_session() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    let codex_session = sessions
        .iter()
        .find(|s| s["source"]["backend"] == "codex")
        .unwrap();
    let canonical_id = codex_session["session_uid"].as_str().unwrap();

    let out = claude_home
        .path()
        .join("projects/-Users-jonas-repos-stead-core/materialized.jsonl");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    stead_core()
        .args([
            "materialize",
            "--repo",
            repo.path().to_str().unwrap(),
            "--session",
            canonical_id,
            "--to",
            "claude",
            "--base-dir",
            claude_home.path().to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(out.exists());
    let refreshed = list_canonical_sessions(repo.path());
    let updated = refreshed
        .iter()
        .find(|s| s["session_uid"] == canonical_id)
        .unwrap();
    assert_eq!(
        updated["extensions"]["native_refs"]["claude"]["path"],
        out.to_str().unwrap()
    );
}

#[test]
#[cfg(unix)]
fn resume_uses_backend_resume_flag_with_prompt() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    let codex_session = sessions
        .iter()
        .find(|s| s["source"]["backend"] == "codex")
        .unwrap();
    let canonical_id = codex_session["session_uid"].as_str().unwrap();
    let expected_native_id = codex_session["extensions"]["native_refs"]["codex"]["session_id"]
        .as_str()
        .unwrap();

    let runner_log = repo.path().join("runner.log");
    let runner_script = repo.path().join("runner.sh");
    std::fs::write(
        &runner_script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{}\"\n",
            runner_log.display()
        ),
    )
    .unwrap();
    let mut perms = std::fs::metadata(&runner_script).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&runner_script, perms).unwrap();

    stead_core()
        .env("STEAD_CORE_RUNNER", runner_script.to_str().unwrap())
        .args([
            "resume",
            "--repo",
            repo.path().to_str().unwrap(),
            "--session",
            canonical_id,
            "--backend",
            "codex",
            "--prompt",
            "Continue with tests",
        ])
        .assert()
        .success();

    let logged = std::fs::read_to_string(runner_log).unwrap();
    assert!(logged.contains("codex"));
    assert!(logged.contains("--resume"));
    assert!(logged.contains(expected_native_id));
    assert!(logged.contains("Continue with tests"));
}

#[test]
#[cfg(unix)]
fn handoff_uses_existing_native_ref_and_resume_prompt() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    let codex_session = sessions
        .iter()
        .find(|s| s["source"]["backend"] == "codex")
        .unwrap();
    let canonical_id = codex_session["session_uid"].as_str().unwrap();
    let expected_native_id = codex_session["extensions"]["native_refs"]["codex"]["session_id"]
        .as_str()
        .unwrap();

    let runner_log = repo.path().join("runner-handoff.log");
    let runner_script = repo.path().join("runner-handoff.sh");
    std::fs::write(
        &runner_script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{}\"\n",
            runner_log.display()
        ),
    )
    .unwrap();
    let mut perms = std::fs::metadata(&runner_script).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&runner_script, perms).unwrap();

    stead_core()
        .env("STEAD_CORE_RUNNER", runner_script.to_str().unwrap())
        .args([
            "handoff",
            "--repo",
            repo.path().to_str().unwrap(),
            "--session",
            canonical_id,
            "--to",
            "codex",
            "--resume",
            "Continue handoff",
        ])
        .assert()
        .success();

    let logged = std::fs::read_to_string(runner_log).unwrap();
    assert!(logged.contains("codex"));
    assert!(logged.contains("--resume"));
    assert!(logged.contains(expected_native_id));
    assert!(logged.contains("Continue handoff"));
}

#[test]
#[cfg(unix)]
fn handoff_materializes_target_backend_before_resume_when_needed() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    let codex_session = sessions
        .iter()
        .find(|s| s["source"]["backend"] == "codex")
        .unwrap();
    let canonical_id = codex_session["session_uid"].as_str().unwrap();

    let materialized_out = claude_home
        .path()
        .join("projects/-Users-jonas-repos-stead-core/handoff-materialized.jsonl");
    if let Some(parent) = materialized_out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    let runner_log = repo.path().join("runner-handoff-materialize.log");
    let runner_script = repo.path().join("runner-handoff-materialize.sh");
    std::fs::write(
        &runner_script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{}\"\n",
            runner_log.display()
        ),
    )
    .unwrap();
    let mut perms = std::fs::metadata(&runner_script).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&runner_script, perms).unwrap();

    stead_core()
        .env("STEAD_CORE_RUNNER", runner_script.to_str().unwrap())
        .args([
            "handoff",
            "--repo",
            repo.path().to_str().unwrap(),
            "--session",
            canonical_id,
            "--to",
            "claude",
            "--base-dir",
            claude_home.path().to_str().unwrap(),
            "--out",
            materialized_out.to_str().unwrap(),
            "--resume",
            "Continue in claude",
        ])
        .assert()
        .success();

    assert!(materialized_out.exists());
    let refreshed = list_canonical_sessions(repo.path());
    let updated = refreshed
        .iter()
        .find(|s| s["session_uid"] == canonical_id)
        .unwrap();
    let native_id = updated["extensions"]["native_refs"]["claude"]["session_id"]
        .as_str()
        .unwrap();
    let native_path = updated["extensions"]["native_refs"]["claude"]["path"]
        .as_str()
        .unwrap();
    assert_eq!(native_path, materialized_out.to_str().unwrap());

    let logged = std::fs::read_to_string(runner_log).unwrap();
    assert!(logged.contains("claude"));
    assert!(logged.contains("--resume"));
    assert!(logged.contains(native_id));
    assert!(logged.contains("Continue in claude"));
}

#[test]
fn sync_backfills_shared_session_uid() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    assert!(!sessions.is_empty());
    for session in sessions {
        assert_eq!(session["shared_session_uid"], session["session_uid"]);
    }
}

#[test]
fn materialize_uses_uuid_native_ids_for_cross_backend_targets() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    let codex_session = sessions
        .iter()
        .find(|s| s["source"]["backend"] == "codex")
        .unwrap();
    let claude_session = sessions
        .iter()
        .find(|s| s["source"]["backend"] == "claude_code")
        .unwrap();

    let codex_uid = codex_session["session_uid"].as_str().unwrap();
    let claude_uid = claude_session["session_uid"].as_str().unwrap();

    let codex_to_claude = stead_core()
        .args([
            "materialize",
            "--repo",
            repo.path().to_str().unwrap(),
            "--session",
            codex_uid,
            "--to",
            "claude",
            "--base-dir",
            claude_home.path().to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let codex_to_claude: Value = serde_json::from_slice(&codex_to_claude).unwrap();
    let claude_native_id = codex_to_claude["native_id"].as_str().unwrap();
    let claude_out = codex_to_claude["output_path"].as_str().unwrap();
    assert!(Uuid::parse_str(claude_native_id).is_ok());
    assert!(claude_out.ends_with(&format!("{claude_native_id}.jsonl")));

    let claude_to_codex = stead_core()
        .args([
            "materialize",
            "--repo",
            repo.path().to_str().unwrap(),
            "--session",
            claude_uid,
            "--to",
            "codex",
            "--base-dir",
            codex_home.path().to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let claude_to_codex: Value = serde_json::from_slice(&claude_to_codex).unwrap();
    let codex_native_id = claude_to_codex["native_id"].as_str().unwrap();
    let codex_out = claude_to_codex["output_path"].as_str().unwrap();
    assert!(Uuid::parse_str(codex_native_id).is_ok());
    assert!(codex_out.contains("/sessions/"));
    assert!(codex_out.contains("/rollout-"));
    assert!(codex_out.ends_with(&format!("-{codex_native_id}.jsonl")));
}

#[test]
fn materialize_to_codex_prunes_stale_same_id_rollouts() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    let claude_session = sessions
        .iter()
        .find(|s| s["source"]["backend"] == "claude_code")
        .unwrap();
    let canonical_id = claude_session["session_uid"].as_str().unwrap();
    let shared_uid = claude_session["shared_session_uid"].as_str().unwrap();
    let predicted_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("stead-native:codex:{shared_uid}").as_bytes(),
    )
    .to_string();

    let stale = codex_home.path().join(format!(
        "sessions/2026/02/17/rollout-stale-{predicted_id}.jsonl"
    ));
    if let Some(parent) = stale.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(
        &stale,
        format!(
            "{{\"type\":\"session_meta\",\"timestamp\":\"2026-02-17T00:00:00Z\",\"payload\":{{\"id\":\"{predicted_id}\",\"cwd\":\"{}\",\"model_provider\":\"unknown\"}}}}\n",
            repo.path().display()
        ),
    )
    .unwrap();

    let output = stead_core()
        .args([
            "materialize",
            "--repo",
            repo.path().to_str().unwrap(),
            "--session",
            canonical_id,
            "--to",
            "codex",
            "--base-dir",
            codex_home.path().to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let materialized: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(materialized["native_id"].as_str().unwrap(), predicted_id);

    let mut matches = Vec::new();
    for entry in walkdir::WalkDir::new(codex_home.path())
        .into_iter()
        .flatten()
    {
        if !entry.path().is_file() {
            continue;
        }
        let Some(name) = entry.path().file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if name.ends_with(&format!("-{predicted_id}.jsonl")) {
            matches.push(entry.path().to_path_buf());
        }
    }
    assert_eq!(
        matches.len(),
        1,
        "expected stale rollout files to be pruned"
    );
    let first_line = std::fs::read_to_string(&matches[0])
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_string();
    assert!(first_line.contains("\"originator\""));
}

#[test]
fn sync_upserts_cross_backend_materialized_sessions_instead_of_creating_duplicates() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    let codex_session = sessions
        .iter()
        .find(|s| s["source"]["backend"] == "codex")
        .unwrap();
    let canonical_id = codex_session["session_uid"].as_str().unwrap();

    stead_core()
        .args([
            "materialize",
            "--repo",
            repo.path().to_str().unwrap(),
            "--session",
            canonical_id,
            "--to",
            "claude",
            "--base-dir",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let before = list_canonical_sessions(repo.path());
    let before_count = before.len();

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let refreshed = list_canonical_sessions(repo.path());
    assert_eq!(refreshed.len(), before_count);

    let updated = refreshed
        .iter()
        .find(|s| s["session_uid"] == canonical_id)
        .unwrap();
    assert!(updated["extensions"]["native_refs"]["codex"]["session_id"].is_string());
    assert!(updated["extensions"]["native_refs"]["claude"]["session_id"].is_string());
}

#[test]
#[cfg(unix)]
fn resume_uses_real_backend_command_shape_and_repo_cwd() {
    let repo = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let claude_home = TempDir::new().unwrap();

    let codex_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/codex");
    let claude_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stead-session-adapters/tests/fixtures/claude");
    copy_tree(&codex_fixture, codex_home.path());
    copy_tree(&claude_fixture, claude_home.path());

    stead_core()
        .args([
            "sync",
            "--repo",
            repo.path().to_str().unwrap(),
            "--codex-base",
            codex_home.path().to_str().unwrap(),
            "--claude-base",
            claude_home.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let sessions = list_canonical_sessions(repo.path());
    let codex_session = sessions
        .iter()
        .find(|s| s["source"]["backend"] == "codex")
        .unwrap();
    let claude_session = sessions
        .iter()
        .find(|s| s["source"]["backend"] == "claude_code")
        .unwrap();
    let codex_uid = codex_session["session_uid"].as_str().unwrap();
    let claude_uid = claude_session["session_uid"].as_str().unwrap();

    let codex_log = repo.path().join("fake-codex.log");
    let codex_bin = repo.path().join("fake-codex.sh");
    std::fs::write(
        &codex_bin,
        format!(
            "#!/bin/sh\npwd > \"{}\"\nprintf '%s\\n' \"$@\" >> \"{}\"\n",
            codex_log.display(),
            codex_log.display()
        ),
    )
    .unwrap();
    let mut perms = std::fs::metadata(&codex_bin).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&codex_bin, perms).unwrap();

    let claude_log = repo.path().join("fake-claude.log");
    let claude_bin = repo.path().join("fake-claude.sh");
    std::fs::write(
        &claude_bin,
        format!(
            "#!/bin/sh\npwd > \"{}\"\nprintf '%s\\n' \"$@\" >> \"{}\"\n",
            claude_log.display(),
            claude_log.display()
        ),
    )
    .unwrap();
    let mut perms = std::fs::metadata(&claude_bin).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&claude_bin, perms).unwrap();

    stead_core()
        .env("STEAD_CORE_CODEX_BIN", codex_bin.to_str().unwrap())
        .args([
            "resume",
            "--repo",
            repo.path().to_str().unwrap(),
            "--session",
            codex_uid,
            "--backend",
            "codex",
            "--prompt",
            "Continue in codex",
        ])
        .assert()
        .success();

    let codex_logged = std::fs::read_to_string(&codex_log).unwrap();
    assert!(codex_logged.contains(repo.path().to_str().unwrap()));
    assert!(codex_logged.contains("\nexec\n"));
    assert!(codex_logged.contains("\nresume\n"));
    assert!(codex_logged.contains("Continue in codex"));

    stead_core()
        .env("STEAD_CORE_CLAUDE_BIN", claude_bin.to_str().unwrap())
        .args([
            "resume",
            "--repo",
            repo.path().to_str().unwrap(),
            "--session",
            claude_uid,
            "--backend",
            "claude",
            "--prompt",
            "Continue in claude",
        ])
        .assert()
        .success();

    let claude_logged = std::fs::read_to_string(&claude_log).unwrap();
    assert!(claude_logged.contains(repo.path().to_str().unwrap()));
    assert!(claude_logged.contains("\n-p\n"));
    assert!(claude_logged.contains("\n-r\n"));
    assert!(claude_logged.contains("Continue in claude"));
}
