# Validation report

- `cargo fmt --all --check`: ok.
- `cargo clippy --workspace --all-targets -- -D warnings`: ok.
- `cargo test --workspace --exclude tracr-desktop`: all green. Walkthrough C (52/13) and walkthrough B (87/20) are unchanged, because their fixtures have no agent segment ends.
- **Real data.** I made a copy of the live ledger with `sqlite3 .backup` into the scratchpad and ran a debug CLI `rebuild` on it. The live ledger was not written.

  | Day | Before (0.10.1) | Rule only | Rule + record types |
  |---|---|---|---|
  | 10-24 | 0 / 446 unknown | 0 | 134 / 313 |
  | 10-26 | 0 / 312 | 0 | 58 / 254 |
  | 10-27 | 0 / 221 | 0 | 47 / 174 |
  | 10-29 | 0 / 540 | 0 | 63 / 477 |
  | 11-02 | 0 / 16 | 0 | 5 / 11 |

  - The remaining unknown time is mostly long autonomous runs that got a reply more than 5 min later. That is intended.
  - Before the record-type fix, 2,751 min already counted, but only on days before 10-22.
- **Not verified:**
  - Re-derivation inside the installed app after an update.
  - The display in the desktop app. Only CLI output was checked.
