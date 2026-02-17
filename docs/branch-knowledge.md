# Branch Knowledge

## Milestone 1: Canonical Model + Schema Contract

### Problem solved
- Establish a versioned, event-first canonical session model as the foundation for adapter work.
- Lock deterministic ordering and sequence validation rules before any importer/exporter code.

### What was implemented
- Rust workspace scaffold.
- `stead-session-model` canonical types and validation primitives.
- JSON Schema `v0.1.0` for serialized session contracts.
- Test suite with:
  - unit tests (ordering, sequence validation, schema version lock),
  - property tests (UID determinism),
  - snapshot test (canonical JSON stability),
  - schema validation tests (valid/invalid payload contracts).

### Key decisions
- Canonical ordering: timestamp, then stream priority (`main` first), then line number, then event uid.
- Canonical IDs: `stead:<backend>:<original_session_id>`.
- `schema_version` is locked to `0.1.0` for this milestone.

### Lessons learned
- Snapshot tests require fixed timestamps; dynamic `now()` values create flaky snapshots.
- Store sequence as an explicit event field and validate contiguity in the model itself.

## Milestone 2: Codex Adapter (Import/Export/Round-Trip)

### Problem solved
- Parse real Codex-style JSONL streams into canonical sessions.
- Export canonical sessions back into Codex JSONL shape.
- Prove loss-minimizing round-trip for core semantics.

### What was implemented
- `CodexAdapter` with:
  - recursive session discovery under `sessions/**/*.jsonl`,
  - session summary listing sorted by recency,
  - import by native id and import by file path,
  - canonical export with stable JSONL envelopes.
- Codex-focused fixture corpus and test suite:
  - discovery/integration tests,
  - round-trip contract test,
  - export snapshot test.

### Key decisions
- Preserve full input lines in `raw_vendor_payload.lines` for loss-aware debugging.
- Map `event_msg.token_count` into canonical `system_progress` events.
- Emit explicit `token_count` envelopes on export so imported event counts stay stable.

### Lessons learned
- Round-trip tests uncovered real lossiness early (progress events); snapshot-only tests would not.

## Milestone 3: Claude Adapter (Import/Export/Subagent Merge)

### Problem solved
- Parse Claude Code JSONL sessions into canonical events, including mixed content formats.
- Merge main + subagent streams into a deterministic single timeline.
- Export canonical sessions back to Claude-compatible JSONL and verify round-trip.

### What was implemented
- `ClaudeAdapter` with:
  - discovery of main sessions under `projects/**/*.jsonl` (excluding `subagents` for listing),
  - import by session id with automatic subagent merge,
  - import from a single file with explicit stream id,
  - export to Claude-style event lines.
- Test suite additions:
  - session list/import integration tests,
  - subagent merge behavior contract,
  - round-trip test,
  - export snapshot test.

### Key decisions
- Main stream id is `main`; subagents are tagged as `subagent:<file_stem>`.
- Claude `message.content` supports both string and array forms and is normalized event-by-event.
- Progress entries are modeled as canonical `system_progress` events.

### Lessons learned
- Subagent files can share the same Claude `sessionId`; merging must rely on both session id and stream origin.

## Milestone 4: CLI + E2E Workflow Proof

### Problem solved
- Provide an end-user interface to run real import/export/convert flows without writing code.
- Prove all core workflows are runnable through the binary.

### What was implemented
- `stead-core` CLI commands:
  - `sessions list`
  - `import`
  - `export`
  - `convert`
- E2E CLI test suite covering:
  - backend listing,
  - import to canonical JSON,
  - export from canonical JSON,
  - codex -> claude conversion,
  - claude -> codex conversion.
- Docs for standard, rationale, and usage.

### Key decisions
- `export` supports both `--in` and `--input` for script compatibility.
- CLI remains local-first with explicit `--base-dir` paths for deterministic tests and ops.

### Lessons learned
- CLI alias compatibility (`--in` vs `--input`) should be asserted in tests to avoid accidental interface breaks.

## Milestone 5: Repo Sync + Materialize + Resume

### Problem solved
- Enable practical shared-session workflow at canonical level:
  - sync backend sessions into repo canonical store,
  - materialize canonical sessions into target backend native formats,
  - resume backend sessions from canonical mapping with a new prompt.

### What was implemented
- New CLI commands:
  - `sync`
  - `materialize`
  - `resume`
- Repo-local canonical store:
  - `.stead-core/sessions/*.json`
- Native session reference tracking under:
  - `extensions.native_refs.codex`
  - `extensions.native_refs.claude`
- E2E tests validating sync/materialize/resume flows.

### Key decisions
- Resume command can use a test runner override via `STEAD_CORE_RUNNER` for deterministic e2e testing.
- If a native projection is missing, `resume` can materialize first when `--base-dir` is provided.
- Canonical session identity stays stable; backend projections are tracked as refs rather than replacing canonical IDs.

### Lessons learned
- Runtime orchestration needs explicit native ref tracking to avoid backend/session-ID ambiguity.

## Milestone 6: Live Session Compatibility Hardening

### Problem solved
- Real local session directories from current Codex/Claude CLI runs were not discoverable when users passed direct leaf roots (`~/.codex/sessions`, `~/.claude/projects`).
- Claude discovery/import could fail globally when unrelated session files had malformed JSON or unsupported message content shapes.

### What was implemented
- Adapter root normalization:
  - Codex accepts both home root and direct `sessions` root.
  - Claude accepts both home root and direct `projects` root.
- Sync repo scoping:
  - `sync` now prefers sessions whose `project_root` matches the target repo path.
  - legacy fallback remains: if no matches exist for a backend, sync keeps previous behavior and imports all discovered sessions.
- Discovery/import resilience:
  - `list_sessions` and `import_session` now skip unparseable unrelated files instead of failing the whole operation.
- Claude content parsing hardening:
  - support for raw/unknown `message.content` shapes via fallback variant.
  - support non-string `tool_result.content` by preserving JSON as string output.
- New regression tests:
  - adapter tests for leaf base dirs,
  - malformed-file tolerance tests,
  - Claude non-string `tool_result.content` import test,
  - CLI e2e tests for `sync` with leaf backend directories and repo-scoped filtering behavior.

### Key decisions
- Favor graceful degradation for discovery paths: skip invalid files, keep parsing valid sessions.
- Keep canonical fidelity for odd tool-result payloads by stringifying unknown JSON content instead of dropping events.

### Lessons learned
- Local user histories contain heterogeneous historical formats; strict per-file parsing in discovery paths is too brittle.
- Compatibility tests must include real-world path variants (`root` vs `leaf`) to avoid “works on fixtures only” regressions.
