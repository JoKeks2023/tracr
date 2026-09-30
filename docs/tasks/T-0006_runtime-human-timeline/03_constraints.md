# Constraints

- The invariants from T-0005 apply unchanged.
- The OS sensor records device state only: idle duration, lock, sleep and wake. It records no application, window, keystroke or screen content (PS-01) and needs no macOS privacy permission.
- The permanent data directory is not written until the user confirms.
- No push.
- **Disk is tight** (about 5–7 GB free on a 228 GB volume). A Tauri release build filled the disk on 2026-09-30.
  - Rule: keep at most one cargo profile's build cache in `target/`, and delete `target/release` after bundling.
  - The collector kept writing and the ledger passed `quick_check`; heartbeats had no gap.
