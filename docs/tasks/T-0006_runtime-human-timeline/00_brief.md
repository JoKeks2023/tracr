# T-0006: S1.4 runtime, device sensor, human timeline

Date: 2026-09-30. Follows T-0005 (commits `a658727`, `ae01e92`).

User decisions (2026-09-30):

- The 10-minute quiet-gap rule for agent segments is right ("just about right").
- S1.4 is next.

Order within S1.4, chosen because evidence is time-critical: every day without the device sensor has no countable human time.

1. **M1: collection.** `tracr run` plus the desktop app hosting the same runtime: file hints, sweeps, discovery, collector lifecycle and heartbeats, and the macOS sensor (idle, lock, sleep/wake). After M1, the user is asked to switch to the permanent data directory.
2. **M2: derivation.** Coverage and completeness, the device lane, the human timeline and counting (no user records). The Day view's pending lanes are filled.
3. **M3:** structural attribution (with `tracr-git` probes, deferred from S1.3) and incremental derivation with dirty units (INV-6, incremental half).

User decisions (2026-09-30, later):

- **Switch to the permanent data directory now.** The Claude root is approved via the CLI on the user's behalf (`tracr-cli` user action).
- **CSV export is needed**, pulled forward from S4, for all three uses: timesheet/billing, agent usage analysis, and import elsewhere (target tool not yet named).
  - The agent-sessions export is built now.
  - The timesheet export follows with M2 (human time).
  - A tool-specific column profile will follow once the tool is named.

User decision (2026-09-30): **long gaps are blank.** An unbridged gap longer than 2 h is a break (blank), not unknown, and it separates unknown windows. Shorter unbridged gaps stay unknown for review. Rule `human-timeline@2`, parameter `max_unknown_gap_ms = 7200000` (AD-13, adjustable).
