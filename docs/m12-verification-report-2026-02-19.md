# M12 Verification Report (2026-02-19)

## Scope

- Manual drift checklist execution (non-mutating command set).
- Final practical interop matrix execution in fresh repo.

## Non-mutating drift checklist

Command:

```bash
./scripts/non_mutating_verify.sh /Users/jonas/repos/stead-core-live-m12
```

Result:
- `cargo test --workspace`: pass
- `cargo fmt --all --check`: pass
- `cargo clippy --workspace --all-targets -- -D warnings`: pass
- Codex and Claude session listing commands returned valid JSON arrays.
- Repo-scoped candidates for `/Users/jonas/repos/stead-core-live-m12` were discoverable.

## Final practical interop matrix

Fresh repo:
- `/Users/jonas/repos/stead-core-live-m12`

Recorded flow checks:

1. `Codex -> generic -> Codex -> resume`: pass (`FOUND:C_MARKER_M12_001`)
2. `Claude -> generic -> Claude -> resume`: pass (`FOUND:Q_MARKER_M12_002`)
3. `Codex -> generic -> Claude -> resume`: pass (`FOUND:C_MARKER_M12_001`)
4. `Claude -> generic -> Codex -> resume`: pass (`FOUND:Q_MARKER_M12_002`)

Store stability:
- Canonical file count before loop: `2`
- Canonical file count after final sync: `2`
- Unique session_uids in final sync output: `2`

## Limits observed

- Non-fatal MCP startup warnings can appear during backend CLI resumes (e.g., stale Notion token), but did not affect resume/sync correctness.
