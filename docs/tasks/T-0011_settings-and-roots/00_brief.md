# T-0011 Settings and approving several folders

Issue: #4 (also fixes #3). Branch: `feat/settings-and-multi-approve`.

## Problem
- Provider folders could be approved only one at a time, each with its own button; with three proposals the buttons overlapped (#3).
- No settings at all: no way to see, add or withdraw approved folders, repository roots, data location or version.

## Decisions (defaults, labelled)
- **Approve several at once:** a checklist with all proposals selected and one button. Used on first run, in the banner ("Approve all (n)", "Choose…") and in Settings.
- **Withdrawing a root is allowed and never deletes history.** The ledger is append-only, so a removed root only stops future discovery. The window asks first and says so.
- **Folders outside the proposals** are added through the native folder panel (the window has no filesystem access, AD-12), not a free-text field.
- **Settings contents (v1):** provider folders, repository folders, collector state and sensor, data location, version and license. Not in v1: time zone (changing it moves day boundaries and needs a rebuild), launch at login, appearance.
- **Paths show as `~/…`**, so screenshots do not carry the user name.
