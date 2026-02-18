# Release Baseline: v0.1.0-m1

- Date: 2026-02-18
- Target branch: `main`

## Baseline checks

- Branch protection API (`GET /branches/main/protection`): unavailable on current private repo plan (HTTP 403).
- Repository rulesets API (`GET /rulesets`): unavailable on current private repo plan (HTTP 403).
- GitHub Actions workflows configured: none (`total_count: 0`).

## Actionable follow-ups

- Add CI workflows for `cargo test --workspace`, `cargo fmt --check`, and `cargo clippy --workspace -- -D warnings`.
- Enable required checks and branch protection once repository plan/settings permit it.
