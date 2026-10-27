# T-0015 step 1 validation report

Checked: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p tracr-app --test sessions --test derive --test projects` (new `tests/sessions.rs`: 2 tests), `tsc --noEmit`, `cargo build -p tracr-desktop`.
Seen in the real window (dark, German, wide): sidebar with three destinations, Heute page with the empty state (the demo ledger has no activity today), toolbar, timer control.

NOT verified: the Sitzungen list and detail, Arbeitszeit after the move, the palette, the new menu shortcuts, the period control, light mode, English, narrow window, Tab order, VoiceOver. Reason: checking them needs clicking in the running app, which is not done.
Design-review gate (12 questions): not completed; 2-11 need the unverified items above.
