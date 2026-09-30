# Done when

M1: collection

- [x] Runtime records `coverage_started`, heartbeats every 60 s and `coverage_ended` on a clean stop (tested; real run)
- [x] OS sensor transitions (`device_idle`, `device_active`, lock, suspend/resume) as `os_sensor` observations (tested; real sample)
- [x] File hints reconcile live (trigger `hint`); new files are found by directory hint (tested; real run: 0.3–4.4 s latency)
- [x] One collector per data directory (lock); the app shows "collecting in another tracr process" (tested; observed)
- [ ] User confirms switching to the permanent data directory

M2 / M3: see [01_scope](01_scope.md); criteria are architecture §13.1 S1.4.
