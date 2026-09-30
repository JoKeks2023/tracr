# Validation report

## M1: collection (2026-09-30)

`cargo fmt --all --check`, workspace clippy with `-D warnings` (including `tracr-desktop`) and `cargo test --workspace` pass: 101 tests.

| Check | Status | Evidence |
|---|---|---|
| Tracker rules (idle at the last input, bounded active, lock, kernel wake, no pre-start wake) | Tested | `tracr-core::device` (5 tests) |
| Lifecycle and device records | Tested | `tracr-app/tests/runtime.rs::a_run_records_coverage_lifecycle_and_device_transitions` |
| Live hints and directory hints | Tested | `hints_read_known_files_live_and_discover_new_files` |
| Single collector | Tested | `only_one_runtime_can_collect` |
| Real sensor | Manually verified | The sample read idle, lock state and the last wake. The wake time matched `pmset -g log` (15:43:22 vs 15:43:24 local). |
| Real `tracr run` (scratch copy) | Manually verified | Startup reconcile of 489 sources; `device_idle` recorded while the user was away; heartbeats every 60 s; live `hint` records of the running Claude session 0.3–4.4 s after they were written; Ctrl-C wrote `coverage_ended` (reason `interrupted`). Idle RSS 12 MB. |
| App with the collector held elsewhere | Manually verified | Screenshot: "Collecting in another tracr process"; Refresh disabled. |
| App as the collector; tray; hide on close; first-run approval | **Unverified** | Not clicked (no UI automation); compiles and type-checks. |
| IOKit power notifications | Not built | Deviation 1 in [01_scope](01_scope.md) |

## Switch and export (2026-09-30)

| Check | Status | Evidence |
|---|---|---|
| Permanent directory collecting | Manually verified | `coverage_started` (tracr-cli, 18:49:26) → `coverage_ended` (interrupted, 18:56:43) → `coverage_started` (tracr-desktop, 18:56:53); heartbeats every 60 s; `PRAGMA quick_check` ok after the disk-full incident |
| Release app | Manually verified | Screenshot: "Collecting", Export CSV and Refresh present; UI renders under the production CSP |
| CSV format and grain | Tested | `agent_sessions_csv_has_one_row_per_day_session_actor_and_prompts_once` (RFC 4180 quoting, column count, prompts only on the main row, tokens per actor); `export::fields_are_quoted_per_rfc4180` |
| Real export | Manually verified | September: 510 rows, 21 days, 39 sessions, 582 human prompts, 15.12 M output tokens; read while the collector was writing |
| App export flow (sheet → save panel → file) | **Unverified** | Not clicked |
| Tray, hide on close, Quit → `coverage_ended` from the app | **Unverified** | Not clicked |

`cargo fmt --all --check`, workspace clippy with `-D warnings` and `cargo test --workspace` pass: 103 tests.
