# Claude Code integration

Research date: **2026-09-28**. Official documentation is current at retrieval; installed-version compatibility must be tested later. [Sources C1](SOURCES.md#c1--claude-interfaces)

## Official and documented

| Signal | Documented surface | Qualification |
|---|---|---|
| Session ID, CWD, transcript path | Hook common input | Prefer supplied path |
| Prompts | UserPromptSubmit | Sensitive content |
| Tool arguments/results | PreToolUse/PostToolUse | Event-specific fields |
| Start/resume/end | SessionStart/SessionEnd | Lifecycle, not work duration |
| Subagents | SubagentStart/Stop, agent IDs/path | Preserve parent relationship |
| Completion/failure | Stop/StopFailure | Not necessarily process exit |
| CWD changes | CwdChanged | Version dependent |
| Model | SessionStart/status line | Optional in some starts |

Hooks run code; passive tracking should avoid returning decisions or injecting context. Do not interpret TeammateIdle as human idle. Hook input is not a complete timestamped billing ledger. [Hooks](https://code.claude.com/docs/en/hooks)

Status-line JSON exposes workspace/model/session and cost/context observations. Current `context_window.total_input_tokens` includes cached input and represents the current context; `total_output_tokens` refers to the latest response. Repeated status refreshes cannot simply be added. [Status line](https://code.claude.com/docs/en/statusline)

OpenTelemetry supplies token categories, model/request/prompt correlation and active-time categories `user` and `cli`. CLI processing can overlap another session's human work. Export is configurable; prompt/tool content choices have privacy consequences. Treat provider active-time counters as provider observations, not a reconciled personal timesheet. [Monitoring](https://code.claude.com/docs/en/monitoring-usage)

Programmatic print/JSON/stream-JSON operation is an integration option when tracr owns a launched process. Plugins package capabilities; MCP lets the agent use tools. Neither by itself observes every pre-existing terminal session. [CLI](https://code.claude.com/docs/en/headless), [plugins](https://code.claude.com/docs/en/plugins)

## Public but unstable

The local JSONL ecosystem provides historical records, but compatibility should be pinned to observed producer versions. Inspected parsers use message IDs, record UUIDs, timestamps, `sessionId`, `cwd`, `gitBranch`, message model, usage categories, tool records and nested subagent files. These are concrete source dependencies, not a blanket stable schema guarantee. [T2](SOURCES.md#t2--claudoscope-native-macos), [T5](SOURCES.md#t5--claude-usage)

| Data requirement | Candidate acquisition | Evidence/risk |
|---|---|---|
| Repository | Local Git query against event CWD | P: verify worktree identity at observation time |
| Branch history | Recorded branch or event-time Git snapshot | C/P: current branch cannot recover historical switches |
| Timestamps | Transcript/telemetry time; hook receipt time separately | C/P: no invented producer timestamp |
| Input/output/cache totals | OTel or deduplicated response records | C: streaming records repeat usage |
| Prompt/tool history | Transcript parser | C: partial lines and schema changes |
| Subagent usage | Child sources + parent linkage | C: copied parent context can duplicate usage |
| Interruptions | Failure/lifecycle evidence, incomplete stream | U: not every interruption has a complete final usage record |
| Activity state | Event-derived estimate | P: not a documented human-focus truth |

## Undocumented / inferred

Folder-name-to-CWD decoding, context-fork filename rules, incomplete-stream billing election and internal analytics facets are format assumptions. T1 reconstructs paths; T2 contains defensive logic for copied history and orphan usage. They are useful compatibility research, not provider promises. The maximum usage seen for a message is not proof of a settled invoice. [T1](SOURCES.md#t1--claude-code-timelog), [T2](SOURCES.md#t2--claudoscope-native-macos)

Do not derive subscription percentage from tokens, treat filesystem mtime as activity time, or infer human presence from tool output. Do not read OAuth credentials or undocumented account endpoints merely because another monitor does so. **P:** prefer explicit supported telemetry and local evidence with clear coverage.

## Candidate integration approaches

| Approach | Benefit | Tradeoff and open question |
|---|---|---|
| Hooks + durable local spool | Low-latency lifecycle/context | Installation and version gating; missing shutdown events |
| OTel collector on loopback | Structured usage/correlation | Additional component/configuration; dedup with transcripts |
| Transcript import/watch | Historical backfill; existing sessions | Unstable format; private payload; delayed flush |
| CLI wrapper | Controlled stream and launch identity | Covers only wrapped launches; changes workflow |

**P:** Evaluate combinations, not four independent sources to sum. Use one canonical identity per observed request and record all observation origins. Retain collector health, producer version and missing-field diagnostics. Default to metadata and usage; make prompt retention an explicit setting.

## Required later compatibility checks

Record fixtures for start, resume, model switch, compaction, cancellation, truncated stream, nested agents, two processes, alternate agent home, Windows paths and log rotation. Verify hook installation preserves existing hooks. Compare usage against provider-reported values while explicitly separating accounting estimates from invoices. Verify background generation does not claim the user's concurrent task time. No hook was installed in this research phase.
