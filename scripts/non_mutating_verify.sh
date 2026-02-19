#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_REPO="${1:-}"
CODEX_BASE="${CODEX_BASE:-$HOME/.codex}"
CLAUDE_BASE="${CLAUDE_BASE:-$HOME/.claude}"

cd "$ROOT_DIR"

echo "[verify] cargo test --workspace"
cargo test --workspace

echo "[verify] cargo fmt --all --check"
cargo fmt --all --check

echo "[verify] cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

echo "[verify] stead-core sessions list codex"
cargo run -q -p stead-core-cli -- sessions list --backend codex --base-dir "$CODEX_BASE" --json \
  | jq '.[0:5] | map({native_id, project_root, file_path, updated_at})'

echo "[verify] stead-core sessions list claude"
cargo run -q -p stead-core-cli -- sessions list --backend claude --base-dir "$CLAUDE_BASE" --json \
  | jq '.[0:5] | map({native_id, project_root, file_path, updated_at})'

if [[ -n "$TARGET_REPO" ]]; then
  echo "[verify] repo-scoped session candidates for: $TARGET_REPO"
  cargo run -q -p stead-core-cli -- sessions list --backend codex --base-dir "$CODEX_BASE" --json \
    | jq --arg repo "$TARGET_REPO" '[.[] | select(.project_root == $repo)][0:3] | map({native_id, project_root, file_path, updated_at})'
  cargo run -q -p stead-core-cli -- sessions list --backend claude --base-dir "$CLAUDE_BASE" --json \
    | jq --arg repo "$TARGET_REPO" '[.[] | select(.project_root == $repo)][0:3] | map({native_id, project_root, file_path, updated_at})'
fi

echo "[verify] done"
