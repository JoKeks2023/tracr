# Codex integration

Research date: **2026-09-28**. Codex is a provider, not tracr's shell or product architecture. [Sources C2](SOURCES.md#c2--codex-interfaces)

## Official/documented interfaces

Current hooks document `session_id`, nullable `transcript_path`, `cwd`, `model` and event name; turn-scoped events add turn identity. Events include prompt submission, tool boundaries, start/end, subagents, stop, compact and interrupt. Subagent hooks use the parent session ID. Hosted tools are not covered by local tool hooks. Background hooks may complete out of order or be canceled at session end. Switching away does not immediately end a session. The documentation explicitly calls transcript format unstable. These are documented capabilities, **not a timeless stability guarantee**. [Hooks](https://learn.chatgpt.com/docs/hooks)

App Server offers thread/turn/item records and notifications, including thread status and token usage. Structured items expose prompts, messages and tool activity; threads carry context/metadata. It is suitable for clients connected to that server. This does not establish a universal passive subscription to every independent CLI or desktop process. [App Server](https://learn.chatgpt.com/docs/app-server)

The CLI's non-interactive mode provides machine-readable events when launched under an integration. It does not retroactively make unrelated sessions observable. [Non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode)

## Capability map

| Requirement | Preferred documented observation | Historical/internal fallback | Confidence boundary |
|---|---|---|---|
| Session/thread identity | Hook ID or server thread ID | Stored thread/rollout metadata | Namespace by provider + machine |
| CWD | Hook/server context | Rollout/DB CWD | May change across turns |
| Repository/branch | Server metadata when provided; local Git | Source metadata | Verify at event time |
| Prompts | Prompt hook/server user item | Transcript records | Optional sensitive payload |
| Tool calls/results | Tool hooks/server item events | Recorded responses | Tool coverage is not universal |
| Model | Hook/server context | Parser model context | Keep fallback flag |
| Usage | Server token update / owned exec stream | Token-count events | Distinguish totals and deltas |
| Timestamp | Source timestamp if present | Ingestion timestamp separately | Never equate the two |
| Transcript location | Nullable hook field | Source discovery | Absence is valid |
| Subagents | Agent lifecycle/items | Child metadata | Preserve child ID, not just parent hook ID |
| Interrupts | Interrupt event / turn status | Incomplete stream evidence | Crash may have no terminal event |
| Activity | Thread/turn state | Event-gap estimate | Agent state is not human presence |

The first column is a tracr requirement; proposed normalization is **P**, not a claim that every listed field is always present. Official evidence: [C2](SOURCES.md#c2--codex-interfaces); observed fallback behavior: [T3](SOURCES.md#t3--ccusage), [T7](SOURCES.md#t7--codex-usage-sqlite-reader).

## Documented but potentially changing

Generate/validate protocol schemas for the specific producer version and negotiate capabilities. The inspected Codex tree contains versioned App Server schemas, hook event modules, a separate rollout crate and internal protocol definitions. This establishes implementation locations, not public support for direct database queries. Plugin configuration and host-provided MCP tools also vary by host; an MCP tool callable in this Codex chat is not automatically available to a standalone tracr application. [C2 source](SOURCES.md#c2--codex-interfaces)

## Observed internal behavior

ccusage reads `turn_context` and `event_msg`/`token_count` records, including cumulative and last-request usage. It tracks model/service tier and suppresses unchanged totals. Its fallbacks demonstrate the need to represent unknown or inferred model identity. qianhaoq's tool reads `threads` in `state_N.sqlite`, assumes columns, excludes archived rows and reads lifetime totals. These are reverse-engineered adapters with different information loss. [T3](SOURCES.md#t3--ccusage), [T7](SOURCES.md#t7--codex-usage-sqlite-reader)

**P:** A SQLite fallback should use read-only mode and fail visibly on incompatible schema. Never write Codex's DB, assume the numerically newest filename is compatible, or treat `tokens_used` as a per-day categorized billing ledger. Archived session history must remain eligible for diary history.

## Common-provider seam to evaluate

**P:** A provider can expose discovery/backfill, live observation, normalization and a capability manifest. Candidate capabilities include lifecycle, request identity, usage categories, child linkage, historical access and source timestamps. Keep native payload extensions separate from portable fields.

Candidate portable envelope: provider ID/version, machine ID, source ID/cursor, native session/turn/item IDs, occurred-at/observed-at, kind, optional repository context, usage semantics (`delta`, `cumulative`, `snapshot`), payload reference, redaction state and quality flags. An absent count is `unknown`, not zero.

Claude and Codex can share event vocabulary and reconciliation while retaining distinct parsers. A tool hook should append a small local record; report generation must not delay the coding session. A transport reconnect should replay safely. This is a candidate boundary only, not an API implementation.

## Follow-up tests before any integration commitment

Use both agent versions and both OSs; exercise CLI and desktop separately. Cover resumed/archived threads, changed model, repeated totals, context compaction, interruption, nested agents, missing transcript, remote sessions, WSL path mapping and collector restart. Verify server subscription scope rather than assuming desktop-wide observation. No Codex launch, plugin installation or private transcript scrape was needed for this research.
