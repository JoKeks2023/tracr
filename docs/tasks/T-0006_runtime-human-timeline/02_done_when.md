# Done when

M1: collection

- [x] Runtime records `coverage_started`, heartbeats every 60 s and `coverage_ended` on a clean stop (tested; real run)
- [x] OS sensor transitions (`device_idle`, `device_active`, lock, suspend/resume) as `os_sensor` observations (tested; real sample)
- [x] File hints reconcile live (trigger `hint`); new files are found by directory hint (tested; real run: 0.3–4.4 s latency)
- [x] One collector per data directory (lock); the app shows "collecting in another tracr process" (tested; observed)
- [x] User confirmed; collecting into `~/Library/Application Support/tracr` since 2026-09-30 18:49 (`tracr run`), handed over to `tracr.app` at 18:56:53

M2: derivation

- [x] Coverage per key from collector runs: unknown before the first run; missing between runs and on heartbeat stalls (unless asleep); live edge (tested)
- [x] Device lane: active, idle, locked, suspended, unknown outside sensor coverage (tested; real data)
- [x] Human timeline §7.3: walkthrough B 87/20; interleaved 0/25 `conflicting_evidence`; lock → blank; idle ≥ 5 min → unknown + proposal; no coverage → `coverage_gap`; blank outside instants (tested)
- [x] End to end through the collector: 10 counted, 5 unknown (tested)
- [x] INV-6 with human tables at a fixed `as_of` (tested)
- [x] Day view lanes, counted/unknown summary; timesheet and export-everything CSVs
- [x] Product decision: long unbridged gaps (> 2 h) are blank (user, 2026-09-30); `human-timeline@2` (tested)
- [ ] Completeness `recovered`/`verified`/`unverified` and the provisional flag (moved to M3)
- [ ] Midnight/DST scenario at app level (core day bounds tested; atoms split at local midnight)

M3: see [01_scope](01_scope.md).

CSV export (pulled forward)

- [x] `agent-sessions` export: CLI `tracr export agent-sessions` and the app's Export CSV (native save panel in the host); columns documented in `docs/export.md` (tested; real export of September)
- [x] Timesheet export of human time; export everything (folder)
- [x] No tool-specific profile: the user wants a complete export (2026-09-30)
