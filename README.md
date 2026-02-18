# stead-core

Vendor-neutral core for importing, normalizing, and exporting local AI coding sessions.

`stead-core` provides:
- a canonical, versioned session model (`schemas/session.v0.1.0.schema.json`),
- adapters for Codex and Claude Code local session formats,
- a CLI proving end-to-end workflows (`list`, `import`, `export`, `convert`, `sync`, `materialize`, `resume`, `handoff`).

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

Sync all local backend sessions into a repo-local canonical store:

```bash
stead-core sync \
  --repo /path/to/repo \
  --codex-base ~/.codex \
  --claude-base ~/.claude
```

Materialize a canonical session into a target backend-native session:

```bash
stead-core materialize \
  --repo /path/to/repo \
  --session <canonical-session-uid> \
  --to claude \
  --base-dir ~/.claude \
  --out /tmp/materialized-claude.jsonl
```

Resume backend session from canonical mapping with a new prompt:

```bash
stead-core resume \
  --repo /path/to/repo \
  --session <canonical-session-uid> \
  --backend codex \
  --prompt "Continue from previous state"
```

Handoff from canonical session to target backend and resume in one step:

```bash
stead-core handoff \
  --repo /path/to/repo \
  --session <canonical-session-uid> \
  --to claude \
  --base-dir ~/.claude \
  --resume "Continue from previous state"
```
