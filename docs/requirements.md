# Requirements

Status: **Draft specification v0.2 (2026-09-28)**. Phase tags: **MVP**, **Later**, **Deferred** (see [MVP](mvp.md)); **All** means every phase. "Must" statements marked **Principle** are binding for every phase. The rest are **Proposed**. A **Declared default** holds until review changes it; what lies beyond it is listed in [open questions](open-questions.md). Requirements describe behavior, not implementation.

## Evidence and events (EV)

| ID | Requirement | Phase |
|---|---|---|
| EV-01 | Every observation is stored with source identity, channel, machine, cursor, producer version (if known), `occurredAt` (nullable) and `observedAt`. Observations are never rewritten. **Principle** | MVP |
| EV-02 | Ingest is idempotent: re-delivering or re-importing the same observation changes no total. **Principle** | MVP |
| EV-03 | Normalized events carry the normalizer version and references to their observations. They are rebuildable from retained observation data (payload where present, retained facts otherwise), for rules whose inputs are retained ([event model §3](event-model.md#3-retained-facts-and-replay-guarantees)). | MVP |
| EV-04 | Unknown or incompatible source formats are quarantined and reported as coverage `degraded`. They are never dropped silently. | MVP |
| EV-05 | Usage records declare semantics (delta/cumulative/snapshot), scope, categories (nullable) and dedup identity. Unknown ≠ zero. **Principle** | MVP |
| EV-06 | Provider-specific deduplication (streaming re-emission, cumulative counters, copied context, multi-channel duplicates) follows [event model §8](event-model.md#8-deduplication). Unidentified records are not merged by similarity. | MVP |
| EV-07 | Repository/worktree/branch context is captured at event time where available. | MVP |
| EV-08 | Payload content (prompts, tool I/O) is optional and off by default (**Declared default** for MVP: `never_stored`). It is governed by a separate retention setting. A never-stored or deleted payload limits renormalization to retained facts ([event model §3](event-model.md#3-retained-facts-and-replay-guarantees)). Affected periods are marked. Opt-in payload window and retention periods: [OQ-P-09](open-questions.md#product-questions). | MVP |
| EV-09 | Coverage intervals are derived per source and consulted by activity and reports. | MVP |
| EV-10 | Every observation keeps the minimum retained fact body ([event model §3](event-model.md#3-retained-facts-and-replay-guarantees)), whatever its payload state. **Principle** | MVP |
| EV-11 | Live notifications (watchers, hooks) are hints, not a complete ledger. tracr reconciles each source against its durable ingest cursor after start/restart, detected notification loss/overflow, rotation/atomic replacement/truncation and resume from suspend, and periodically. Partial writes are retained and re-read. Late and backfilled records are ingested idempotently ([event model §10](event-model.md#10-ingestion-completeness-and-recovery)). Mechanism and schedule are open ([OQ-A-07](open-questions.md#architecture-questions)). **Principle** | MVP |
| EV-12 | Where recovery cannot establish completeness, the interval is a visible coverage gap with a reason. tracr never reports ingestion for it as complete ([event model §10](event-model.md#10-ingestion-completeness-and-recovery)). **Principle** | MVP |
| EV-13 | Archived, resumed and older provider history stays eligible for backfill (e.g. archived Codex threads). | MVP |
| EV-14 | Usage records keep their temporal extent (an instant, or an interval for counter deltas). A delta spanning days or coverage gaps is not silently assigned to one day ([OQ-P-23](open-questions.md#product-questions)). | MVP |

## Activity (AC)

| ID | Requirement | Phase |
|---|---|---|
| AC-01 | The human timeline and the agent timelines are separate. Agent time never contributes to human time by itself. **Principle** | MVP |
| AC-02 | Counted human time in any period ≤ the period's length. Each instant is counted at most once, regardless of how many sessions or contexts are active. Concurrent sessions are never summed. **Principle** | MVP |
| AC-03 | Session span is never used as effort. **Principle** | MVP |
| AC-04 | Boundaries not pinned by evidence are represented as windows (earliest/latest, basis). **Principle** | MVP |
| AC-05 | Continuation bridging only between engagement instants in the same context, blocked by idle/lock/other-context evidence, by missing coverage, and by any interval whose ingestion completeness is `gap` (EV-12, [event model §10](event-model.md#10-ingestion-completeness-and-recovery)). | MVP |
| AC-06 | Idle beyond a threshold proposes an end at the last-input instant (a `policy` boundary). Lock/suspend ends automatic inference; it does not prove that work stopped. Idle and lock never end user-asserted time. | MVP |
| AC-07 | Segments carry a categorical evidence grade and reason codes ([activity model §7](activity-model.md#7-evidence-grades)). | MVP |
| AC-08 | Missing coverage is shown as a coverage gap, never as inactivity or zero. Inside otherwise plausible work it gives work status `unknown` (`coverage_gap`), which is not counted. **Principle** | MVP |
| AC-09 | Derivation is deterministic for identical inputs and rule versions. | MVP |
| AC-10 | Unknown windows are offered for resolution with a finite choice list (work for one engaged context, e.g. A or B; work jointly for several engaged contexts; break; "Other…"). Leaving a window unresolved keeps it `unknown` and uncounted. | MVP |
| AC-11 | Numeric confidence, editor/terminal sensors, learned thresholds. | Deferred |
| AC-12 | **Unknown is not work.** An interval whose work status is `unknown` never enters the counted human total unless the user asserts it (confirm as work, manual entry). Whether an explicitly enabled, labeled policy may count it is **Unresolved** ([OQ-P-18](open-questions.md#product-questions)). Window policies only propose (`policy_estimated`). `unknown` ≠ `unattributed` ([activity model §5](activity-model.md#5-work-status-counting-is-separate-from-activity)). **Principle** | MVP |
| AC-13 | Concurrent agent timelines overlap freely. Each session is logged, stored and displayed individually. Sessions are never collapsed into one active session ([activity model §9](activity-model.md#9-concurrent-agent-sessions-and-the-human-timeline)). **Principle** | MVP |
| AC-14 | A human-timeline interval may reference several engaged contexts. tracr does not split it without evidence or a user statement. | MVP |
| AC-15 | Observed instants are not durations. Each counted minute carries a duration basis ([activity model §4](activity-model.md#4-boundaries-and-duration-basis)). Inferred minutes are never reported as measured or observed. **Principle** | MVP |

## Attribution (AT)

| ID | Requirement | Phase |
|---|---|---|
| AT-01 | Attribution states: confirmed, inferred, ambiguous, conflicting, rejected, unattributed, with origin and evidence refs. `unattributed` applies to counted human work or to agent/usage records only. It is distinct from the work status `unknown` ([attribution §3](attribution.md#3-attribution-states)). | MVP |
| AT-02 | Every Huly reference includes the workspace. A reference without it is invalid. **Principle** | MVP |
| AT-03 | Users can bind sessions, worktrees and timers to a Project, Huly issue or GitHub issue, with validity periods. | MVP |
| AT-04 | Attribution precedence: correction > manual entry > binding > inference > unattributed ([attribution §5](attribution.md#5-precedence)). Bindings affect attribution only; they never create human time (TW-09). Overridden records remain visible. Composition of corrections: [OQ-P-19](open-questions.md#product-questions). Competing bindings are `conflicting` until resolved ([OQ-P-20](open-questions.md#product-questions)). | MVP |
| AT-05 | Rejected suggestions are not re-suggested after reindexing. | MVP |
| AT-06 | No equal split between multiple mentioned issues. Multi-target human time is a single target, an explicit user allocation, or joint (unsplit, counted once). Otherwise it is ambiguous ([attribution §7](attribution.md#7-multiple-targets-joint-contexts-and-allocation)). **Principle** | MVP |
| AT-07 | **Principle** (all phases): External Links (Huly↔GitHub, PR↔issue) require evidence, never title similarity alone. MVP: PR closing relationships read from GitHub context are used as issue *suggestion* evidence (M7). Later: persisted, confirmable External Link records (Huly↔GitHub sync relations, PR↔issue). | All (evidence rule) / MVP (suggestion evidence) / Later (link records) |
| AT-08 | Huly is a context provider. tracr's model is not stored in Huly. **Principle** | MVP |
| AT-09 | Actor identity is capability-limited. Provider and session attribution come from the source where it provides them. The actor is recorded only when identified; otherwise actor = `unknown`. tracr never invents parentage or child IDs and never assigns child usage to `main` by default. Provider and session totals stay valid ([attribution §10](attribution.md#10-agentprovider-attribution); [OQ-I-06](open-questions.md#integration-specific-unknowns)). **Principle** | MVP |
| AT-10 | Operations on human time never reattribute agent sessions, agent segments, usage, tool calls or costs. Those change only through operations whose subject is that record or its session/worktree ([time and worklog §3](time-and-worklog.md#3-two-precedence-axes-and-operation-scope)). **Principle** | MVP |

## Time and worklog (TW)

| ID | Requirement | Phase |
|---|---|---|
| TW-01 | Manual timers and retroactive entries work without agents or network. Timer state persists immediately. | MVP |
| TW-02 | Corrections are append-only, reference the revision they apply to, and record actor, time and reason. **Principle** | MVP |
| TW-03 | Draft entries re-derive automatically. Reviewed/finalized entries never change automatically. New evidence shows as a discrepancy. **Principle** | MVP |
| TW-04 | Overlapping manual entries require user resolution: split, override, explicit allocation or joint. Until then the overlap is counted once (both entries assert work), with attribution `conflicting`. | MVP |
| TW-05 | Stored precision is preserved. Rounding happens only at declared export/report boundaries. | MVP |
| TW-06 | Intervals are split at local midnight in the reporting timezone and handle DST correctly. | MVP |
| TW-07 | Export creates an Export Record per target before sending. An `outcome_unknown` result is never auto-retried. **Principle** | Later |
| TW-08 | Huly time export via a transport-independent port. | Deferred until the idempotency/readback evidence exists |
| TW-09 | Counting precedence: correction > manual entry > inferred activity > unknown. Bindings never appear on the counting axis and never create human time ([time and worklog §3](time-and-worklog.md#3-two-precedence-axes-and-operation-scope)). **Principle** | MVP |
| TW-10 | Every correction records its subject, axis (counting or attribution), the fields it changes, the revision it applies to, actor, time and reason ([time and worklog §3](time-and-worklog.md#3-two-precedence-axes-and-operation-scope)). | MVP |
| TW-11 | Timer elapsed time is computed from durable boundaries. Behavior across sleep/suspend, wall-clock changes and simultaneous starts follows a visible, stated policy ([time and worklog §8](time-and-worklog.md#8-rounding-days-timezones-and-timers); [OQ-P-25](open-questions.md#product-questions)). | MVP |

## Cost (CO)

| ID | Requirement | Phase |
|---|---|---|
| CO-01 | Five monetary kinds stay distinct: actual expense, estimated API cost, provider-reported cost figure, subscription cost, allocated subscription cost. Each real charge is recorded once. Subscription cost classifies a charge; it is never a second amount. Kinds that are not paid charges are never added to paid totals. Allocated shares are never added to actual expense ([cost §1](cost-model.md#1-monetary-kinds)). **Principle** | All (kinds) / MVP displays estimated API cost and provider-reported figures (where an imported source contains them) / Later (actual expense, subscription cost, allocation) |
| CO-02 | Unknown price → unpriced usage, reported in coverage. Never zero. **Principle** | MVP |
| CO-03 | Estimates are computed per usage record with a recorded pricing revision. Finalized reports are not repriced silently. | MVP |
| CO-04 | Pricing sources are versioned and none is treated as universal. Model pricing is never guessed by substring. | MVP |
| CO-05 | Subscription-covered usage estimates are labeled as API-equivalent, not spend. **Principle** | MVP |
| CO-06 | Allocation = shares + unallocated remainder = charge, with a recorded policy/version and allocation basis. | Later |
| CO-07 | Actual expense entry (user-entered). | Later |
| CO-08 | Billing imports, currency conversion, labor-rate analytics. | Deferred |
| CO-09 | Assigning part of a periodic charge to a day, project or issue requires an explicit allocation basis. Otherwise that part is unallocated ([cost §5](cost-model.md#5-subscriptions-and-allocation)). **Principle** | All |

## Integrations (IN)

| ID | Requirement | Phase |
|---|---|---|
| IN-01 | Adapters declare capability manifests. The core never depends on provider names, file layouts or transport tool names. **Principle** | MVP |
| IN-02 | Claude Code and Codex use the same core vocabulary and logic. Differences live only in manifests/normalizers. | MVP |
| IN-03 | Reverse-engineered sources are read-only, version-gated, and fail visibly. | MVP |
| IN-04 | Passive hooks do not alter agent behavior and preserve existing hooks. Installation is explicit and reversible. | MVP |
| IN-05 | Remote integrations are read-only by default. Permission loss marks mirrors unavailable, not deleted. | MVP |
| IN-06 | GitHub access respects rate-limit headers and pagination. | MVP |
| IN-07 | No reading of provider credentials or undocumented account endpoints. **Principle** | MVP |

## Reporting (RP)

| ID | Requirement | Phase |
|---|---|---|
| RP-01 | Daily timeline with the human timeline (work status visible), one agent timeline per session (overlapping), device state, coverage gaps. | MVP |
| RP-02 | Every figure carries a provenance class (observed, derived, inferred, user-asserted, generated, unknown) ([reporting §1](reporting.md#1-provenance-classes-in-reports)). **Principle** | MVP |
| RP-03 | Quality footer (coverage, attribution shares, corrections, usage completeness, cost kinds, unpriced share) included in exports. | MVP |
| RP-04 | Finalized reports are preserved revisions. Late data never changes them automatically. It appears as a visible discrepancy, and a new revision requires explicit user action ([time and worklog §5](time-and-worklog.md#5-worklog-lifecycle), [§6](time-and-worklog.md#6-reconciliation-and-late-evidence)). | MVP |
| RP-05 | **Principle** (all phases): any weekly/monthly total is the sum of the same daily semantics. Weekly/monthly views themselves are Later. | All (rule) / Later (views) |
| RP-06 | Streaks, contribution calendar, generated narrative, allocation section. | Later |
| RP-07 | Counted human total = per-target exclusive + joint + counted work without a resolved target (`unattributed`, `ambiguous`, `conflicting`). Unknown and non-work minutes are shown separately. Per-target exclusive figures, allocation shares and joint minutes counted once never sum to more than the counted human total; joint minutes shown under several targets are labeled "joint with …" and are never summed across targets ([reporting](reporting.md#human-time-totals-and-parallel-sessions)). **Principle** | MVP |
| RP-08 | Mixed-provenance intervals are explained by minutes per duration basis and by boundary basis ([reporting](reporting.md#mixed-provenance)). | MVP |
| RP-09 | Agent activity lists every session individually, with overlapping timelines, execution time and usage per session, provider and actor. An unknown actor is shown as such. | MVP |

## Privacy and security (PS)

| ID | Requirement | Phase |
|---|---|---|
| PS-01 | No screen recording, screenshots, OCR, keystroke content, clipboard or accessibility-tree scraping. **Principle** | All |
| PS-02 | Filesystem access limited to user-approved provider and repository locations. | MVP |
| PS-03 | Transcript/tool content is treated as untrusted display data, never executed or followed as instructions. | MVP |
| PS-04 | Any local server or IPC surface requires access control. | MVP (if one exists) |
| PS-05 | User-directed deletion covers payload, retained facts, derived sensitive content, indexes and exports under tracr's control. Derived records that depend on deleted retained facts become non-rebuildable ([event model §3](event-model.md#3-retained-facts-and-replay-guarantees)). What deletion removes versus tombstones for reviewed worklogs, finalized reports, export records and backups is **Unresolved** ([OQ-P-22](open-questions.md#product-questions)). | MVP |
| PS-06 | Sharing/publishing is a separate, explicit action with a preview. | Later |

## UI conventions (UI)

| ID | Requirement | Phase |
|---|---|---|
| UI-01 | Enumerable choices (projects, issues, workspaces, provenance, rounding, metrics, manual-entry categories, unknown-window resolutions: work for an engaged context, joint, break) use dropdown/select controls, with an "Other…" escape where free text is genuinely needed. (User's global convention.) | All |
| UI-02 | Uncertain boundaries, inferred minutes and attribution, unknown windows and coverage gaps are visually distinct from user-asserted and confirmed data, and not by color alone. | MVP |

## Platform (PL)

| ID | Requirement | Phase |
|---|---|---|
| PL-01 | The product target operating systems are macOS and Windows ([mvp §4](mvp.md#4-platform-target-and-delivery-staging)). **Principle** | All |
| PL-02 | No OS-parity claim without per-OS validation. **Principle** | All |
| PL-03 | Whether MVP delivery is staged by OS is **Unresolved** ([OQ-P-26](open-questions.md#product-questions)). Framework/shell selection is a separate question ([OQ-A-06](open-questions.md#architecture-questions)). | MVP |
