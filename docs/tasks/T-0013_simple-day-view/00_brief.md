# T-0013 A simpler day view

Problem (user, 2026-10-26): the app is too complicated and unclear. Today the day shows eight numbers, nine controls in the header, three lanes plus one lane per agent session, and a lot of jargon ("main chain · actor unknown", "inferred, provisional", "subagent execution").

## Decisions (user)
- **Structure: a "today card" with Details.** Always visible: counted time of the day, the split by project (or repository), one call to action for unclear time, the estimated API cost (small, "not spend"). The timeline, the agent sessions and the secondary numbers sit behind **Details** and open on demand. Nothing is removed.
- **Language: German and English by system language**, like the website.

## Design rules
1. One number first: the counted time. Everything else is smaller.
2. At most five things in the header: day navigation, timer, add time, a "more" menu (review, export, refresh, choose day), settings.
3. Unclear time is a **task** ("1h 51m unclear · Resolve"), not a statistic.
4. Messages ("Counted as work", "Target set") are a toast at the bottom, never part of the header (fixes #7).
5. Plain words first, the precise term in a tooltip. "Counted" and "unclear" instead of "inferred, provisional" and "unknown".
6. Detail is one click away and is the same detail as before; the model and the numbers do not change.

## Steps
1. i18n foundation (`t()`, English and German, system language) and the strings of the files touched here.
2. New header, toast, today card, Details.
3. The remaining sheets in both languages (Interval, Entry, Review, Session, Export, Timeline legends, Quality footer).
