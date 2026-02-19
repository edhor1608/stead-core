# Release Baseline: v0.1.0-m2 (Hardening)

- Date: 2026-02-19
- Target branch: `main`

## Included hardening milestones

- M9 shared-session interop and resume compatibility fixes.
- M10 edge-case fixture/contract hardening.
- M11 long-session stress and reliability cycle checks.
- M12 manual drift review process and reproducible verification checklist.

## Baseline checks

- `cargo test --workspace`: pass
- `cargo fmt --all --check`: pass
- `cargo clippy --workspace --all-targets -- -D warnings`: pass

## Operational gates

- Manual drift review playbook in place: `docs/MANUAL_DRIFT_REVIEW.md`
- Non-mutating verification script in place: `scripts/non_mutating_verify.sh`
- Practical interop matrix run recorded: `docs/m12-verification-report-2026-02-19.md`
