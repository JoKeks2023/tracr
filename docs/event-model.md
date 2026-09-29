# Event model

Status: **Draft specification v0.2 (2026-09-28)**. Evidence: [research/claude-code.md](../research/claude-code.md), [research/codex.md](../research/codex.md), [research/tools.md](../research/tools.md), [research/github.md](../research/github.md), [research/activity-tracking.md](../research/activity-tracking.md), [research/data-model.md](../research/data-model.md). Evidence labels **D/C/L/R/P/U** as in [research/SOURCES.md](../research/SOURCES.md).

## 1. Three layers

| Layer | Contents | Rebuildable? | Retention |
|---|---|---|---|
| **Raw: Observation** | Envelope (always) + retained fact body (always) + payload (optional, policy-controlled) exactly as received | No. Envelope and retained facts are the evidence. | Envelope and retained facts: evidence-metadata retention. Payload: separate. **Declared default** for MVP: off (`never_stored`, EV-08). Whether an opt-in short payload window is offered, and all retention periods, are **Unresolved** ([OQ-P-09](open-questions.md#product-questions)). |
| **Normalized: Normalized Event** | Provider-neutral kind, identities, actor, timestamps, context evidence, usage semantics, quality flags, provider extension | Yes, from observations (payload where present, retained facts otherwise) + normalizer version, for rules whose inputs are retained ([§3](#3-retained-facts-and-replay-guarantees)) | Derived. Compact normalized facts may outlive payload. |
| **Derived: Sessions, Turns, Tool Calls, Token Usage, File Changes, Coverage, Activity…** | Aggregates and inferences over normalized events | Yes, while their required inputs exist ([§3](#3-retained-facts-and-replay-guarantees)) | Derived; referenced records kept while referenced |

**Principle:** every observation keeps its envelope and retained fact body, whatever its payload state. If the payload is `deleted` or `never_stored`, renormalization is limited to rules whose inputs are retained facts. tracr then must **not** claim full replay for the affected period. The envelope records the payload state, so the limit on replay is visible. Guarantees per payload state: [§3](#3-retained-facts-and-replay-guarantees).

## 2. Observation envelope

Conceptual fields. Names are illustrative, not a schema.

```pseudo
Observation {
  id                       // internal
  source {
    adapter, adapterVersion
    channel                // hook | telemetry | transcript | provider_db | app_server | exec_stream
                           // | git | github_api | huly | os_sensor | manual | collector
    machineId
    locator                // file path, endpoint, hook name…
    cursor                 // offset / line / sequence / API cursor, if any
  }
  producer { provider, producerVersion? }   // e.g. Claude Code version if known
  nativeIds { session?, parentSession?, agent?, turn?, item?, request?, message?, toolCall? }
                           // agent and parentSession only if the source provides them
  occurredAt?              // timestamp stated by the source; null if the source states none
  observedAt               // local receipt time (UTC instant)
  monotonicReceipt?        // for ordering within one collector run
  contentHash
  parseStatus              // ok | partial | incomplete_trailing | unknown_format | failed
  facts {                  // retained fact body (§3): non-content facts extracted at ingest
    kind, subtype?         // e.g. user_interaction / prompt_submitted, without prompt text
    timeQuality
    usage?                 // semantics, scope, categories, model + source, serviceTier,
                           // identity, includesCopiedContext, temporalExtent (§6)
    reportedCost?          // provider-reported cost figure if the source contains one: value, unit as stated,
                           // source field, semantics, scope (§3); never billing truth
    tool? { name, kind }   // never arguments or results
    context? { cwd?, repositoryEvidence?, worktreeEvidence?, branchEvidence?, remoteIdentity? }
    collector?             // collector / coverage facts
  }
  extractorVersion         // version of the extractor that produced `facts`
  payload {
    ref?
    state: present | redacted | deleted | never_stored
    retentionClass
    redactedFields?        // when redacted
    deletion? { at, reason }   // when deleted
  }
}
```

Rules:

- **Principle:** `occurredAt` and `observedAt` are never interchanged. A hook's receipt time is not a producer timestamp. If only receipt time exists, normalized events mark their time quality as `receipt`.
- **Principle:** the envelope and `facts` are kept for every observation, whatever its payload state (EV-10). `facts` contains no prompt text, tool arguments, tool results or other payload content.
- Observations are never rewritten (EV-01). How facts re-extracted from a present payload by a newer extractor are stored is part of [OQ-A-04](open-questions.md#architecture-questions).
- Incomplete trailing records (e.g. a partially flushed JSONL line) are kept as `incomplete_trailing` and re-read later. They are not discarded. *(C: T2 watcher precedent.)* See [§10](#10-ingestion-completeness-and-recovery).
- Unknown formats go to quarantine with `parseStatus = unknown_format` and produce a **coverage diagnostic**, never silent loss. *(Principle.)*
- Manual timer actions are observations with `channel = manual`. They also create or modify a Manual Entry, which is the user-authoritative record. The observation is the audit trail of the action.

## 3. Retained facts and replay guarantees

The MVP default is payload off (`never_stored`, **Declared default**, EV-08). Replay guarantees must therefore hold for metadata-only operation. Research lets compact durable facts outlive payload and rules out claiming full replay once source content is deleted ([research/data-model.md](../research/data-model.md), "Retention vocabulary").

**Retained fact body.** A versioned extractor produces it at ingest (`extractorVersion`). It is itself **evidence** and is kept with the envelope (EV-10). Minimum content:

- source identity and location: adapter + version, channel, machine, locator, cursor/offset, content hash, parse status
- provider, producer version (if known)
- native identities: session/thread, turn, item, request/message, tool-call IDs; actor/agent ID **only if the source provides it**; parent-session reference if provided
- timestamps: `occurredAt` (nullable), `observedAt`, time quality, monotonic receipt order
- event kind and subtype (e.g. `prompt_submitted`), without prompt text
- usage facts: semantics (delta/cumulative/snapshot), scope, categories (nullable), model and model source, service tier, dedup identity, copied-context flag, temporal extent ([§6](#6-usage-semantics))
- provider-reported cost figure, where the source contains one: value, currency or unit only as the source states it, source field name, semantics (delta/cumulative/snapshot) and scope as for usage facts ([§6](#6-usage-semantics)). No content. Recorded as the source reports it; it is not billing truth ([cost model §1](cost-model.md#1-monetary-kinds)).
- tool identity (tool name/kind), not arguments or results
- context evidence where available: CWD, repository/worktree/branch evidence, remote identity
- collector and coverage facts
- extractor version

**Durable user decisions.** These are separate user-authoritative records. They never depend on payload: corrections, manual entries, bindings, confirmed/rejected attributions and links, manual mappings (repository → project, project → Huly/GitHub), reviewed worklog revisions, finalized reports, finalized allocations, export records, diary notes, entered subscriptions/expenses. Their preservation follows their origin and lifecycle state ([concepts §2](concepts.md#2-record-origin-and-lifecycle)).

**Payload states and guarantees.**

| Payload state | Durable | Renormalization | Rebuild activity | Rebuild reports |
|---|---|---|---|---|
| `present` | envelope + retained facts + payload | Full, including new content-dependent rules | Yes | Draft reports yes |
| `redacted` | envelope + retained facts + payload minus recorded fields | Only rules that do not need the removed fields | Yes, from retained facts | Draft reports yes |
| `deleted` | envelope + retained facts; deletion time/reason recorded | Only from retained facts. Content-dependent rules cannot run. Extractor defects cannot be corrected. | Yes, from retained facts | Draft reports yes |
| `never_stored` (MVP default) | envelope + retained facts | Same as `deleted`. The payload never existed in tracr. | Yes, from retained facts | Draft reports yes |

**Principle:** if retained facts themselves are removed (user-directed deletion, or evidence-metadata retention expiry), derived records that depend on them become **non-rebuildable**. tracr must not claim they can be rebuilt. Draft derived records are marked non-rebuildable; whether they are kept or removed is **Unresolved** ([OQ-P-22](open-questions.md#product-questions)). Reviewed worklogs, finalized reports and other user records stay. Their evidence links become tombstones, and this is visible.

**Three distinct operations.**

| Operation | Inputs | Produces | Limit |
|---|---|---|---|
| **Renormalization** | Observations (payload where present, retained facts otherwise) + normalizer version | Normalized events | Only rules whose inputs are retained |
| **Activity rebuild** | Normalized events + user records | Agent timelines, human timeline, work status, attribution drafts | User records keep applying. Reviewed revisions are not changed; disagreement becomes a discrepancy ([§9](#9-ordering-and-late-arrival)). |
| **Report rebuild** | Derived records + user records | Draft report projections only | Finalized reports are preserved revisions and are never rebuilt silently |

**Content-dependent derivations** are unavailable under `never_stored` (and under `deleted`, or `redacted` for the removed fields): issue-mention suggestions from prompt or tool text, text search, generated narrative from prompts, and re-extraction to fix extractor defects. Which of these stay permanently unavailable, whether an opt-in temporary payload window allows re-extraction, and how a later-discovered extractor defect is handled are **Unresolved** ([OQ-P-21](open-questions.md#product-questions)).

This section states guarantees, not storage. Storage alternatives stay open ([OQ-A-04](open-questions.md#architecture-questions)). Walkthrough: [metadata-only rebuild](walkthroughs.md#e-metadata-only-rebuild).

## 4. Normalized Event

```pseudo
NormalizedEvent {
  id                      // deterministic: f(observation keys, normalizer version)   [candidate]
  kind                    // see §5
  provider?               // claude_code | codex | git | github | huly | os | manual | tracr
  sessionRef?, turnRef?, itemRef?
  parentSessionRef?       // only where the source provides it
  actor?                  // agent-related events (agent, turn, tool, usage): known(id) | main | unknown
  occurredAt, timeQuality: source | receipt | derived
  observedAt
  context {
    cwd?                  // as observed at this event
    repositoryEvidence?   // git root, common dir, remote identity — as observed, see §7
    worktreeEvidence?
    branchEvidence?
  }
  model? { id, source: reported | fallback | inferred }
  usage?                  // see §6
  derivedFrom: [observationId]
  normalizerVersion
  quality: [flags]        // missing_field:…, fallback_model, out_of_order, duplicate_suppressed, …
  extension: { namespace: provider, … }   // provider-specific, never required by core logic
}
```

**Principle:** core logic (activity, attribution, reporting) reads only portable fields. Provider-specific details live in `extension` and are used only by provider-aware diagnostics or display.

Normalized events are rebuildable from retained observation data (payload where present, retained facts otherwise) for rules whose inputs are retained (EV-03, [§3](#3-retained-facts-and-replay-guarantees)).

**Principle (actor identity, AT-09):** actor identity is capability-limited.

- `known(id)`: the source supplies a native actor/agent ID.
- `main`: only where the provider's documented semantics identify the main agent.
- `unknown`: the source indicates agent activity but does not identify the actor. Example: Codex subagent hooks that carry the parent session ID ([§5.1](#51-claude-code-and-codex-side-by-side)).

Child usage is never assigned to `main` by default. A child ID or parentage is never fabricated. Provider and session refs stay valid when the actor is `unknown`, so provider and session totals remain valid. Per-actor breakdowns show an "unknown actor" row. How Codex child identity can be obtained stays open ([OQ-I-06](open-questions.md#integration-specific-unknowns)). See [attribution §10](attribution.md#10-agentprovider-attribution) and [walkthrough G](walkthroughs.md#g-unknown-actor-identity).

## 5. Event kinds

**Proposed** vocabulary. Kinds describe *what was observed*, not what it implies about human attention. Their meaning for activity is defined separately in [activity model §3](activity-model.md#3-evidence-signals).

| Group | Kinds |
|---|---|
| Session lifecycle | `session_started`, `session_resumed`, `session_ended` |
| Agent lifecycle | `agent_started`, `agent_stopped` (subagents; the main agent has no separate lifecycle kind; actor values per [§4](#4-normalized-event), `main` only where documented, AT-09) |
| Turn lifecycle | `turn_started`, `turn_completed`, `turn_interrupted`, `turn_failed` |
| Interaction | `user_interaction` (subtypes: `prompt_submitted`, `approval_given`, `other`) |
| Tools | `tool_started`, `tool_finished` |
| Context | `cwd_changed`, `context_compacted`, `model_observed` |
| Workspace | `file_changed` |
| Usage | `usage_reported` |
| Version control | `git_commit`, `branch_observed`, `worktree_observed` |
| Remote context | `issue_observed`, `pr_observed`, `issue_linked` (a relation observed or asserted), `issue_state_changed`, `pr_state_changed` |
| Manual | `timer_started`, `timer_stopped`, `manual_entry_recorded`, `binding_set` |
| Device | `device_active`, `device_idle`, `device_locked`, `device_unlocked`, `device_suspended`, `device_resumed` |
| Collector | `coverage_started`, `coverage_ended`, `coverage_degraded`, `parse_failed`, `source_rotated` |

A kind can be emitted only if the adapter's capability manifest declares the underlying capability. Absence of `session_ended` for a crashed session is normal. Sessions may end by **absence of evidence**, which is a derived conclusion, not an event. A `source_rotated` event alone does not establish that nothing was missed ([§10](#10-ingestion-completeness-and-recovery)).

### 5.1 Claude Code and Codex, side by side

Mapping to the *documented* surfaces where research established them. "U" means not established in the research; the adapter must verify it per producer version.

| Normalized kind | Claude Code | Codex |
|---|---|---|
| `session_started` / `session_resumed` | SessionStart hook (**D**) | start hook; App Server thread records (**D**) |
| `session_ended` | SessionEnd hook (**D**). Not guaranteed on crash. | end hook (**D**). Switching away does not immediately end a session (**D**). |
| `user_interaction.prompt_submitted` | UserPromptSubmit (**D**) | prompt-submission hook; App Server user item (**D**) |
| `user_interaction.approval_given` | **U**: no approval source mapped in the research. Emitted only if the adapter's capability manifest declares one. | **U**: as for Claude Code |
| `turn_completed` | Stop (**D**) | stop hook; turn status (**D**) |
| `turn_failed` / `turn_interrupted` | StopFailure (**D**); interruption otherwise inferred from incomplete stream (**U**) | interrupt hook (**D**); a crash may have no terminal event |
| `tool_started` / `tool_finished` | PreToolUse / PostToolUse (**D**) | tool hooks (**D**). Hosted tools are not covered (**D**). |
| `agent_started` / `agent_stopped` | SubagentStart/Stop with agent IDs (**D**) | subagent hooks carry the **parent** session ID (**D**). Child identity is not established (**U**, [OQ-I-06](open-questions.md#integration-specific-unknowns)); App Server items or history are unverified candidates. Until established, actor = `unknown` (AT-09). |
| `cwd_changed` | CwdChanged (**D**, version dependent) | CWD in hook/server context, may change across turns (**D**) |
| `context_compacted` | **U**: no researched hook; compaction is a future fixture case ([research/claude-code.md](../research/claude-code.md)) | compact hook (**D**) |
| `model_observed` | SessionStart / status line (**D**, optional) | hook/server context (**D**); parser fallback flagged (**C**) |
| `usage_reported` | OpenTelemetry (**D**); transcript usage records (public but unstable, **C**) | App Server token usage (**D**); rollout `token_count` records (internal, **C**) |
| History/backfill | local JSONL transcripts (public but unstable) | rollout files and `state_N.sqlite` (internal, reverse-engineered, read-only only). Archived and resumed threads stay eligible (EV-13; [research/codex.md](../research/codex.md), "Observed internal behavior"). |

**Principle:** Claude and Codex share the vocabulary and all downstream logic. Their adapters differ. Asymmetries are recorded as capability-manifest differences. They are not papered over:

- Codex subagent hooks carry the parent session ID, so the child actor is `unknown` until [OQ-I-06](open-questions.md#integration-specific-unknowns) is resolved.
- Claude Code compaction is **U**: no researched hook ([OQ-I-07](open-questions.md#integration-specific-unknowns)).
- Compaction usage possibly absent from cumulative counters is a separate Codex-context lead (ccusage PR #1805, closed unmerged; [research/tools.md](../research/tools.md) §3). It is not evidence about Claude Code ([OQ-I-13](open-questions.md#integration-specific-unknowns)).
- Usage semantics differ ([§6](#6-usage-semantics)).

## 6. Usage semantics

Token usage is the area with the most documented double-counting risk ([research/tools.md](../research/tools.md), ccusage #1288).

```pseudo
Usage {
  semantics: delta | cumulative | snapshot   // what the source number means
  scope: request | turn | session | context_window
  categories {                                // each nullable = unknown
    inputFresh?, inputTotal?, cacheRead?, cacheCreation? { byTtlTier? },
    output?, reasoningOutput? (component of output unless provider says otherwise),
    toolCharges?
  }
  model, serviceTier?
  identity { requestId?, messageId? }         // dedup keys
  includesCopiedContext?                      // subagent/fork parent-context flag
  temporalExtent: instant | [from, to)        // [from, to) for deltas from cumulative counters
}
```

- **Principle:** original counts and their semantics are preserved. Disjoint billable categories are derived only where the provider's definition supports it.
- `snapshot` values (e.g. Claude status-line context counts) are never summed across refreshes (**D**: status line docs).
- `cumulative` counters are converted to deltas with reset detection. Unchanged totals are suppressed. *(C: ccusage Codex parser.)*
- A usage record keeps its temporal extent: an instant, or `[from, to)` for a counter delta (EV-14). A delta may span midnight or a coverage gap. It is not silently assigned to the day of its snapshot or receipt. Session-level totals are temporally coarse in the same way. How such records are reported per day is **Unresolved** ([OQ-P-23](open-questions.md#product-questions)).

## 7. Repository and worktree evidence

**Principle:** repository context is evidence captured *at event time*, not looked up later. The current branch cannot reconstruct historical branch switches.

Captured where available: CWD, Git root, common Git directory, worktree path, HEAD/ref (or detached), normalized remote identity. Non-Git directories, missing remotes, detached HEAD and submodules are valid states, not errors. A directory basename is a display label only. Expensive Git queries may be cached per worktree and refreshed on relevant change. *(Candidate approach.)*

## 8. Deduplication

Several independent duplication mechanisms exist. Each is handled at its own layer:

| # | Duplication source | Layer | Rule (Proposed) |
|---|---|---|---|
| 1 | Same observation delivered twice (retry, rescan, restart, reconciliation) | Observation ingest | Idempotent on source + cursor/native ID + content hash |
| 2 | Same fact reported by several channels (hook + transcript + telemetry) | Normalization | One canonical normalized fact per provider identity (request/message/tool-call ID). `derivedFrom` lists all observations. Channels are ranked per kind in the provider manifest. |
| 3 | Streaming re-emission of the same message with growing usage | Provider normalizer | Provider-specific election (e.g. final usage-bearing record; defined orphan-stream rule) (**C**: T2, T5) |
| 4 | Cumulative counters repeated | Provider normalizer | Delta with saturating/reset handling (**C**: T3) |
| 5 | Copied parent context in subagents or forks | Provider normalizer | Exclude copied history from usage and interaction counts (**C**: T2) |
| 6 | Records without identity | Provider normalizer | Do **not** collapse by similarity. Keep them separate, flagged `unidentified`, and count them in a usage-quality metric (**C**: T6 caveat). Whether they enter counted usage totals, an uncertainty band, or both is **Unresolved** ([OQ-P-24](open-questions.md#product-questions)). |
| 7 | Same external entity seen from Huly and GitHub | Attribution | External Link, not merge. Each keeps its natural key. |
| 8 | Same work time from manual entry and automatic segment | Worklog resolution | Interval union with counting precedence ([time and worklog §3](time-and-worklog.md#3-two-precedence-axes-and-operation-scope)), never addition |

**Principle:** replaying stored observations, or re-delivering or re-importing the same observation (identity per row 1), must not change any total (EV-02). One exception is known. A record without a dedup identity that is re-read after rotation, atomic replacement or truncation may not match its earlier observation, and it may be duplicated ([§10](#10-ingestion-completeness-and-recovery)). How such records count is **Unresolved** ([OQ-P-24](open-questions.md#product-questions)). Idempotent replay alone does not make ingestion complete ([§10](#10-ingestion-completeness-and-recovery)).

## 9. Ordering and late arrival

- Normalized events are ordered by `occurredAt` where `timeQuality = source`, otherwise by `observedAt`. Mixed-quality ordering is flagged.
- Late-arriving evidence (backfill after live collection, a delayed transcript flush, records found by reconciliation, [§10](#10-ingestion-completeness-and-recovery)) triggers re-derivation of affected activity windows. Reviewed worklog revisions are **not** changed automatically. A later derivation that disagrees with a reviewed revision produces a visible **discrepancy** for the user ([time and worklog §6](time-and-worklog.md#6-reconciliation-and-late-evidence)).
- Wall-clock corrections can reorder observations. A collector's monotonic receipt order is kept for tie-breaking within a run. *(Candidate approach.)*

## 10. Ingestion completeness and recovery

**Principle (EV-11):** live notifications (hooks, file-change notifications, server notifications) are **hints**, not a complete ledger. A missed notification must never silently become missing evidence. Research: watchers are invalidation hints; reconcile periodically and after overflow, restart, rotation or atomic replacement ([research/activity-tracking.md](../research/activity-tracking.md), "Reconciliation and invariants").

Each source keeps a durable ingest cursor (`source.cursor`, [§2](#2-observation-envelope)). tracr reconciles the source against that cursor on these triggers:

| Trigger | Obligation |
|---|---|
| Collector start or restart | Read each source from its durable cursor. |
| Detected notification loss or overflow | Reconcile the affected source from its cursor. |
| Rotation, atomic replacement, truncation | Re-read the source. Atomic replacement is **not** deletion. Records already ingested are skipped by their dedup identity ([§8](#8-deduplication) rows 1–2), so they are not counted twice. Records without a dedup identity stay separate and flagged `unidentified` (§8 row 6); a re-read may duplicate them. How they count is **Unresolved** ([OQ-P-24](open-questions.md#product-questions)); duplicate reconciliation between live and backfill is [OQ-A-07](open-questions.md#architecture-questions). |
| Resume from suspend | Reconcile the sources that were being observed. |
| Periodically | Reconcile even when no trigger was detected. |

The reconciliation mechanism, its schedule, the primary channel per provider and late-event windows are **Unresolved** ([OQ-A-07](open-questions.md#architecture-questions)). No watcher technology is chosen. The research precedent is a watcher that rescans on overflow and resolves file existence after a debounce, so atomic replacement is not mistaken for deletion ([research/tools.md](../research/tools.md) §2, T2 watcher). It is a precedent, not a selection.

Rules:

- Partial trailing records are retained as `incomplete_trailing` and re-read later ([§2](#2-observation-envelope)).
- Late and backfilled records are ingested idempotently ([§8](#8-deduplication) row 1). Draft activity is re-derived. Reviewed revisions receive discrepancies, never silent changes ([§9](#9-ordering-and-late-arrival)).
- Archived, resumed and older provider history stays eligible for backfill (EV-13). Archived Codex threads are not excluded ([research/codex.md](../research/codex.md), "Observed internal behavior").
- **Idempotent replay alone is not sufficient.** Replay makes re-reading safe. It does not discover records that a watcher missed.

**Coverage completeness.** Reconciliation records a completeness value for the intervals it covers:

| Completeness | Meaning |
|---|---|
| `unverified` | Live-ingested; no reconciliation has covered the interval yet. The day stays provisional. |
| `verified` | Reconciliation covered the interval, found no records beyond those already ingested, and nothing indicates that evidence may be missing (e.g. records rotated or truncated away before they were read). Where evidence may be missing, the interval is `gap`, never `verified`. |
| `recovered` | Reconciliation found and ingested records that live observation missed. |
| `gap` (with reason) | Evidence may be missing and cannot be recovered, e.g. rotation removed records before reconciliation read them. |

`gap` takes precedence: an interval (or sub-interval, as in [walkthrough F3](walkthroughs.md#f-missed-watcher-notification)) where evidence may be missing is never labeled `verified` or `recovered`.

**Principle (EV-12):** where recovery cannot establish completeness, the interval is a visible coverage gap with a reason. tracr never reports ingestion as complete for an interval where evidence may be missing. An interval that no reconciliation has covered yet is `unverified`, never `verified`. Activity treats a gap as a coverage gap, never as zero activity ([activity model §8](activity-model.md#8-coverage-gaps)). Inside otherwise plausible work, the human-timeline interval is `unknown` (reason `coverage_gap`) and is not counted ([activity model §5](activity-model.md#5-work-status-counting-is-separate-from-activity)). Reports state that activity and usage there may be incomplete.

Walkthrough: [missed watcher notification](walkthroughs.md#f-missed-watcher-notification).

## 11. Coverage and diagnostics

Collectors emit `coverage_*` events. Coverage Intervals are derived per source and state one of: `collecting`, `degraded` (e.g. parse failures, missing fields), `missing` (collector not running, permission lost, source unreachable), `unknown` (before first coverage info).

Each Coverage Interval also carries a **completeness** attribute from reconciliation: `unverified`, `verified`, `recovered`, or `gap` with a reason ([§10](#10-ingestion-completeness-and-recovery)). The two are independent. A source can be `collecting` while an interval inside it is a `gap`, because notifications were lost and the records were rotated away before reconciliation.

Capability limits are diagnostics, not missing coverage. Example: usage recorded with actor `unknown` ([§4](#4-normalized-event)) is reported as such.

Reports and activity inference must consult both the coverage state and its completeness. See [activity model §8](activity-model.md#8-coverage-gaps) and the [report quality footer](reporting.md#4-quality-footer).

## 12. Provider-specific versus portable

| Stays provider-specific (adapter + `extension`) | Portable (core) |
|---|---|
| File locations, transcript layouts, folder-name decoding, DB schemas | Event kinds, identities, timestamps with quality |
| Streaming/billing-record election rules | Usage with explicit semantics and categories |
| Hook names and payload shapes | Context evidence (CWD, repository, worktree, branch) |
| Provider active-time counters (e.g. Claude OTel `user`/`cli`) | Coverage intervals and quality flags |
| Subagent linkage mechanics | Agent/parent relations as normalized refs |

Provider active-time counters are kept as provider observations. They are **not** used as human work time and are not a duration basis ([activity model §4](activity-model.md#4-boundaries-and-duration-basis), [AC-15](requirements.md#activity-ac)). (**D**: Claude monitoring docs note the categories. **P**: they are not a personal timesheet.)
