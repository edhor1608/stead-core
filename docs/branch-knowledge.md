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
