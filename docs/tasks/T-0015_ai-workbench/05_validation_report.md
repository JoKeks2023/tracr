# T-0015 step 1 validation report

Checked: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p tracr-app --test sessions --test derive --test projects` (new `tests/sessions.rs`: 2 tests), `tsc --noEmit`, `cargo build -p tracr-desktop`.
Seen in the real window (dark, German, wide): sidebar with three destinations, Heute page with the empty state (the demo ledger has no activity today), toolbar, timer control.

NOT verified: the Sitzungen list and detail, Arbeitszeit after the move, the palette, the new menu shortcuts, the period control, light mode, English, narrow window, Tab order, VoiceOver. Reason: checking them needs clicking in the running app, which is not done.
Design-review gate (12 questions): not completed; 2-11 need the unverified items above.

## Step 2

Checked: fmt, clippy `-D warnings`, `cargo test -p tracr-app --test sessions --test derive`, `tsc --noEmit`, debug build.
NOT verified: how Ablauf, Kontext and Werkzeuge look in the real window (no click into the running app), light mode, English, narrow window, keyboard use of the tabs, VoiceOver. The charts are SVG with a text label for assistive tech; their hover details are `<title>` tooltips only.

## Prices, limits, projects (backend)

Checked: clippy `-D warnings`, `cargo test` of tracr-app/core, the tap end to end with the real debug binary against a temp settings file (install, chained command output, spool line holds only the numbers, uninstall restores the file), `tracr limits show` against the demo ledger (Codex 5 h and weekly figures read from the real rollouts), prices visible in `tracr day` after the rebuild.
NOT done: installing the tap into the real `~/.claude/settings.json` (it is an opt-in action for the user, not run in this session); no UI for limits or the projects page yet; the tap has not been seen with a live Claude Code session, so it is not verified that the fields arrive on this account (T-0010 caveat stands).

## design/v2

Checked: fmt, clippy `-D warnings`, `cargo test --workspace --exclude tracr-desktop`, `tsc --noEmit`, debug build; the window (dark, German, wide) for Heute, Projekte, Limits and Zeit against the demo ledger (screenshots of the tracr window only).
NOT verified: light appearance, English, narrow window, keyboard use and focus order, VoiceOver, the menu shortcuts (⌘1 to ⌘4, ⌘K), session page tabs and the Claude tap switch in the real window (no clicking in the running app), the tap with a live Claude Code session. The design-review gate (12 questions) is not completed.

## Quota, menu bar, hooks

Checked: fmt, clippy `-D warnings`, `cargo test --workspace --exclude tracr-desktop` (quota 8, hooks 3, limits 6 tests), `tsc`; the tap and the hooks end to end with the real debug CLI against temp settings (install keeps existing hooks, payload text is not stored, uninstall restores the file); Heute in the real window with live Claude figures from the user's own tap (Claude 5 h and weekly windows, pace badge, forecast line, "yesterday" cost).
NOT verified: the menu bar title and tray menu lines (the screenshot tool captures only the window), notifications (macOS needs a bundled app with permission; a `target/debug` build may not show any), the hooks with a live Claude Code session (not installed into the real settings), the rings view, the Notices pane, light mode and English.
Caveat found: the user's real `~/.claude/settings.json` currently routes the status line through the debug build and a scratchpad data directory (switched on from the demo app). It must be switched off before those paths disappear.
