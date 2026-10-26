# T-0011 implementation report

## What changed
- `tracr-app`: `App::remove_root` (a `setting.<key>.remove` user action plus the new list, same append-only path as `add_root`), `Reader::repository_roots`, `RootsChange::{Removed, NotApproved}`.
- CLI: `tracr roots remove --provider|--repo <path>` and `tracr roots list`.
- Desktop backend: `approve_roots` (several paths, one collector restart), `choose_root` (native folder panel, then approve), `remove_root`; `status` now carries `approved_repository_roots`, `home` and `app_version`. Proposal-vs-approved is compared on the resolved path too (`is_approved`).
- Desktop UI: `RootsPicker` (checklist), `SettingsSheet` (gear icon in the header, Cmd-,), compact banner ("Approve all (n)", "Choose…", "Not now"), first-run screen with the checklist and "Choose another folder…", `~/…` display via `tildify`.

## Evidence
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `pnpm typecheck`, `cargo test --workspace`: all green (33 suites).
- New tests: `crates/tracr-app/tests/roots.rs` (approve, withdraw, history stays, a folder gone from disk can still be withdrawn) and `is_approved` in the desktop crate (a symlinked proposal counts as approved).
- Run in the real desktop binary (debug build, Vite dev server) against a throwaway data directory and a fake `$HOME` whose proposals point at invented demo data, never against a real ledger:
  - first run shows the checklist (three folders, all selected, `~/…` paths); one click on "Approve (3)" approved all three and the day loaded (4 sessions);
  - before the symlink fix the banner still offered "Approve all (2)" afterwards, after the fix it is gone;
  - Settings opens from the gear; "Remove" asks inline; "Stop reading" removed the folder, the header said "No longer reading ~/.codex/archived_sessions", the banner and "Found on this Mac" offered it again;
  - the ledger holds 3 `setting.approved_provider_roots.add` and 1 `.remove` user actions; `tracr roots list` shows the remaining two.

## Not verified
- **"Add folder…"** (the native folder panel) was not clicked: the panel cannot be driven from here. The Rust path is the same one `export_everything` already uses plus `add_root`, which is tested.
- **⌘, shortcut** was not pressed.
- **Repository folders** section: only the empty state was seen.
- The release app was not rebuilt; this is on a branch and not released.

## Addendum: reporting time zone (small items, 2026-10-26)
- AD-14 says the reporting zone is one fixed home zone chosen at setup. Settings now has a "Time zone" section: a dropdown (System, then every IANA zone the web view knows). `App::set_timezone` validates the name, appends a `setting.timezone.set` user action plus the setting (null = follow the system), and the desktop rebuilds the derived store because day boundaries move.
- **Locked after the first reviewed day** (default, to be confirmed): a reviewed revision records the zone it was cut in, and re-cutting reviewed days would create spurious "new evidence differs" banners. The dropdown is disabled and says why; `set_timezone` refuses with the number of reviewed days.
- Tests: `the_reporting_zone_can_be_chosen_until_a_day_is_reviewed` (unknown zone refused, change, back to system, refusal after a review, an unchanged value is not a change).
- Run in the real desktop binary on the invented demo ledger: chose Asia/Tokyo (timeline restarted at 16:00, notice shown, the demo day now spans two days), then System (one day again, `Europe/Berlin`); ledger shows two `setting.timezone.set` actions.
- Not verified: the locked state in the window (covered by the test only), and zones other than the two tried.
