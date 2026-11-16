# Validation report

- `cargo test -p tracr-app`: all suites ok (incl. 5 new integration tests and 4 unit tests).
- `cargo clippy -p tracr-app -p tracr-store --all-targets -D warnings`: clean. `cargo check -p tracr-desktop`: ok. `pnpm typecheck`: ok.
- **Not verified:** any UI (none built); a real ledger (not run, the machine was busy); full workspace tests (left to CI).
