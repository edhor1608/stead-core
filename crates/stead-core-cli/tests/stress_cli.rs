use assert_cmd::prelude::*;
use serde_json::Value;
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

fn list_canonical_sessions(repo_root: &Path) -> Vec<Value> {
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

#[test]
#[cfg(unix)]
fn repeated_crossover_cycles_keep_canonical_store_stable() {
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

    let baseline = list_canonical_sessions(repo.path());
    let baseline_count = baseline.len();
    assert!(baseline_count >= 2);

    let codex_canonical = baseline
        .iter()
        .find(|session| session["source"]["backend"] == "codex")
        .unwrap()["session_uid"]
        .as_str()
        .unwrap()
        .to_string();
    let claude_canonical = baseline
        .iter()
        .find(|session| session["source"]["backend"] == "claude_code")
        .unwrap()["session_uid"]
        .as_str()
        .unwrap()
        .to_string();

    let runner_script = repo.path().join("runner.sh");
    std::fs::write(&runner_script, "#!/bin/sh\nexit 0\n").unwrap();
    let mut perms = std::fs::metadata(&runner_script).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&runner_script, perms).unwrap();

    for round in 0..2 {
        stead_core()
            .args([
                "materialize",
                "--repo",
                repo.path().to_str().unwrap(),
                "--session",
                &codex_canonical,
                "--to",
                "claude",
                "--base-dir",
                claude_home.path().to_str().unwrap(),
            ])
            .assert()
            .success();

        stead_core()
            .args([
                "materialize",
                "--repo",
                repo.path().to_str().unwrap(),
                "--session",
                &claude_canonical,
                "--to",
                "codex",
                "--base-dir",
                codex_home.path().to_str().unwrap(),
            ])
            .assert()
            .success();

        stead_core()
            .env("STEAD_CORE_RUNNER", runner_script.to_str().unwrap())
            .args([
                "resume",
                "--repo",
                repo.path().to_str().unwrap(),
                "--session",
                &codex_canonical,
                "--backend",
                "claude",
                "--prompt",
                &format!("stress-round-{round}-codex-to-claude"),
            ])
            .assert()
            .success();

        stead_core()
            .env("STEAD_CORE_RUNNER", runner_script.to_str().unwrap())
            .args([
                "resume",
                "--repo",
                repo.path().to_str().unwrap(),
                "--session",
                &claude_canonical,
                "--backend",
                "codex",
                "--prompt",
                &format!("stress-round-{round}-claude-to-codex"),
            ])
            .assert()
            .success();

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
    }

    let refreshed = list_canonical_sessions(repo.path());
    assert_eq!(refreshed.len(), baseline_count);
    let codex_updated = refreshed
        .iter()
        .find(|session| session["session_uid"] == codex_canonical)
        .unwrap();
    assert!(codex_updated["shared_session_uid"].is_string());
    assert!(codex_updated["extensions"]["native_refs"]["codex"]["session_id"].is_string());
    assert!(codex_updated["extensions"]["native_refs"]["claude"]["session_id"].is_string());

    let claude_updated = refreshed
        .iter()
        .find(|session| session["session_uid"] == claude_canonical)
        .unwrap();
    assert!(claude_updated["shared_session_uid"].is_string());
    assert!(claude_updated["extensions"]["native_refs"]["codex"]["session_id"].is_string());
    assert!(claude_updated["extensions"]["native_refs"]["claude"]["session_id"].is_string());
}
