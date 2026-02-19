# Manual Drift Review Playbook

## Purpose

Detect and triage local session format drift in Codex/Claude before it breaks import/export/resume interoperability.

## Cadence

- Run every 2 weeks.
- Run before every release tag.
- Run immediately after upgrading Codex CLI or Claude Code CLI versions.

## Severity rubric

- `S0` Critical:
  - parser crashes on common local sessions,
  - resume/materialize fails in any direction,
  - canonical store duplicates explode in standard sync loop.
- `S1` High:
  - meaningful data loss on import/export (tool links, timestamps, ids),
  - one directional crossover no longer carries context.
- `S2` Medium:
  - non-critical metadata drift (new optional fields) with fallback still working.
- `S3` Low:
  - cosmetic/snapshot-only drift with no behavior change.

## Response path

1. Log findings in `docs/branch-knowledge.md` with date and impacted backend/version.
2. If `S0` or `S1`, open hotfix branch from `main` immediately and add failing tests first.
3. If `S2` or `S3`, batch into next hardening milestone.
4. Update snapshots only after behavior is validated and documented.

## Non-mutating verification command set

Run from repo root:

```bash
./scripts/non_mutating_verify.sh /path/to/target/repo
```

Equivalent manual commands:

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -q -p stead-core-cli -- sessions list --backend codex --base-dir ~/.codex --json | jq '.[0:5]'
cargo run -q -p stead-core-cli -- sessions list --backend claude --base-dir ~/.claude --json | jq '.[0:5]'
```

## Practical interop matrix (release gate)

Run in a fresh repo and record outputs:

1. `Codex -> generic -> Codex -> resume`
2. `Claude -> generic -> Claude -> resume`
3. `Codex -> generic -> Claude -> resume`
4. `Claude -> generic -> Codex -> resume`

Expected result for each:
- resume command exits successfully,
- prompt confirms prior marker/context is found,
- follow-up `sync` does not increase canonical session count unexpectedly.
