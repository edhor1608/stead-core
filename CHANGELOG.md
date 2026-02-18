# Changelog

All notable changes to this project are documented in this file.

## v0.1.0-m1 - 2026-02-18

Milestone baseline release for canonical session model + Codex/Claude adapters + core CLI workflows.

### Merged PRs

- [#2](https://github.com/edhor1608/stead-core/pull/2): M2 Codex adapter import/export + round-trip tests
- [#7](https://github.com/edhor1608/stead-core/pull/7): M3 Claude adapter import/export + subagent merge + round-trip tests (replacement for closed #4)
- [#3](https://github.com/edhor1608/stead-core/pull/3): M4 CLI workflows (`sessions list`, `import`, `export`, `convert`) + e2e tests
- [#5](https://github.com/edhor1608/stead-core/pull/5): M5 repo sync + materialize + resume orchestration
- [#6](https://github.com/edhor1608/stead-core/pull/6): M6 live compatibility hardening for Codex/Claude local sessions

### Included capabilities

- Versioned canonical session schema (`0.1.0`) and deterministic event ordering.
- Vendor adapters for Codex and Claude Code with loss-minimizing import/export behavior.
- CLI support for listing/import/export/convert/sync/materialize/resume workflows.
- Regression coverage across unit/integration/snapshot/e2e workflows.
