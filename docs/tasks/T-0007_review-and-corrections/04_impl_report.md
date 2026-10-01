# Implementation report

## S2.1 corrections (2026-10-01)

| Part | Where |
|---|---|
| `TargetRef` (repository, category, other) and fixed categories; correction ops; `apply_corrections` (counting and attribution axes, revokes, candidate kept underneath); totals | `crates/tracr-core/src/counting.rs` |
| Ledger: `append_correction`, `corrections`, `max_action_seq` | `crates/tracr-store/src/ledger.rs` |
| `App::correct` (validation; `user_action` + `correction` in one transaction; a revoke is recorded on the axis of what it undoes) | `crates/tracr-app/src/correct.rs` |
| Derivation: counting stage between candidates and atoms; atoms carry `decided_by`, candidate status and reason, explicit target; explicit attribution level wins the report target | `crates/tracr-app/src/derive_human.rs` |
| Day DTO: per-atom correction fields, correction history with status (effective, partly superseded, superseded, revoked); targets list | `crates/tracr-app/src/day.rs`, `lib.rs` (`targets`) |
| Runtime `Job::Correct` (records, then updates the derived store at once); desktop `correct` and `targets` commands | `runtime.rs`, `apps/desktop/src-tauri/src/main.rs` |
| UI: clickable human intervals → interval panel (count / break / undo, target dropdown with "Other…", history); "N to review" → review list with quick Work and Break | `apps/desktop/ui/src/{IntervalSheet,ReviewList,Timeline,App}.tsx` |

Corrections are range-anchored (§6.4): they survive every re-derivation and can never become orphans. Orphans appear only with segment- or turn-anchored corrections (later).
