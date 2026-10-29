# Sources and evidence register

Research conducted **2026-09-27–2026-09-28**. Last checked **2026-09-28**. Public documentation is a moving target; pinned source URLs below identify the exact inspected implementation, not a promise of compatibility with every installed release. No third-party application was installed or runtime-tested. Source inspection means the listed implementation or selected relevant functions were read; it does not mean a whole-repository audit.

Evidence labels used throughout: **D** official documentation; **C** inspected source; **L** live workspace/tool observation; **R** repository-maintainer claim; **P** tracr proposal/inference; **U** unresolved. A missing capability in an inspected path is not proof that no extension anywhere implements it.

## CONTEXT

- Local `git ls-tree -r --name-only HEAD`, file inventory, `git log`, `git status`, and complete `README.md` read: only `README.md`, containing `# tracr`; initial commit `7fc322648c6574f1062c0ff3553aaff14c1ed00b`. No local CLAUDE.md, AGENTS.md, MCP config, imported docs, research, or architecture files at inspection time.
- GitHub connector, account JoKeks2023: [repository contents](https://api.github.com/repos/JoKeks2023/tracr/contents) and [branches](https://api.github.com/repos/JoKeks2023/tracr/branches) confirmed the same single-file `main` on September 27. Local inventory rechecked September 28.
- User-supplied global AGENTS instructions and research brief in this chat are the available project requirements. No instructions were replaced. Parent instruction-file checks found no applicable files in the checked home/Projects paths. Unrelated repositories' instructions were not adopted.
- Memory registry search for tracr/Huly/JoKeks found no relevant entries; no remembered project facts are used.

## HULY-LIVE

Live `mcp__huly_joris` tool catalog and calls on September 28:

- `list_workspaces({})`: two workspaces (the project's own and an unrelated one).
- `list_projects({workspace:"jokeks2023"})`: TRACR and TSK.
- `get_project({workspace:"jokeks2023",identifier:"TRACR"})`: tracr, zero issues; Backlog, Todo, In Progress, Done, Canceled.
- `list_issues({workspace:"jokeks2023",projectIdentifier:"TRACR",limit:100})`: no issues.
- `list_teamspaces({workspace:"jokeks2023"})`: no teamspaces.
- Tool schemas now accept explicit workspace selection; mutation schemas require it. `log_time` exposes workspace, identifier, hours, optional description. Full capability inventory and unverified return semantics: [huly.md](huly.md).

This is direct tool evidence, not a public URL. Earlier September 27 observations of SERV/PLAY/TSK belonged to the other workspace and are **not** tracr findings. No write probe, fake time entry, or credential inspection was performed. Fork repository/revision and backend implementation remain unidentified.

## T1 — Claude Code Timelog

Repository [RemoteCTO/claude-code-timelog](https://github.com/RemoteCTO/claude-code-timelog). Inspected commit `063f0fd67d60a42c7e65e8428c1334c8e2e7352c`.

- [scripts/report.mjs](https://github.com/RemoteCTO/claude-code-timelog/blob/063f0fd67d60a42c7e65e8428c1334c8e2e7352c/scripts/report.mjs): `buildSlices`, `aggregate`, `dateKey`, CLI break threshold.
- [hooks/timelog.mjs](https://github.com/RemoteCTO/claude-code-timelog/blob/063f0fd67d60a42c7e65e8428c1334c8e2e7352c/hooks/timelog.mjs): Git/CWD detection, 16-KiB transcript tail, branch ticket matching.
- README inspected for hook events, backfill and reports; backfill implementation was not deeply audited.
- GitHub metadata: created 2026-02-12, pushed 2026-02-13. Metadata dates are not release dates.

## T2 — Claudoscope, native macOS

Repository [cordwainersmith/claudoscope](https://github.com/cordwainersmith/claudoscope). Commit `812a54296c7b28ec3e2c22a7cf1e709ef3e2993d`.

- [SessionParser.swift](https://github.com/cordwainersmith/claudoscope/blob/812a54296c7b28ec3e2c22a7cf1e709ef3e2993d/Claudoscope/Services/SessionParser.swift): billing-record election, message-ID/UUID deduplication, parent-context exclusion, cache TTL breakdown.
- [ClaudeFileWatcher.swift](https://github.com/cordwainersmith/claudoscope/blob/812a54296c7b28ec3e2c22a7cf1e709ef3e2993d/Claudoscope/Services/ClaudeFileWatcher.swift): FSEvents, 300-ms debounce, dropped-event rescan, atomic replacement handling.
- [SessionSummaryStore.swift](https://github.com/cordwainersmith/claudoscope/blob/812a54296c7b28ec3e2c22a7cf1e709ef3e2993d/Claudoscope/Services/Cache/SessionSummaryStore.swift): SQLite/GRDB derived cache, parser/pricing/timezone invalidation, parent fingerprint.
- README platform/privacy/MCP statements are maintainer claims, not network-trace verification. Created 2026-03-15; pushed 2026-09-17.

## T3 — ccusage

[ccusage/ccusage](https://github.com/ccusage/ccusage), commit `11f62f501345e8edd6ad90232cb1e7ba8a7f141a`.

- [Codex parser](https://github.com/ccusage/ccusage/blob/11f62f501345e8edd6ad90232cb1e7ba8a7f141a/rust/adapters/codex/src/parser.rs): cumulative-vs-last usage, saturating deltas, model fallback, service tiers, headless variants.
- [cost.rs](https://github.com/ccusage/ccusage/blob/11f62f501345e8edd6ad90232cb1e7ba8a7f141a/rust/crates/ccusage-core/src/cost.rs): auto/display/calculate modes, cache creation tiers, long-context rates.
- Recursive tree and README inspected: Rust core/adapters with npm-facing packaging and report documentation. Historical descriptions of an exclusively TypeScript implementation are not sufficient for this snapshot.
- Created 2025-05-29; repository still updated on research date. HEAD can move during research; all code findings use the pinned commit.

## T4 — ClaudeScope, multi-agent explorer

[vladar107/claudescope](https://github.com/vladar107/claudescope), commit `03e951709c5b97e385dec2efd5961f7c33e5e62d`.

- [connectors/types.ts](https://github.com/vladar107/claudescope/blob/03e951709c5b97e385dec2efd5961f7c33e5e62d/packages/server/src/connectors/types.ts): AgentConnector, discovery, preparation, SQL projection, session loading, auxiliary PR links.
- [connectors/canonical.ts](https://github.com/vladar107/claudescope/blob/03e951709c5b97e385dec2efd5961f7c33e5e62d/packages/server/src/connectors/canonical.ts): canonical DuckDB column contract, NDJSON projections, compaction rows.
- README and recursive tree inspected. TypeScript/local web explorer; created 2026-06-03, pushed 2026-09-24. Distinct from T2 and JoniMartin27/claudescope.

## T5 — claude-usage

[phuryn/claude-usage](https://github.com/phuryn/claude-usage), commit `3eea154474e93761f774ed38beeaf45baf838a45`.

- [scanner.py](https://github.com/phuryn/claude-usage/blob/3eea154474e93761f774ed38beeaf45baf838a45/scanner.py): sessions/turns/agents/processed_files/schema_meta tables, partial unique message-ID index, streaming record replacement, title precedence.
- Tree shows Python CLI/dashboard and TypeScript VS Code extension; their entire runtime behavior was not audited. Created 2026-04-07, pushed 2026-07-10.

## T6 — Claude Monitor

[Maciek-roboblog/Claude-Code-Usage-Monitor](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor), commit `c59a83bf943f329f0e61f1a29c760353ee1860a5`.

- [data/reader.py](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor/blob/c59a83bf943f329f0e61f1a29c760353ee1860a5/src/claude_monitor/data/reader.py): token extraction, message/request identity, model filtering, pricing delegation.
- README: Python terminal monitor, refresh controls, daily/monthly views, predictive plan limits. Those plan estimates are tool assumptions, not official entitlement contracts. Tree includes warehouse/official-overlay modules; their persistence/remote semantics are not established by the reader audit.

## T7 — codex-usage, SQLite reader

[qianhaoq/codex-usage](https://github.com/qianhaoq/codex-usage), commit `0c01df38fc8ae1e178ab6d9c5fc44941d797bb4b`.

- [db.py](https://github.com/qianhaoq/codex-usage/blob/0c01df38fc8ae1e178ab6d9c5fc44941d797bb4b/codex_usage/db.py): highest state_N.sqlite selection, read-only connection, schema checks, created_at filtering, archived exclusion, rollout rate-limit observation.
- [pricing.py](https://github.com/qianhaoq/codex-usage/blob/0c01df38fc8ae1e178ab6d9c5fc44941d797bb4b/codex_usage/pricing.py): assumed 70/30 input/output blend. Not a measured token-category split.
- Created/pushed 2026-04-06. README's state_5.sqlite description is narrower than implementation discovery.

## T8 — Recently created/additional candidates

- [zJay26/codex-usage](https://github.com/zJay26/codex-usage), commit `dc4e1a8d038385b33f1baeaf995422990cbcc710`: Go CLI/local analytics with platform-specific files; created **2026-07-31**, pushed September 27. README supports Windows/macOS/Linux. Tree and selected functions in [internal/pricing/estimator.go](https://github.com/zJay26/codex-usage/blob/dc4e1a8d038385b33f1baeaf995422990cbcc710/internal/pricing/estimator.go) inspected: integer nano-USD, overflow checks, rounded division and priced/unpriced coverage. Pricing catalogue completeness and end-to-end accounting remain unverified.
- [douglasmonsky/codex-usage-tracker](https://github.com/douglasmonsky/codex-usage-tracker), tree commit `43278d1408416c3262086028bdfa7a522cfc35f8`: Python, local dashboard/MCP and accounting provenance claims; created May 17, pushed August 20. Discovery-level inspection, not an implementation endorsement.
- [physcom/claude-usage-dashboard](https://github.com/physcom/claude-usage-dashboard): README claims SQLite daily aggregates survive source log deletion. Useful reporting comparison; retention behavior untested.
- [ClaudeCodeUsage/ClaudeCodeUsage](https://github.com/ClaudeCodeUsage/ClaudeCodeUsage): README distinguishes local observations/API-equivalent estimates and optional publication; discovery only.
- [JoniMartin27/claudescope](https://github.com/JoniMartin27/claudescope): separate CLI/local transcript-search project; discovery only.
- [rjwalters/claude-monitor](https://github.com/rjwalters/claude-monitor): distinct Swift monitor, README includes account/quota access; not the T6 Python tool. Authentication approach not recommended without a separate review.

## A1 — ActivityWatch

- [Architecture](https://docs.activitywatch.net/en/latest/architecture.html): watchers, local server/storage and frontend separation.
- [heartbeat_merge](https://github.com/ActivityWatch/aw-core/blob/b1f04b598706042a6590026f5c7e1a6476b50a61/aw_transform/heartbeats.py): inspected complete function; equal payload and bounded temporal adjacency, monotonic duration extension.
- [AFK watcher](https://github.com/ActivityWatch/aw-watcher-afk/blob/e906c0f8b762e20c21175b9b2679c653ffac3a86/aw_watcher_afk/afk.py): inspected initialization/polling and heartbeat construction; platform input-idle adapters.
- [macOS adapter](https://github.com/ActivityWatch/aw-watcher-afk/blob/e906c0f8b762e20c21175b9b2679c653ffac3a86/aw_watcher_afk/macos.py): CoreGraphics elapsed-input API; session-lock dictionary key is an implementation dependency, not established here as stable public API.

## A2 — WakaTime

[Official FAQ](https://wakatime.com/faq), sections accuracy, keystroke timeout, offline tracking. Editor heartbeats, two-minute recurring typing heartbeat, default 15-minute joining threshold, offline buffering. Hosted aggregation; no server algorithm source audit performed.

## M1 — Professional time entry semantics

- [Timing time entries](https://timingapp.com/help/time-entries): manual entries override automatic app activity for attribution.
- [Kimai timesheet](https://www.kimai.org/documentation/timesheet.html): begin/end, running entries, editing, billable flag, exported-record locking and permissions.
- Toggl help/API requests failed or returned no results in this environment. No Toggl-specific behavioral claim is relied on. Kimai audit-log page also failed; no claim of a particular audit implementation is made.

## C1 — Claude interfaces

- [Hooks reference](https://code.claude.com/docs/en/hooks): official event-specific input/output; volatile feature surface.
- [Status line](https://code.claude.com/docs/en/statusline): model, session, workspace, cost and context usage. Current context counts must not be mistaken for a lifetime usage ledger.
- [Monitoring/OpenTelemetry](https://code.claude.com/docs/en/monitoring-usage): usage counters, correlation attributes, active-time user/CLI categories, opt-in export.
- [Programmatic CLI](https://code.claude.com/docs/en/headless): print/JSON/stream-JSON operation.
- [Plugins](https://code.claude.com/docs/en/plugins): packaging surface, not an automatic observation channel.

## C2 — Codex interfaces

- [Hooks](https://learn.chatgpt.com/docs/hooks): common fields, interrupts, subagents, tool coverage and explicit transcript-instability warning.
- [App Server](https://learn.chatgpt.com/docs/app-server): threads, turns, items, usage notifications, requests/approvals. Original developers.openai.com URL redirected here.
- [Non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode): machine-readable execution.
- [OpenAI Codex source](https://github.com/openai/codex/tree/4f63088cce14065f57c152a0689d9cdf6cd88dea): inspected tree and selected hook matcher helpers in `codex-rs/hooks/src/events/common.rs`; rollout/protocol paths located but not comprehensively read. Internal-format conclusions rely on T3/T7 parser inspection as well, not a claimed complete Codex audit.

## G1 — Git and GitHub

- [Git worktree](https://git-scm.com/docs/git-worktree): shared repository versus per-worktree state.
- [REST limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api).
- [GraphQL limits](https://docs.github.com/en/graphql/overview/rate-limits-and-query-limits-for-the-graphql-api).
- [Webhook best practices](https://docs.github.com/en/webhooks/using-webhooks/best-practices-for-using-webhooks).
- [PR–issue linking](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue).

## G2 — Contributions

- [Profile overview](https://docs.github.com/en/account-and-profile/concepts/contributions-on-your-profile).
- [Detailed contribution reference](https://docs.github.com/en/account-and-profile/reference/profile-contributions-reference). Its event-specific timezone explanation is more nuanced than the overview's UTC summary.
- [GraphQL users reference](https://docs.github.com/en/graphql/reference/users), ContributionCalendar/ContributionCalendarDay. The old monolithic objects URL redirected to the split reference index.
- [W3C use of color](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html).

## H1 — Huly upstream

- [GitHub integration](https://docs.huly.io/integrations/github/): mapping and two-way synchronization.
- [API entry point](https://docs.huly.io/getting-started/api-tools/) and linked [platform API examples](https://github.com/hcengineering/huly-examples/tree/29b9f7f060d6308842d2897996776ef42374b5c6/platform-api). Example file inventory inspected; not an assertion of support in the custom MCP.
- [HR time tracking](https://docs.huly.io/team-resources/human-resources/): explicitly preliminary HR feature, distinct from issue worklogs.
- [Tracker types](https://github.com/hcengineering/platform/blob/f4f3d30bf1b62d766a6d6d2b481bdcdf6f24c37c/plugins/tracker/src/index.ts): inspected Issue and TimeSpendReport definitions; aggregate reportedTime versus individual reports, date, employee, person-hours and description.
- [GitHub issue sync base](https://github.com/hcengineering/platform/blob/f4f3d30bf1b62d766a6d6d2b481bdcdf6f24c37c/services/github/pod-github/src/sync/issueBase.ts): inspected selected DocSyncInfo lookup and GitHub mixin fields. Other sync files located, not comprehensively audited. Wrong `hulylabs/platform` lookup returned 404; canonical repository is hcengineering/platform.

## I1 — Implementation limitations and requests

- [ccusage #1288](https://github.com/ccusage/ccusage/issues/1288): June 11 report against v20.0.9–20.0.11 includes a concrete re-emission fixture. Read September 28; closed with resolved label. Reported real-world overcount percentages were not independently reproduced and are not applied to the current pinned parser.
- [ccusage #1811](https://github.com/ccusage/ccusage/issues/1811) and [closure comment](https://github.com/ccusage/ccusage/issues/1811#issuecomment-5865726485): September 28 request for timestamped normalized usage exports/stable library API was closed by automated contribution-gate triage as outside maintenance scope. Evidence of a requested compatibility surface, not a shipped feature or a personal maintainer commitment.
- [ccusage PR #1805](https://github.com/ccusage/ccusage/pull/1805): description reports compaction usage potentially absent from cumulative counters. Retrieved as closed, unmerged September 28; private-data observation and proposed fix, not an independently verified current contract.

## P1 — Pricing

- [Claude pricing](https://platform.claude.com/docs/en/about-claude/pricing).
- [OpenAI pricing](https://developers.openai.com/api/docs/pricing).
- Both checked September 28. This package intentionally does not turn today's rates into historical prices. No billing account/invoice was accessed. Tool formulas are separately evidenced under T2/T3/T7.

## S1 — Streaks

- [Loop StreakList.kt](https://github.com/iSoron/uhabits/blob/7e993e17b2b674d4b5b1291ebd18677b74810df2/uhabits-core/src/commonMain/kotlin/org/isoron/uhabits/core/models/StreakList.kt): inspected complete recompute/getBest functions.
- [Maintainer FAQ](https://github.com/iSoron/uhabits/discussions/689): skipped-day semantics and smoothed habit score.
- [Current-streak discussion](https://github.com/iSoron/uhabits/discussions/690): evidence of UX ambiguity, not an authoritative current implementation description.

## D1 — Desktop candidates

- Tauri: [overview](https://v2.tauri.app/start/), [autostart](https://v2.tauri.app/plugin/autostart/), [SQL](https://v2.tauri.app/plugin/sql/).
- Electron: [process model](https://www.electronjs.org/docs/latest/tutorial/process-model), [powerMonitor](https://www.electronjs.org/docs/latest/api/power-monitor), [Tray](https://www.electronjs.org/docs/latest/api/tray).
- [Flutter desktop support](https://docs.flutter.dev/platform-integration/desktop).
- [.NET MAUI supported platforms](https://learn.microsoft.com/en-us/dotnet/maui/supported-platforms?view=net-maui-10.0); [Avalonia supported platforms](https://docs.avaloniaui.net/docs/supported-platforms).
- [Windows GetLastInputInfo](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getlastinputinfo): interactive-session scope, not per-agent focus.
- Native macOS example: T2; macOS idle implementation example: A1. No installer, signing, accessibility or resource-use benchmark was run.

## Verification boundaries

README claims, code findings and proposals are deliberately separated. Source links were retrieved while researching; documentation links can subsequently move. Discovery dates do not prove first release dates. Search results were used to find repositories, not as evidence for their algorithms. Missing imported context and unknown custom-MCP backend code remain explicit follow-up items, not guessed answers.
