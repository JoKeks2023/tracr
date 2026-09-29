# T-0005 — S1.3 derivation, then the Day view first

Date: 2026-09-29. Follows T-0004 (commit `2df83b4`).

User decision (2026-09-29): "I'd like to start with the real app (with the UI) ASAP so we have something to look at."

The architecture order was S1.3 → S1.4 → S1.5 → S1.6 (Codex) → S1.7 (Tauri Day view). This task reorders it:

1. **S1.3**, in full except `tracr-git` probes: Claude normalizer, copy election, agent timelines, `derived.sqlite` with rebuild swap, `tracr rebuild`, `tracr sessions --day`.
2. **S1.7 Day view shell**, moved ahead of S1.4–S1.6: a Tauri 2 + React window that reads only `tracr-app` query DTOs. Lanes that are not derived yet (human, device, coverage, cost) are shown as "not derived yet", never as zero.

S1.4 (runtime, device sensor, human timeline), S1.5 (cost) and S1.6 (Codex) follow and fill the pending lanes.
