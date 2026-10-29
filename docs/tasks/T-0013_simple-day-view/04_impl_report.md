# T-0013 implementation report

## What changed (all in `apps/desktop/ui`, the model and the numbers are untouched)
- **i18n** (`i18n.ts`): English and German, chosen by the system language, `t(key, vars)`; one key set for both (the German table is typed `Record<Key, string>`, so a missing translation does not compile). Dates follow the locale.
- **Header** (`App.tsx`): `‹ date ›` day navigation, a small status line (collector dot, reviewed/not reviewed), timer, add time, a **More** menu (go to day, export, refresh) and the settings gear. Down from nine controls to five. Messages are a **toast** (`Toast.tsx`), so a message can no longer push the header (fixes #7).
- **Today card** (`Overview.tsx`): the big counted-time number with "review day" beside it; **by project** (or by repository while no project exists) as bars, a project expands into its repositories; one **unclear time** card with a "Resolve" button; the estimated cost as one quiet line with the full explanation in a tooltip.
- **Details** (collapsible, remembered): the secondary numbers (sessions, agent execution, prompts, output tokens), the timeline and agent lanes, legend and quality footer, as before.
- Everything user-facing in the header, card, banners, settings, projects, timer, roots, review list, review day, entry, export, interval, session, timeline legend and quality footer is translated. The review list reason now wraps instead of being cut off (#8).

## Evidence
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `pnpm typecheck`, `cargo test --workspace`: green (34 suites; no Rust change in this task).
- In the real window (debug build, invented demo ledger, German system language): the card with big number, bars, the unclear-time card and the cost line; the "More" menu; Details opening with the facts row, the timeline and the quality footer; the "Resolve" list ("Tag durchgehen", "Arbeit"/"Pause", "Gezählt · abgeleitet", reasons wrapped). Header, card and Details line up in one centered column.

## Not verified
- **English** was not looked at in the window (the system language is German here); the English texts are the previous ones, but wording changes (e.g. "counted · not reviewed yet", "Resolve") are only typechecked.
- Clicking through the other sheets in German (interval sheet, add time, export, session sheet) after the translation: typechecked, not clicked. Their layout may need small width fixes where German is longer.
- Timeline hover texts (segment and lane tooltips) and some internal reason codes still show English.
- The first-run screen and the settings in German were not re-checked after the card change.
- No project mapping exercised in the card (step A is its own PR).
