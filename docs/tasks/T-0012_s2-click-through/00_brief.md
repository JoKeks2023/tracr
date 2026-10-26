# T-0012 Click-through of S2 and the desktop app (TRACR-18)

Goal: look at every S2 feature in the real app, with someone clicking. Results go into `05_validation_report.md`; what turns out wrong becomes a GitHub issue. Nothing here proves a feature on its own: "I clicked it and saw X" is the evidence.

## Setup (a throwaway ledger of invented data, never your real one)

```sh
cd ~/Projects/tracr
cargo run -q -p tracr-app --example demo_day -- ~/tracr-demo          # about 30 s, writes ~/tracr-demo
open -n --env TRACR_DATA_DIR=$HOME/tracr-demo/data /Applications/tracr.app
```

- The app shows Mon 28 Sep 2026. A yellow banner may offer your real `~/.claude` and `~/.codex` folders: click **Not now**. **Never** click Approve here, that would read your real history into the demo ledger.
- Reset: quit tracr, `rm -rf ~/tracr-demo`, run the first command again.

## Expected start (day view)
5h 22m counted human time · 2h 10m unknown · 4 agent sessions · 7h 36m main-chain execution · 39 human prompts · about 573k output tokens · ≥ $23.98 est. API cost. Counted by repository: acme/web 3h 24m, side/notes 1h, acme/api 58m.

## Checks
| # | Do | Expect |
|---|---|---|
| 1 | Hover "≥ $23.98" | Tooltip: "API-equivalent estimate, not spend", catalog 2026-10-24.1; no number is called spend |
| 2 | Click a session name (e.g. `web`) | Sheet with segments, tokens per model, **Est. cost**, and the provider's own total labelled "not spend" |
| 3 | Click a hatched (unknown) block in **Human time** | Interval sheet "Does this time count?" |
| 4 | **Count as work**, then look at the timeline and the "counted" number | Block turns green, counted goes up, unknown goes down by the same amount |
| 5 | Same block: **Undo** | Back to hatched, numbers back |
| 6 | **Mark as break** on another unknown block | Block shows as break (excluded), counted unchanged |
| 7 | "What was it for?": pick a target, then **Other…** with a short label, **Set** | "Counted by repository" changes accordingly |
| 8 | Header **Add time**: set From/To, a target and a note, **Add** | A block with your entry appears; clicking it shows **Edit or delete** and Save/Delete work |
| 9 | **Start timer**, wait a minute, **Stop** | Menu bar shows the running time; stopping adds an entry |
| 10 | **N to review** | A list of unknown blocks you can go through |
| 11 | **Review day**: set intervals to Counted/Break/Unknown, **Review** | Header says "Reviewed r1"; the day's numbers are the reviewed ones |
| 12 | After the review, **Add time** again | Banner "New evidence differs from your review" with **Accept as r2…** and **Keep r1** |
| 13 | Gear (or ⌘,): Settings | Folders as `~/…` or full path, **Add folder…** opens the macOS panel, Time zone dropdown is **disabled** after step 11 and says why, Collector "Collecting", version |
| 14 | Menu bar icon | Today line, collector state, Open tracr, Start/Stop timer, Quit |
| 15 | Close the window, reopen from the menu bar | The window comes back; collecting never stopped |
| 16 | **Export CSV** → timesheet and agent sessions | A file per kind; numbers match the screen |

## What to note
Anything that is slow, unclear, wrongly worded, ugly, or that you did not expect. One line is enough; I turn it into issues.
