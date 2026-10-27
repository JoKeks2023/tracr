# T-0015: tracr as an AI work tool (information architecture)

Status: brief, decided by the product owner on 2026-10-27. Supersedes the navigation of T-0014 (sidebar of days + settings).

## Why

tracr is not only a time tracker. It should help with AI work for real, including everything the native Claudoscope
(`cordwainersmith/claudoscope` v1.4) offers. The day list is one view among several, not the structure of the app.

## Decisions (product owner)

- **Diffs are in.** tracr shows what the agent changed per session, per file. Projects show their repository.
- **Hooks, skills, MCPs, agents, plugins, commands, memory, canon are in.** tracr reads them as an inventory with a linter.
- "tracr should be able to do all of it." Scope follows Claudoscope's feature list, ordered below.
- Defaults I chose (change if wrong): read-only toward `~/.claude/` (no hardening/routing installer); the window stays without shell/network.

## Data principle (replaces "never reads content")

- Diffs and chat are **read on demand from the approved provider files** and shown in the window. They are **not copied into the ledger** or the derived store, and never uploaded.
- The ledger keeps metadata only (time, tokens, models, tool names, hashes), as today.
- Consequence: the sentence in `README.md` ("tracr never stores prompt text, ... Searching or re-reading message content is therefore deliberately impossible") and the "metadata-only" claims on the site must be rewritten before release. Not done yet.
- Provider roots stay approval-gated (canon: "tracr is local-first"). New sources (`~/.claude/sessions/`, config files, hooks) need the same approval.
- Public repo rule stands: no real project names or content in docs and screenshots.

## Navigation (sidebar, by question)

1. **Heute**: time and agent activity side by side, alerts.
2. **Live** (fleet): running sessions, "needs you", working/parked/landed, time in state, spend per hour, cache time left, context gauge.
3. **Sitzungen**: list across providers; detail tabs Trace, Chat, Dateien (diffs), Kontext, Tools, Kosten.
4. **Arbeitszeit**: the current day view (review, resolve, export).
5. **Kosten & Limits**: analytics, cache (5 min / 1 h), models, what-if Opus to Sonnet, alerts, later share of limit (T-0010).
6. **Projekte**: time, sessions, cost, **repository**, branches, PRs; assignment suggestions (T-0009).
7. **Umgebung**: hooks (config + runtime), skills, MCPs, agents, plugins, commands, memory, canon, health linter.
8. **Einstellungen** (sidebar foot).

Global: time and project filter in the toolbar; command palette (⌘K); menu bar popup (`feat/menubar-popup`).
Shortcut note: ⌘K is "resolve open time" in the T-0014 menu; it moves to the palette.

## Order of work

1. Shell with the new navigation; Heute, Arbeitszeit (existing day view moves in); global time filter.
2. Sitzungen: list and detail with Trace, Kontext, Tools, Kosten (metadata only).
3. Dateien/diffs and Chat (on-demand content reading), Projekte with repository.
4. Live/fleet (`~/.claude/sessions/`), notifications via hooks, cost alerts, menu bar popup.
5. Kosten & Limits.
6. Umgebung with linter.

Each step is its own task/PR with a design review gate and a privacy check for every new source.

## Open points

- Search across messages (needs an index of content: would break "not stored"; decide separately, default: search only within an open session).
- Secret scanning of transcripts: not in scope until decided.
- Hardening / agent routing installer (writes `~/.claude/`): not in scope.
- Windows: reading `~/.claude/sessions/` and terminal focus are macOS-specific.
