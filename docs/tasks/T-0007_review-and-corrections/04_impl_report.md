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

## S2.2 manual entries and the timer (2026-10-01)

| Part | Where |
|---|---|
| `ManualEntry`; counting precedence correction → manual → inferred → unknown; timer → `timer_measured`, retroactive → `user_asserted`; manual targets (one confirmed, several conflicting) | `crates/tracr-core/src/counting.rs` (`count`) |
| Ledger: `manual_entry_rev` revisions (`append_manual_rev`, `manual_entries` = current revision) | `crates/tracr-store/src/ledger.rs` |
| `App::manual`: start timer (stops a running one at the same instant, AD-15), stop, add, edit, delete, each as one user action plus revisions | `crates/tracr-app/src/manual.rs` |
| Derivation: running timer ends at `as_of`; `decided_by = manual_entry:<id>@<rev>`; explicit attribution with origin `manual` | `crates/tracr-app/src/derive_human.rs` |
| Runtime `Job::Manual`; a running timer marks the store dirty every heartbeat, so its minutes grow | `crates/tracr-app/src/runtime.rs` |
| Desktop: `manual` command; `running_timer` in status; menu-bar item shows elapsed time, its menu has Start/Stop timer | `apps/desktop/src-tauri/src/main.rs` |
| UI: timer control in the header (start with target, live clock, stop), "Add time" sheet, edit or delete from the interval panel | `apps/desktop/ui/src/{TimerControl,EntrySheet,TargetSelect,IntervalSheet,App}.tsx` |
| `derived.sqlite` in WAL mode (schema v5): readers never wait for an update | `crates/tracr-store/src/derived.rs` |

**Decision (conservative default):** starting a timer while one runs stops the running one at the same instant. AD-15 allows at most one running timer and does not say how a second start is handled.

## S2.3 review (2026-10-01)

| Part | Where |
|---|---|
| Ledger migration v2 (`worklog_revision`, `discrepancy_dismissal`, append-only triggers); the migration test is now version-generic | `crates/tracr-store/migrations/ledger/0002_worklog.sql`, `migrate.rs`, `tests/immutability.rs` |
| Snapshot of the resolved day by value; fingerprint over intervals, status, basis and attribution; compare and attach; `review_day` (reason required from r2), `dismiss_discrepancy` | `crates/tracr-app/src/review.rs` |
| A reviewed day is provisional only while a discrepancy is open | `review::attach` |
| Runtime jobs Review and Dismiss (review brings the derived store up to date first); desktop `review` and `dismiss` commands | `runtime.rs`, `apps/desktop/src-tauri/src/main.rs` |
| Timesheet: reviewed revision for reviewed days; new columns `worklog_state`, `non_work_minutes`, `duration_basis`, `target`, `target_key` (the boundary and reason columns were dropped) | `crates/tracr-app/src/export.rs` |
| UI: "Review day" sheet (what gets frozen; unknown stays uncounted), read-only "Reviewed rN" lane, summary from the revision, discrepancy banner (Accept as rN+1, Keep rN) | `apps/desktop/ui/src/{ReviewDaySheet,App,Timeline}.tsx` |

- **AD-18:** attribution is stored as shown, so inferred stays inferred. Review freezes counting and records attribution by value.
- **Fingerprint.** Only what review freezes is part of it. A rebuild at a later `as_of` with the same resolved intervals produces no discrepancy.
