# T-0015 step 1 implementation report

Backend: `crates/tracr-app/src/day.rs` — the per-session loop of `day_agents` is now `session_days(conn, start, end, revision)`;
new `SessionList` and `sessions(conn, zone, from, to)`; `Reader::sessions` in `lib.rs`; Tauri command `sessions` in `apps/desktop/src-tauri/src/main.rs`.
Menu: Darstellung = Heute ⌘1, Sitzungen ⌘2, Arbeitszeit ⌘3, Tagesübersicht ⌥⌘1, Agenten-Zeitleiste ⌥⌘2, Voriger/Nächster Zeitraum ⌘[ ⌘], Zu heute ⌘0, Befehle ⌘K. "Offene Zeit klären" moved to ⌥⌘K.
UI (`apps/desktop/ui/src`): `nav.ts` (areas, scope, period math), `Toolbar.tsx`, `Sidebar.tsx` (destinations), `Sessions.tsx`, `SessionRow.tsx`, `SessionDetail.tsx` (extracted from `SessionSheet.tsx`, which keeps the modal for the agent timeline), `Today.tsx`, `Palette.tsx`, `App.tsx` (area routing), CSS and i18n DE/EN.

Deviations from the plan (named):
- The period is not one global control: Sitzungen has day/week/month with its own anchor; Arbeitszeit keeps its own day (reviewing is per day). Today ignores both.
- Work-time commands (add time, review, resolve) switch to Arbeitszeit first, because they act on its day.
- Narrow window (<720 px): the sidebar becomes a top strip instead of disappearing.
- `App.tsx` is still large; the `useNav` hook from the plan was not extracted (state stays in `App.tsx`).

## Step 2: session views (Ablauf, Kontext, Werkzeuge)

Backend: `tracr-store/src/derived.rs` (`tool_call_times_of`, `usage_points_of`, `all_instants_of`), `tracr-app/src/trace.rs` (`SessionTrace`, `session_trace`), `Reader::session_trace`, Tauri command `session_trace`. Names, times and token counts only; no content is read.
UI: tabs in the Sitzungen detail (`Sessions.tsx`); `SessionInsights.tsx` has `TraceView` (lanes per actor, your prompts, tool ticks by kind, pauses over 5 min shortened), `ContextView` (request input size over the main chain, peak/latest/requests/cache share, drops marked as inferred), `ToolsView` (counts per tool).
Choices: no context percentage (the model's limit is not known to tracr); a size drop of more than half is marked "possible compaction, inferred", because compaction markers are extracted but not stored in the derived store; tool failure is not shown (not recorded). The main chain has actor kind `unknown` with no agent id; every non-`known` actor is shown as the main chain.
Test: `tests/sessions.rs::trace_lays_out_prompts_tools_and_request_sizes_without_content`.

## Start page (Heute)

Reworked `Today.tsx` after a screenshot showed "0m tracked" as the hero while 4 h were unresolved, identical session titles, and a three-lane sensor timeline on the start page.
Order: facts strip (Erfasst, Ungeklärt, Agenten, Geschätzte Kosten; same weight, unresolved in warning colour), "Braucht dich" row with the action, "Dein Tag" strip (you: tracked/unresolved; agents: when they ran), sessions today with provider, run span and repo·branch, projects only when time is tracked, cost line. The full timeline with device and coverage lanes stays under Arbeitszeit → Agenten-Zeitleiste.
Also: no "no activity" flash while loading; "1 Abschnitt" singular; unpriced shows "—" instead of "ohne Preis" in every row; a session's span is its run time in the shown period (date added when it crosses midnight); provider names as words.

## Rust parts before the UI shell decision (prices, limits, projects)

- **Prices** (`pricing/catalog.toml`, catalog 2026-10-27.1): `claude-sonnet-5-5` (2 / 2.50 / 4 / 0.10 / 10 USD per MTok), `claude-haiku-5-5` (up to 100K prompt only: 0.10 / 0.125 / 0.20 / 0.01 / 0.50), `gpt-6.1-sol` (standard short context: 2 / 0.10 / 10), from the official pages fetched 2026-10-27. Not modeled: Haiku 5.5 above 100K, gpt-6.1-sol above 272K, fast/batch. `codex-auto-review` is not on the OpenAI page and stays unpriced. Effect on the demo ledger after the automatic rebuild: unpriced cost estimates fell from about 3,700 to 569; the day's Claude Code sessions now show an estimate, Codex stays a floor (cache writes are only a marker).
- **Limits** (`crates/tracr-app/src/limits.rs`): provider-reported snapshots, a separate kind from money, never attributed to a session. Codex: the last `rate_limits` of the newest rollout under the approved provider roots, read on demand (percent, window minutes, reset, plan only). Claude Code: opt-in `statusLine` tap (`tracr statusline-tap`, hidden) that stores only the percentages and reset times in `<data>/limits/claude_statusline.jsonl`, writes a line only when a figure changed, and runs the previous statusLine command so the user sees no difference. Install copies `settings.json` to `<data>/backups/` first, keeps key order and other keys, is idempotent, and `uninstall` restores the previous statusLine. CLI: `tracr limits show|install|uninstall`; Tauri: `limits`, `limits_tap`.
- **Projects** (`projects::overview`, `Reader::project_overview`, Tauri `project_overview`): sessions of a range grouped by project, else by repository, else `none`; defined projects always appear; with branches, providers, agent time, estimated cost.
- Tests: `tests/limits.rs` (5), `tests/projects.rs::overview_*`, `tests/sessions.rs`; cost tests updated to the new catalog version.

## design/v2: the new interface (Tauri + React, dense workbench)

Decided with the product owner on 2026-10-27: Tauri + React again, new design from zero, dense like Claudoscope, Heute as the "A" board. Mock-ups (invented data) were reviewed first; the chosen ones are the source of this build.
Built on branch `design/v2`:
- Shell: icon rail (Heute, Projekte, Zeit, Limits, Einstellungen), top bar per area, theme in `ui/src/v2/theme.css` scoped under `.v2` (legacy sheets and the work-time pieces keep working; their gray tokens are remapped). New class names avoid the legacy ones.
- **Heute** (`v2/Home.tsx`): stats strip, quota panel (provider-reported), "Braucht dich" (unresolved time, reached limits, Claude tap not set up), board of sessions (Aktiv = evidence in the last 15 min; Früher heute), day strip.
- **Projekte** (`v2/Projects.tsx`): list of projects, else repositories, else working folders; the project's sessions as a table; a session opens as a full page with Übersicht, Ablauf, Kontext, Werkzeuge. Sessions are no longer an area of their own.
- **Limits** (`v2/LimitsPage.tsx`): quota cards, the opt-in switch for the Claude statusLine tap, cost per model (estimates).
- **Zeit**: the former work-time view, unchanged in content, in the new shell.
- Backend: `projects::overview` now groups sessions without a repository by working folder (`folder` entries) so the page is useful before any repository root is approved.
Deviations from the mock-ups (named): no "Wartet auf dich" column and no "Verbrauch jetzt" (both need the live source, not built); no sparklines in the project list; the session detail is a full page instead of a side inspector (the trace needs width); no cost-per-day chart, cache panel, alarms or Opus→Sonnet what-if on the Limits page yet; project tabs Zeit, Kosten, Dateien are not there.
