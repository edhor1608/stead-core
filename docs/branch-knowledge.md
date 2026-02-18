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
