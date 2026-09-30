# Done when

M1: collection

- [x] Runtime records `coverage_started`, heartbeats every 60 s and `coverage_ended` on a clean stop (tested; real run)
- [x] OS sensor transitions (`device_idle`, `device_active`, lock, suspend/resume) as `os_sensor` observations (tested; real sample)
- [x] File hints reconcile live (trigger `hint`); new files are found by directory hint (tested; real run: 0.3–4.4 s latency)
- [x] One collector per data directory (lock); the app shows "collecting in another tracr process" (tested; observed)
- [x] User confirmed; collecting into `~/Library/Application Support/tracr` since 2026-09-30 18:49 (`tracr run`), handed over to `tracr.app` at 18:56:53

M2 / M3: see [01_scope](01_scope.md); criteria are architecture §13.1 S1.4.

CSV export (pulled forward)

- [x] `agent-sessions` export: CLI `tracr export agent-sessions` and the app's Export CSV (native save panel in the host); columns documented in `docs/export.md` (tested; real export of September)
- [ ] Timesheet export of human time (after M2)
- [ ] Column profile for the target import tool (tool not named yet)
