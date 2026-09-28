# Feature comparison

Research date: **2026-09-28**. This matrix compares inspected scopes, not every extension a tool might support. **C** = source inspected; **D** = official documented; **R** = maintainer claim; **?** = not verified; **—** = not part of the demonstrated path. “Multi-session” does not mean overlap-correct human time. Sources per tool: [tools.md](tools.md), [SOURCES](SOURCES.md).

| Tool | Claude | Codex | Git | GitHub | Huly | Sessions | Idle | Multi-session | Tokens | Costs | Manual time | Automatic time | Reports | Streaks | Contribution graph | Local-first | Desktop |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Timelog [T1](SOURCES.md#t1--claude-code-timelog) | C hooks/logs | — | C root/branch | Ticket regex, not API | — | C IDs | C gap heuristic | C independent slices; overlap remains | — | — | — | C estimate | C timesheets | ? | ? | C local JSONL | CLI/plugin |
| Native Claudoscope [T2](SOURCES.md#t2--claudoscope-native-macos) | C | ? | Metadata; live Git scope ? | ? | — | C parser/cache | Human idle ? | Multiple histories; arbitration ? | C | C estimates | — | Human time ? | R analytics | ? | ? | R local; updates network | Swift macOS |
| ccusage [T3](SOURCES.md#t3--ccusage) | R/C engine | C parser | Metadata, not full Git tracker | — | — | C | — | Aggregates usage | C | C modes/estimates | — | — | R daily/week/session | ? | ? | Local-source CLI | CLI |
| Multi-agent Claudescope [T4](SOURCES.md#t4--claudescope-multi-agent-explorer) | C contract | C contract | Context metadata | C optional PR projection; API ? | — | C | — | C provider seam | C canonical fields | C central derivation contract | — | — | R analytics/search | ? | R heatmap plan; behavior ? | R local | Local web |
| claude-usage [T5](SOURCES.md#t5--claude-usage) | C | — | C recorded branch | — | — | C, subagents | — | C stored histories | C | Calculation behavior ? | — | — | Dashboard/CLI tree | ? | ? | C SQLite | Web/VS Code |
| Python Claude Monitor [T6](SOURCES.md#t6--claude-monitor) | C | — | Project metadata | — | — | Usage sessions/blocks | — | Aggregation, arbitration ? | C | C pricing delegation | — | — | R live/daily/monthly | — | — | Local reader; overlay privacy ? | Terminal |
| qianhaoq codex-usage [T7](SOURCES.md#t7--codex-usage-sqlite-reader) | — | C | CWD only | — | — | C DB threads | — | Lifetime totals | C combined total | C assumed blend | — | — | R periods; creation-date caveat | — | — | C read-only local DB | CLI |
| zJay26 codex-usage [T8](SOURCES.md#t8--recently-createdadditional-candidates) | — | R | R project context | ? | ? | R | ? | R histories | R/C estimator input | C coverage/scaled money | ? | ? | R analytics | ? | ? | R | Go CLI/local server; OS adapters |
| ActivityWatch [A1](SOURCES.md#a1--activitywatch) | Custom watcher needed | Custom watcher needed | Extension-dependent | ? | ? | Watcher buckets | C OS AFK | Separate sources; custom correlation needed | — | — | ? | C heartbeats | D local dashboard | ? | ? | D local server | Desktop collection/local web |
| WakaTime [A2](SOURCES.md#a2--wakatime) | Agent integration unverified | Agent integration unverified | Editor/project metadata | Integration scope ? | ? | Coding durations | D timeout | Cross-agent arbitration ? | — | — | ? | D editor heartbeat | D dashboard | ? | ? | Hosted, offline buffer | Editor integrations |
| Timing [M1](SOURCES.md#m1--professional-time-entry-semantics) | — | — | ? | ? | ? | App activity | Exact algorithm ? | Manual-over-auto documented | — | — | D | D app usage | Not audited | ? | ? | Full sync/privacy not audited | macOS product |
| Kimai [M1](SOURCES.md#m1--professional-time-entry-semantics) | — | — | ? | ? | ? | Timesheet records | — | Rules/configuration not audited | — | Rates/billable concept | D | Agent detection — | D timesheets/export | ? | ? | Deployment-dependent | Web; no native claim |

## Interpretation

The strongest demonstrated capabilities occupy different layers: parsers/accounting, activity sensing, or editable time entries. Combining tools' checkmarks would not automatically solve their identity, interval and billing-semantic differences. No inspected implementation demonstrates the complete requested GitHub↔Huly↔Claude/Codex↔manual-work reconciliation.

Unknowns should stay unknown. A stored branch string is not Git integration breadth; a PR URL parsed from a transcript is not synchronized GitHub state; a session list is not human idle detection. A heatmap plan file is not verified shipped behavior. A terminal application running on a desktop OS is not a menu-bar/tray product.

## Areas where further comparison is worthwhile

- Run a shared sanitized log corpus through the accounting tools and compare duplicate/fork/model handling.
- Validate ActivityWatch's platform sensor permissions on target OS versions.
- Investigate zJay26's ingest/dedup engine after the promising estimator review.
- Inspect douglasmonsky's provenance/MCP implementation before judging it against tracr's diary scope.
- Obtain working Toggl API/help sources before making a commercial-feature claim; current fetch failures are recorded in SOURCES.

These are explicit validation gaps, not a reason to replace the deeply inspected tools with a larger unverified catalogue.
