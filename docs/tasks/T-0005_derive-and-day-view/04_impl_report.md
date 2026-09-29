# Implementation report

## Milestone 1: S1.3 derivation (2026-09-29)

| Part | Where |
|---|---|
| Normalized event vocabulary (`EventKind`, `TurnOrigin`, `ActorRef`), `Normalizer` trait, usage candidates | `crates/tracr-core/src/normalize.rs` |
| Copy election: containment → file creation time (else lowest `seq`) → stream key | `crates/tracr-core/src/copy.rs` |
| Agent timelines: actors, turns, tool calls, execution segments, engagement instants | `crates/tracr-core/src/agent.rs` |
| Deterministic derived IDs (BLAKE3-128, length-prefixed parts) | `crates/tracr-core/src/ids.rs` (`derived_id`) |
| Human-prompt rule decided from retained markers; used by the extractor and the normalizer | `crates/tracr-provider-claude/src/rules.rs` |
| Claude normalizer: events, automated/agent turn starts, last-record usage election per `(requestId, message.id)` | `crates/tracr-provider-claude/src/normalize.rs` |
| `derived.sqlite`: schema v1, build `derived.next.sqlite` then rename, read-only open with version check, canonical dump | `crates/tracr-store/schema/derived.sql`, `crates/tracr-store/src/derived.rs` |
| Rebuild orchestration | `crates/tracr-app/src/derive.rs` |
| Day queries (reporting zone, DST-correct bounds, per-session DTOs, pending lanes) | `crates/tracr-app/src/day.rs` |
| CLI `rebuild`, `days`, `sessions [--day] [--json]` | `apps/cli/src/main.rs` |

### Decisions made during implementation (conservative, documented)

1. **Automated turn starts.** In the real history, 206 `task-notification` inputs on the main chain start agent work without a human.
   - With only human prompts starting turns, one turn would stretch across hours.
   - Such inputs therefore start a turn with origin `automated`. Scheduled inputs, and human-shaped prompts from a non-interactive entrypoint, are treated the same way.
   - A subagent's own task prompt starts a turn with origin `agent`.
   - Only `human` turns come with an engagement instant. The Claude natural key "turn = uuid of the starting record" is kept.
2. **Quiet-gap split, rule `agent-segment@2`.** A segment ends at its last record when the actor is silent for more than 10 minutes; the turn continues.
   - Real data: `AskUserQuestion` waits of 3,005 and 1,296 minutes; a `Read` permission prompt answered after 922 minutes.
   - Without the split, one session showed 1,150 execution minutes in one day; with it, 98.
   - The parameter is in `rule_ref` (`agent-segment@2#quiet_gap_max_ms=600000`) and is provisional (AD-13). **Open for user confirmation.**
3. **Within-stream record election.** One observation per record key: the first in file order. This covers the 56 re-appended changed lines found in T-0004. Later versions are counted, not used.
4. **Usage semantics.** The elected record states the request's usage, so it is stored as one `delta` per request. The rule name is `claude.last_record_per_request_message@1`.
5. **Session context and label.** The cwd and branch come from the earliest canonical record that states a cwd. The label is the cwd's last path component.
6. **Reporting zone.** The `timezone` setting if present, else the system zone (AD-14). Nothing is written to settings automatically.
7. **Normalized events not emitted in S1.3:**
   - `usage_reported`: usage lives in `usage_record` with source links;
   - `session_started`;
   - `agent_started` / `agent_stopped`: actors are derived from streams;
   - `turn_completed`: Claude's `system/turn_duration` would fit, but it is not fixture-validated as a completion marker.

### Findings

- **Copies are common.** 6,136 copied records across 8 main-session pairs (resume or fork; only 1 is plain containment) and 26 subagent pairs in the same session.
- **Election effect on output tokens:**
  - naive sum over every usage-bearing record: 37.56 M;
  - elected per request: 17.40 M;
  - canonical, without copies: 15.46 M.

  A naive counter would overstate output by 2.4×.
- Some subagent streams start with one agent record and its tool result before the subagent's task prompt, a few seconds earlier. They form a short turnless segment (`bounded` start). This is correct under the rule and left as is.
- **Rebuild resources:** 6–9 s on the real history, 435 MB peak RSS. The whole fact set is held in memory; streaming by session would lower this if needed.

## Milestone 2: Day view shell (2026-09-30)

| Part | Where |
|---|---|
| Tauri 2 host: commands `status`, `days`, `day_agents`, `refresh` (import + rebuild under the writer lock); capability `core:default` only; strict production CSP | `apps/desktop/src-tauri/` (workspace member `tracr-desktop`) |
| React + TypeScript UI (Vite): day dropdown, summary metrics, time axis, pending lanes, one lane per session with actor rows, collapsed subagent overlay, session detail sheet | `apps/desktop/ui/src/` |
| CI: Linux and Windows exclude `tracr-desktop`; macOS builds the UI and then the whole workspace | `.github/workflows/ci.yml` |

### Decisions made during implementation

- **Visual language:**
  - the user's design-language skill;
  - dark-first on true black, with light mode through `prefers-color-scheme`;
  - accent: lime;
  - content sits directly on the canvas; session detail opens in a sheet, not in cards.
- **Pending lanes** (human, device, coverage) are labeled tracks reading "Not derived yet". They are not hatched, because hatching will mean *unknown* in S1.4. Cost is noted in the session sheet.
- **Headline execution** is split into main chain and subagents. One summed "88h" figure read like a claim about the day.
- **Refresh button:** the app runs import + rebuild. This matches AD-03, where the app process hosts the collector. It is a small addition that goes beyond a strictly read-only view, and it is named here explicitly.
- **Not built yet:**
  - the menu bar item (§13.4), which needs S1.4 minutes;
  - the usage/cost table as its own screen section; tokens are in the session sheet.
