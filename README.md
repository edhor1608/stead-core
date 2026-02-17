# stead-core

Vendor-neutral core for importing, normalizing, and exporting local AI coding sessions.

`stead-core` provides:
- a canonical, versioned session model (`schemas/session.v0.1.0.schema.json`),
- adapters for Codex and Claude Code local session formats,
- a CLI proving end-to-end workflows (`list`, `import`, `export`, `convert`).

## Why this standard exists

Local sessions from different agent CLIs are incompatible. This blocks:
- continuing a session in another tool,
- building stable tooling on top of session history,
- loss-aware analysis of prompts, tool calls, and outcomes.

The Stead Session Standard is event-first and vendor-neutral to preserve timeline fidelity while enabling cross-backend conversion.

## Project structure

```text
crates/
  stead-session-model/      # Canonical model + schema contract tests
  stead-session-adapters/   # Codex + Claude adapters
  stead-core-cli/           # CLI (stead-core)
schemas/
  session.v0.1.0.schema.json
docs/
  SESSION_STANDARD.md
  DESIGN_RATIONALE.md
  branch-knowledge.md
```

## Build and test

```bash
cargo test --workspace
```

## CLI usage

List sessions from one backend:

```bash
stead-core sessions list --backend codex --base-dir ~/.codex --json
stead-core sessions list --backend claude --base-dir ~/.claude --json
```

Import native session to canonical JSON:

```bash
stead-core import \
  --from codex \
  --base-dir ~/.codex \
  --session <native-session-id> \
  --out /tmp/session.canonical.json
```

Export canonical JSON to native format:

```bash
stead-core export \
  --to claude \
  --base-dir ~/.claude \
  --in /tmp/session.canonical.json \
  --out /tmp/claude-session.jsonl
```

Convert backend-to-backend in one command:

```bash
stead-core convert \
  --from codex \
  --to claude \
  --source-base ~/.codex \
  --target-base ~/.claude \
  --session <native-session-id> \
  --out /tmp/converted-claude.jsonl
```
