# Constraints and context

- Architecture invariants are binding:
  - `ledger.sqlite` immutable (trigger-enforced);
  - one read path, `reconcile(source)`; transcript files are the durable source;
  - provider-free, IO-free core;
  - no generic total cost;
  - nothing turns agent time or unknown time into human time.
- Privacy: no prompt, message or tool content is read into the ledger or committed. Local transcripts were inspected only for key names and enum-like values. The committed fixtures are structure-only (see `crates/tracr-provider-claude/fixtures/README.md`).
- Machine: M1 with 8 GB RAM. Rust 1.98.1 was installed via rustup at the user's request (`~/.cargo`, `~/.rustup`). `rust-toolchain.toml` pins 1.98.1 with rustfmt and clippy.
- Dogfooding used scratch data directories, not the default `~/Library/Application Support/tracr`. A pre-release import into an immutable ledger should not become the product's permanent state.
- No commit, push, PR, GitHub or Huly write. There is no existing Huly or GitHub issue for T-0004 to update, and none was created (planned work, not a discovered bug or TODO).
