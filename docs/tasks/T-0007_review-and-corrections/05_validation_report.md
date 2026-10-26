# Validation report

## S2.1 (2026-10-01)

`cargo fmt --all --check`, workspace clippy with `-D warnings`, `cargo test --workspace` and `pnpm typecheck` pass: 129 tests.

| Check | Status | Evidence |
|---|---|---|
| Walkthrough B outcomes | Tested | `counting::tests::walkthrough_b_outcomes` |
| Supersession and undo | Tested | `a_later_correction_supersedes_only_the_overlap_and_revoke_restores` |
| Blank confirm; targets do not change minutes; reject | Tested | `confirming_blank_time_counts_it_and_attribution_never_changes_minutes` |
| End to end and INV-5 | Tested | `runtime.rs::corrections_confirm_and_undo_without_touching_agent_records` |
| App UI (panel, review list) | **Unverified** | Not deployed yet; see 02_done_when |


## S2.2 and S2.3 in the real window (added 2026-10-26)
Corrections (count as work, break, target, undo), manual entries (add, edit, delete, timer), the review of a day (r1) and the discrepancy banner (accept or keep) were clicked in the real desktop app on a throwaway ledger of invented data. Evidence per step and the open points are in `docs/tasks/T-0012_s2-click-through/05_validation_report.md` (14 of 16 checks passed outright, 2 partly or with a finding; the macOS save panel and the folder panel were not driven). Not covered there: Accept as r2 and a free-label target.
