# Stead Session Standard v0.1.0

## Canonical shape

Top-level object:
- `schema_version` (locked to `0.1.0`)
- `session_uid` (`stead:<backend>:<native_id>`)
- `source`
- `metadata`
- `events` (ordered timeline)
- `artifacts`
- `capabilities`
- `extensions`
- `raw_vendor_payload`

Schema file:
- `schemas/session.v0.1.0.schema.json`

## Event-first design

The canonical source of truth is `events[]`, not `messages[]`.

Supported event kinds:
- `message_user`
- `message_assistant`
- `tool_call`
- `tool_result`
- `system_progress`
- `system_note`
- `session_marker`
- `artifact_ref`

Each event contains:
- `event_uid`
- `stream_id` (`main` or `subagent:*`)
- `line_number`
- `sequence` (assigned deterministically)
- `timestamp`
- `kind`
- `payload`
- `raw_vendor_payload`

## Deterministic ordering

Canonical sort rule:
1. `timestamp` ascending
2. `stream_id` priority (`main` first)
3. `line_number` ascending
4. `event_uid` ascending

`sequence` is assigned after sorting and validated for contiguity.

## Loss handling

- Raw original lines are preserved under `raw_vendor_payload` for adapter debugging and loss audits.
- Unknown/extra data can be stored in `extensions`.
- Export reports include `warnings` and `losses` for explicit fidelity reporting.
