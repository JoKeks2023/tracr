# T-0012 validation report

Run on 2026-10-26 by Claude, driving the real desktop binary (debug build of branch `feat/timezone-setting`, Vite dev server) with synthetic mouse, wheel and key events and screenshots, against a throwaway ledger of invented data (`demo_day`, never a real ledger). "Pass" means: done in the real window and the result was seen on screen. What a person must still judge or click is listed at the end.

| # | Check | Result | What was seen |
|---|---|---|---|
| 1 | Cost tooltip | Pass | "API-equivalent estimate, not spend", catalog 2026-10-24.1 as of 2026-10-01, "Unpriced: cache write not reported", "Assumed: … priced as global / standard" |
| 2 | Session sheet | Pass | Provider, branch, evidence span, prompts, execution 4h 43m, tokens per model, Est. cost $14.65 with the not-spend explanation, provider's own total $15.90 labelled "not spend". Finding: full working-dir path (#8) |
| 3 | Unknown block → sheet | Pass | "11:18–12:20 · 1h 2m · Unknown · prompts in several sessions interleave", Count as work / Mark as break |
| 4 | Count as work | Pass | counted 5h 22m → 6h 24m, unknown 2h 10m → 1h 8m, block green; the hour shows as "unattributed 1h 2m". Finding: notice pushes Refresh out of the window (#7) |
| 5 | Undo | Pass | back to 5h 22m / 2h 10m, "Undone"; the sheet had shown "Decided by your correction. Underneath, tracr derived: unknown (conflicting evidence)" and the correction as "effective" |
| 6 | Mark as break | Pass | counted unchanged, unknown −19 m (1h 51m), break pattern in the block, "Marked as break" |
| 7 | Target | Pass (list entry); not tried: "Other…" with a free label | "Target set": acme/web 3h 24m → 4h 26m, "unattributed" gone, counted total unchanged |
| 8 | Add time / edit / delete | Pass | 17:00–18:00 with a note added only +26 m (counted until 17:34 already: time is counted once); "Edit entry" with Delete/Cancel/Save; Delete gave "Entry deleted" and back to 5h 22m (no confirmation) |
| 9 | Timer | Pass | Start gave "Timer started" and a running 0:01:07 with Stop; Stop gave "Timer stopped"; menu-bar line "Today: 1m counted" |
| 10 | N to review | Pass | "Review the day · 7 unknown windows · 1h 51m not counted", Work/Break per row, counted windows below |
| 11 | Review day | Pass | sheet with counted per repository, unknown, break and the "unknown stays unknown" note; header "Reviewed r1 · 26 Oct at 18:10", "counted human time · reviewed r1" |
| 12 | Discrepancy | Pass | after Add time: "New evidence differs from your review (r1): counted +26m, unknown ±0m, break ±0m", Accept as r2… / Keep r1, two lanes "Human time · now" and "Reviewed r1" |
| 13 | Settings | Pass | gear opens it; folders, time zone, collector, data, about; after the review the zone note appears ("1 reviewed day keep their zone…", grammar fixed in the same branch). Not seen: Add folder… (native panel) |
| 14 | Menu bar | Pass | Today line, Collecting, Open tracr, Start timer, Quit tracr |
| 15 | Close and reopen | Pass | closing hides the window and the process keeps running; "Open tracr" brings it back with its state |
| 16 | Export CSV | Partly | the sheet (What, From, To, the "never one total" note) was seen; the macOS save panel behind "Export…" was not driven, so no file was written or compared. Finding: From/To text truncated (#8) |

## Still for a person
- Export…: pick a folder in the save panel and compare the CSV with the screen.
- Settings → Add folder…: the macOS folder panel.
- Target "Other…" with a free label, and Accept as r2.
- Whether it feels right: wording, density, the amount of text per sheet. I can say what is on screen, not how it feels.

## Issues filed
- #7 header overflows, Refresh clipped while a notice is shown.
- #8 small polish: truncated texts, full paths, banner wrapping.
