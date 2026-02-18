# Design Rationale

## Why event-first

Message-only models lose structure for tool actions, progress hooks, and non-chat events.  
An event timeline preserves execution semantics and supports:
- deterministic replay,
- richer debugging,
- future orchestration metadata.

## Why schema versioning now

Session formats drift. Locking `schema_version` from the first milestone enables:
- strict compatibility checks,
- explicit migrations,
- stable downstream integrations.

## Why adapter-local source dirs

Adapters accept explicit base directories instead of hardcoded global paths. This improves:
- testability,
- reproducibility,
- support for custom homes and fixtures.

## Why loss-aware roundtrip tests

Snapshot tests alone can miss semantic loss.  
Roundtrip tests (`import -> export -> import`) assert behavioral fidelity on:
- event count,
- session identity,
- core message/tool semantics.

## v0.1 tradeoffs

- Scope is local disk formats only (no vendor API calls).
- Two backends implemented first: Codex and Claude Code.
- Exporters prioritize stable, parseable output over reproducing every vendor-specific field.
