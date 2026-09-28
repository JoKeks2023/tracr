# Existing tools: implementation findings

Research date: **2026-09-28**. Evidence grades and pinned revisions: [SOURCES](SOURCES.md). Product scope is a desktop developer diary; an agent-usage dashboard is a data-source precedent, not automatically a suitable product base.

## Identity and depth

Names collide. This research distinguishes native **Claudoscope** (cordwainersmith), multi-agent **Claudescope** (vladar107), and the separate JoniMartin27 project. “Claude Monitor” means Maciek-roboblog's Python project unless qualified. “codex-usage” is not a unique repository; both qianhaoq and the newer zJay26 project were examined. [T2–T8](SOURCES.md#t2--claudoscope-native-macos)

| Project | Purpose/interface | Runtime/storage | Platform and deployment | Depth |
|---|---|---|---|---|
| RemoteCTO/claude-code-timelog | Reconstruct timesheets; Claude plugin + CLI | Node ESM, daily JSONL | Local; portable JS, but path/shell assumptions require Windows testing | Hook and slice algorithm read |
| cordwainersmith/claudoscope | Agent dashboard/transcripts, menu bar | Swift, SQLite/GRDB, memory cache | README requires Apple Silicon/macOS 14+ | Watcher, parser and summary cache read |
| ccusage/ccusage | Usage/cost CLI reports | Current tree has Rust core/adapters and npm packaging; reads source logs | Local CLI; distribution/platform matrix needs release verification | Codex parser and shared cost engine read |
| vladar107/claudescope | Multi-agent transcript search/explorer | TypeScript server, DuckDB, canonical NDJSON | Local web interface; not evidence of a packaged desktop app | Provider contract/projections read |
| phuryn/claude-usage | Usage dashboard/CLI/editor view | Python, SQLite; TS VS Code extension | Local server/extension; native desktop not established | Scanner/schema read |
| Maciek-roboblog/Claude-Code-Usage-Monitor | Live allowance/burn-rate terminal view | Python; JSONL reader; tree also includes warehouse/overlay modules | Terminal runtime; exact OS release support not tested | Reader and identity handling read |
| qianhaoq/codex-usage | Small usage CLI | Python stdlib, provider SQLite + rollout JSONL | Local CLI; no native shell | DB reader and blended pricing read |
| zJay26/codex-usage | Codex usage analytics | Go, local CLI/server/platform adapters | README: macOS, Windows, Linux | Tree and selected estimator functions inspected |

Sources by row: [T1](SOURCES.md#t1--claude-code-timelog), [T2](SOURCES.md#t2--claudoscope-native-macos), [T3](SOURCES.md#t3--ccusage), [T4](SOURCES.md#t4--claudescope-multi-agent-explorer), [T5](SOURCES.md#t5--claude-usage), [T6](SOURCES.md#t6--claude-monitor), [T7](SOURCES.md#t7--codex-usage-sqlite-reader), [T8](SOURCES.md#t8--recently-createdadditional-candidates).

## 1. Timelog: useful baseline, insufficient concurrency model

**C:** `buildSlices` groups by session ID, sorts events, and assigns each positive gap below `breakMs` to the preceding event's project/ticket/model. The CLI fallback is 1,800 seconds. Gaps at or above the threshold contribute zero; a final prompt gets zero duration. Aggregation sums these slices. `dateKey` uses UTC ISO dates, while week-start construction uses local calendar operations. No global interval-union or owner-election step appears in this report path. [T1 report](SOURCES.md#t1--claude-code-timelog)

**C:** Project detection tries configured transcript file-path patterns, then Git root or CWD basename. The hook derives a transcript filename from a slash-replaced CWD and reads its final 16 KiB. Branch names feed ticket regex matching. **R:** README describes prompt ticket fallback, SessionStart/UserPromptSubmit/SessionEnd events and backfill provenance. This is ticket inference, not Huly/GitHub issue validation. [T1 hook/README](SOURCES.md#t1--claude-code-timelog)

**P:** Learn event-level attribution and explicit backfill provenance. Do not equate session independence with overlap reconciliation. Prefer a supplied transcript path over re-encoding the CWD. Basenames collide; explicit repository identities are needed. Threshold sensitivity and midnight splitting require separate design.

## 2. Native Claudoscope: resilient observation and rebuildable indexes

**C:** The FSEvents watcher debounces paths, handles overflow by requesting a rescan, and resolves file existence after the debounce so atomic replacement is not mistaken for deletion. It watches a wider agent directory but filters transcript/config/spool events. This is a concrete watcher-reconciliation precedent. [T2 watcher](SOURCES.md#t2--claudoscope-native-macos)

**C:** The parser distinguishes final usage-bearing responses from incomplete streams, elects the largest usage record for certain orphan streams, deduplicates response IDs and record UUIDs, and excludes copied parent context for context-fork subagents. Its SQLite summary cache includes parser version, pricing key and timezone; file fingerprints include parent dependencies where needed. [T2 parser/cache](SOURCES.md#t2--claudoscope-native-macos)

**R:** README describes local processing, optional read-only MCP, notifications and update checking. No independent traffic/permission test was performed. Session notifications are not OS idle detection. The inspected paths do not establish manual timers, Huly integration or exclusive human-time attribution.

**P:** Learn invalidation and recovery, not the assumption that every cache is disposable. tracr's user corrections and confirmed worklogs would be authoritative and must survive cache rebuilds.

## 3. ccusage: accounting is a normalization problem

**C:** Codex parsing distinguishes repeated cumulative snapshots from new usage, prefers usable last-request counts, and otherwise computes saturating deltas. Model/service-tier state can change. Fallback model attribution is flagged. Cost modes separately display stored cost or calculate from tokens; the shared engine handles cache-write TTL and context-size pricing details. [T3](SOURCES.md#t3--ccusage)

**P:** A provider abstraction needs accounting semantics, not just a uniform field name. Detect resets, forks and duplicate transport observations. An estimate from a fallback model should remain visibly uncertain. A report CLI is a strong oracle for fixture comparisons, not a source of human-time truth.

Issue evidence adds useful limits: #1288 reports repeated-counter overcounting in older releases, with a small reproducible fixture; the current inspected parser contains the cumulative-progress guard. #1811 requests a normalized timestamped event API and was closed by automated scope triage, so no stable downstream-library promise should be assumed. PR #1805 describes separate compaction-usage records but was closed unmerged; this remains a fixture-investigation lead, not a verified contract. [Issue sources](SOURCES.md#i1--implementation-limitations-and-requests)

## 4. Multi-agent Claudescope: a concrete connector seam

**C:** AgentConnector separates discovery, optional normalization preparation, SQL event projection, auxiliary projections and session loading. Discovery records path/mtime/size. DuckDB performs shared query/index work; canonical columns are generated from one contract. PR links are optional auxiliary session projections. Cost is derived centrally from token columns. [T4](SOURCES.md#t4--claudescope-multi-agent-explorer)

**P:** Learn capability-based providers and explicit raw/normalized boundaries. A SQL-projection contract is attractive for analytics but couples plugins to the query engine; an event-record contract offers stronger storage independence. Neither is selected here. Its source-memory boundary is useful inspiration for limiting file access.

## 5. claude-usage and Claude Monitor: two accounting choices

**C:** phuryn's scanner stores sessions, turns, agents and processed-file metadata in SQLite. It retains the last streamed record per message ID and has a partial unique index for nonempty message IDs. Metadata includes CWD-derived project, branch and timestamps; user titles outrank inferred titles. These columns do not constitute live GitHub integration. [T5](SOURCES.md#t5--claude-usage)

**C:** the Python monitor's reader constructs a dedup key only when both message and request IDs exist, maps token categories, filters models and delegates pricing. **R:** predictive allowance views use configured/learned limits. Neither the reader nor those predictions proves exact server quota or measured human activity. [T6](SOURCES.md#t6--claude-monitor)

**P:** Cross-check accounting with incomplete identity fixtures. Missing IDs must not silently collapse independent requests; retaining every unidentified record also risks duplication. Keep those cases measurable.

## 6. codex-usage: simplicity with important loss of precision

**C:** qianhaoq selects the highest numbered state database, opens it read-only and checks selected columns. Its query excludes archived threads and filters on creation time while reading lifetime token totals. A thread begun yesterday and used today is therefore not represented as today's usage. Pricing assumes 70% input and 30% output; it cannot reconstruct observed cache categories from one total. [T7](SOURCES.md#t7--codex-usage-sqlite-reader)

**P:** Read-only access and compatibility errors are useful. A versioned internal database filename is not an integration contract. Lifetime totals are a poor daily ledger, and assumed input/output mixtures should not be presented as measured spend.

## Detection/integration coverage

| Tool | Transcript/session detection | Activity, idle and concurrent handling | Git/tickets/GitHub/MCP | Permissions/privacy boundary |
|---|---|---|---|---|
| Timelog | Live hooks; README backfill | Per-session gap estimates; no observed global arbitration | Git root/branch, ticket regex; no verified remote issue sync/MCP | Hook installation; stores prompt excerpts and paths locally |
| Native Claudoscope | File watcher + transcript parser | Live file changes and session state; human idle unverified | Optional MCP claimed; no verified Huly mapping | Agent-file reads; optional configuration-writing features mean not every app action is read-only |
| ccusage | Provider-specific local log readers | Concurrent histories can be aggregated; not exclusive human time | Provider metadata; remote GitHub/Huly sync not established | Local log access; pricing/network behavior must be assessed per configuration |
| Multi-agent explorer | Connector discovery and normalized transcripts | Multiple providers/sessions; no demonstrated human-time arbitration | Optional PR-link projection; API-backed GitHub sync unverified | Local transcript indexing; private text enters search index |
| claude-usage | JSONL scanner + file metadata | Session/subagent usage; idle algorithm not established | Branch field; remote integrations unverified | Local SQLite/server/editor access |
| Python monitor | JSONL scan | Refresh/prediction; budget block is not work session | No verified ticket correlation | Local reader; additional official-overlay modules need separate privacy review |
| qianhaoq codex-usage | Internal DB threads + rollout quota snapshot | Session lifetime, archived excluded | CWD; remote integrations unverified | Read-only DB; no OS surveillance |

All unverified cells are deliberate limits of the inspected paths. See [comparison](comparison.md) for product-level coverage.

## Newer discovery, not a popularity contest

GitHub metadata verifies vladar107's June 2026 creation and zJay26's July 2026 creation, with September pushes. The latter's Go estimator exposes integer nano-USD arithmetic and overflow checks—worth further accounting review. douglasmonsky's May-created tracker is worth studying for local MCP and provenance. Creation/push dates do not prove a first packaged release date. [T8](SOURCES.md#t8--recently-createdadditional-candidates)

**P:** Deep-study shortlist: ActivityWatch, Timelog, ccusage, native Claudoscope, multi-agent Claudescope. Use these as independent references rather than forking a transcript viewer into a timekeeping product. No surveyed combination has yet demonstrated tracr's complete correlation/correction workflow; that is a bounded finding, not a claim that no competitor exists.
