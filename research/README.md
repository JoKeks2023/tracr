# tracr research

Research date: **2026-09-27–2026-09-28**. Research only: no product implementation, finalized application architecture or framework selection.

tracr is a macOS/Windows developer diary and time tracker. Claude Code and Codex are data sources alongside manual timers, Git, GitHub and Huly. The central research conclusion is that **agent usage, human effort and financial expense need separate accounting**. A useful diary reconciles those records with transparent attribution and corrections. It cannot recover precise human attention from open sessions alone. Evidence and worked examples: [activity tracking](activity-tracking.md), [costs](costs.md), [comparison](comparison.md).

## Reading guide

| Document | Main questions |
|---|---|
| [Tools](tools.md) | Which implementations are worth studying, and what do their parsers/watchers actually do? |
| [Claude Code](claude-code.md) | Documented interfaces versus unstable transcript observations |
| [Codex](codex.md) | Hooks, App Server, execution events and internal storage boundaries |
| [Activity tracking](activity-tracking.md) | A→B→A, idle, concurrency, confidence and reconciliation |
| [Manual time tracking](time-tracking.md) | Timers, overlap, edits, provenance and external worklogs |
| [Git and GitHub](github.md) | Repository/worktree identity, issue relationships, API budgets and sync |
| [Huly](huly.md) | Exact custom MCP inventory, live TRACR findings and upstream distinctions |
| [Costs](costs.md) | Token deduplication, estimates, subscription allocation and price history |
| [Reporting](reporting.md) | Daily/weekly facts, narratives, snapshots and corrections |
| [Streaks](streaks.md) | Qualifying days, timezone policies and historical recalculation |
| [Contribution graph](contribution-graph.md) | Calendar semantics, possible metrics and accessible interaction |
| [Desktop](desktop.md) | Cross-platform shell and background integration candidates |
| [Candidate data model](data-model.md) | Raw/derived/editable records, relationships and retention options |
| [Comparison matrix](comparison.md) | Required capabilities with evidence strength and limitations |
| [Best practices](best-practices.md) | Strong evidence, promising ideas and approaches to avoid |
| [Gaps](gaps.md) | Bounded differentiation, candidate MVP and unresolved research |
| [Sources](SOURCES.md) | Exact source files, commit snapshots, official docs and live evidence |

## Context and method

The entire initial repository contained only `README.md` (`# tracr`) at commit `7fc322648c6574f1062c0ff3553aaff14c1ed00b`. Local inventory and GitHub contents/branches did not expose the intentionally imported CLAUDE.md, workflow or MCP documentation. They cannot be treated as read. The user-supplied global instructions and research brief provide the available conventions; no existing instructions or root README were rewritten. Operational handoff: [task brief](../docs/tasks/T-0001_research/00_brief.md). [Context evidence](SOURCES.md#context)

Huly access was verified with explicit `workspace:"jokeks2023"`: TRACR exists and returned zero issues; no teamspaces were listed. The custom tool catalog is documented separately from upstream Huly features. `log_time` is callable in the catalog, but no time entry was written and its return/readback/idempotency semantics remain unverified. Earlier observations from another workspace are excluded. [Live evidence](SOURCES.md#huly-live)

Research prioritizes official interfaces, then pinned source functions, then repository issues/discussions. Tool names were disambiguated by owner. Recently created repositories were checked alongside established tools; creation/push dates are not release dates. No third-party app was installed or runtime-tested. Source inspection was targeted at parsers, accounting, watchers, schemas and correlation; it is not a whole-codebase audit.

Evidence labels throughout are **D** documented, **C** source-inspected, **L** live environment, **R** maintainer claim, **P** proposal/inference and **U** unresolved. “Official” does not imply a permanent compatibility guarantee. Capability absence means not established in the inspected material unless explicitly stated otherwise. All design suggestions remain candidates.

## Findings that materially affect later design

- **Concurrency needs global reconciliation.** Timelog's per-session gap sums can count the same human minutes twice. ActivityWatch heartbeats and WakaTime thresholds provide useful evidence patterns but do not establish exact issue ownership. [Activity tracking](activity-tracking.md)
- **Usage needs identity, not line counting.** Streaming replacements, repeated cumulative counters and inherited subagent context demand provider-specific rules. Current ccusage's parser checks cumulative progress; native Claudoscope elects billing records and excludes copied context. [Tools](tools.md)
- **Watching is a hint, not a durable event ledger.** Claudoscope explicitly handles dropped filesystem events, debouncing and replacement. Replayable source checkpoints and visible coverage loss are candidate reliability requirements. [Tools](tools.md)
- **A common provider vocabulary must retain differences.** ClaudeScope's connector contract is instructive; a universal transcript schema or lifecycle interpretation would hide important distinctions. [Claude](claude-code.md), [Codex](codex.md)
- **Huly export is not yet safe to assume.** The custom schema supplies hours and description but no dated interval or idempotency key. Upstream report entities prove the distinction between individual worklogs and issue aggregates, not accessibility through our MCP. [Huly](huly.md)
- **Time, tokens and money require separate provenance.** Manual overrides must remain explainable; paid subscriptions and API-equivalent token estimates must never be collapsed into one “actual cost.” [Time](time-tracking.md), [Costs](costs.md)

## Confidence and remaining evidence

This package supplies all requested topics and candidate entities, with deep source inspection concentrated on a small set of implementations. It does not establish sensor permission behavior on both operating systems, invoice accuracy, a deployed Huly backend implementation, or complete market coverage. Specific runtime and design-sign-off questions are collected in [gaps](gaps.md); documentation checks and arithmetic verification are recorded in the [validation report](../docs/tasks/T-0001_research/05_validation_report.md).

The missing imported context should be reconciled before design decisions. Establish a provider-version fixture corpus and measure real A→B→A workflows before claiming automatic issue-level time accuracy. A recent ccusage request for normalized timestamped event exports was closed by automated scope triage: studying a parser does not imply an upstream-supported library contract. [Issue evidence](SOURCES.md#i1--implementation-limitations-and-requests)

## Executive Summary

### 1. What existing approaches are strongest?

Provider-specific deduplication, rebuildable derived caches, robust watcher recovery, bounded heartbeats, and explicit manual-entry precedence have the strongest evidence. They solve distinct parts of the problem; no inspected tool establishes the complete tracr workflow. [Best practices](best-practices.md)

### 2. What technologies do they use?

The inspected set spans Swift/GRDB/SQLite/FSEvents in native Claudoscope; a Rust core and provider adapters in current ccusage; TypeScript/DuckDB projections in multi-agent ClaudeScope; Python/SQLite in claude-usage; Node hook scripts in Timelog; and Go with scaled-integer pricing in zJay26's Codex tool. Tauri, Electron, Flutter, native apps, Rust hybrids and .NET are desktop candidates with different integration costs, not a selected stack. [Tools](tools.md), [Desktop](desktop.md)

### 3. What should tracr learn from them?

Keep observations, normalized usage, inferred activity, user corrections and published worklogs distinguishable. Retain source identity and parser/rule versions. Recover missed events, represent missing coverage and support revising an attribution without erasing its evidence. [Data model](data-model.md)

### 4. What should tracr avoid?

Avoid session lifetime as effort, summing concurrent sessions, exact switch times invented from sparse events, undocumented databases treated as stable APIs, unknown prices silently converted to zero, and subscription estimates labeled actual spend. Screen recording and keystroke-content collection are outside the product brief. [Best practices](best-practices.md)

### 5. What data does tracr need?

At minimum: provider/session/actor identities; source and receipt timestamps; CWD/repository/worktree evidence; interaction and lifecycle observations; token categories/model/usage scope; manual intervals and revisions; confirmed issue links; coverage diagnostics; pricing provenance and actual expense records when available. Prompt/tool content should be optional and minimized. [Providers](claude-code.md), [Candidate model](data-model.md)

### 6. What is the hardest technical problem?

Inferring and reconciling human attention across concurrent agents, editors and quiet thinking, while allowing trustworthy correction. More transcript events do not guarantee more knowledge of human effort. The A→B→A example requires explicit ownership evidence or an acknowledged estimate. [Activity tracking](activity-tracking.md)

### 7. What is the smallest sensible MVP?

A local diary with manual timers/corrections, Claude and Codex imports, explicit project/issue binding, conservative reviewable automatic segments, separate usage/estimated cost, and a daily timeline/export. Start remote context read-only; condition Huly writeback on verified receipt and reconciliation behavior. This is a sequencing proposal, not authorization to implement. [Gaps](gaps.md)

### 8. Which projects are worth studying deeply?

Native Claudoscope for ingestion/cache/watch recovery; ccusage for provider accounting; multi-agent ClaudeScope for connector/projection contracts; ActivityWatch for heartbeat and AFK boundaries; Timelog for a small understandable gap algorithm and its limits; claude-usage for scanner persistence. Timing/Kimai supply useful manual-worklog semantics. [Tools](tools.md), [Time tracking](time-tracking.md)

### 9. Which projects are inspiration but should NOT be forked?

At this stage, do not choose any as the tracr base. In particular, a macOS-only viewer, a transcript explorer, a token CLI, or an internal-Codex-SQLite reader brings a different product boundary. Learn their patterns without inheriting their scope or unstable dependencies. This is a product-fit judgment, not a criticism of code quality or a license assessment. [Gaps](gaps.md)

### 10. What makes tracr meaningfully different?

A coherent, correctable issue-level development history across human work, multiple agents, GitHub and Huly, with honest time and expense provenance. Manual plus automatic effort, subscription allocation and diary reporting become valuable together only when reconciliation is trustworthy. This differentiates the proposed combination from the inspected tools; it is not a claim of universal market uniqueness. [Comparison](comparison.md), [Gaps](gaps.md)
