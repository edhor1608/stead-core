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
