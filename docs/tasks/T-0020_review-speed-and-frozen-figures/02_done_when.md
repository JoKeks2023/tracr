# Done when

- [x] The cause of the r2 hang is measured, not guessed (job trace + stack samples on a copy of the real ledger).
- [x] A review on that copy takes a few seconds in a release build (was 11–24 s).
- [x] A test fails if the update's ledger queries fall back to table scans.
- [x] Today and work-time pages show the revision's unclear-window count on a reviewed day; "Den Tag einteilen" is labelled as the current state.
- [x] fmt, clippy, `cargo test --workspace --exclude tracr-desktop` and the UI typecheck are green.
- [ ] Clicked through in the running app (reviewed day with later changes). Not done yet.
- [ ] Merged and released; the installed app migrates the live ledger to v4.
