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
