# Validation report

- `cargo fmt --all --check`: ok.
- `cargo clippy --workspace --all-targets -- -D warnings`: ok.
- `cargo test --workspace --exclude tracr-desktop`: all green. Walkthrough C (52/13) and walkthrough B (87/20) are unchanged, because their fixtures have no agent segment ends.
- **Real data.** I copied the local ledger into a temporary folder and ran a debug CLI `rebuild` on the copy. The live ledger was not written.
  - Before the change, the recent days had 0 human minutes.
  - With only the new rule, they still had 0, because every gap was blocked as degraded.
  - With the rule and the record-type fix, every recent day has counted human minutes.
  - The remaining unknown time is mostly long autonomous runs that got a reply more than 5 min later. That is intended.
  - Real figures stay out of this public repo.
- **Not verified:**
  - Re-derivation inside the installed app after an update.
  - The display in the desktop app. Only CLI output was checked.
