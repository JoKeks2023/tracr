# MVP boundary

Status: **Draft specification v0.2 (2026-09-28)** — **Proposed**. It follows the research's candidate MVP ([research/gaps.md](../research/gaps.md#smallest-sensible-mvp-candidate), executive summary Q7). This is a sequencing proposal. It does not authorize implementation.

## 1. Goal of the MVP

To prove that tracr can produce a **trustworthy, correctable daily record** for one developer who runs several coding-agent sessions concurrently. Human time is never double-counted, and time whose status as work is unknown is never counted as work. Counted time is attributed to projects/issues with reviewable evidence. Every concurrent agent session stays fully visible beside it, with its usage and estimated cost, which is never presented as spend. Everything else depends on this ledger being trusted first.

## 2. Included

| # | Capability | Scope limits |
|---|---|---|
| M1 | **Local diary and ledger** for one user on one machine | Local-first. No account, no sync, no team. |
| M2 | **Manual timers and retroactive entries**, with corrections (confirm as work, mark break/exclude, split, merge, trim, extend, reattribute, allocate, mark joint) and revision history | Works with no agent and no network. Each correction states its axis, counting or attribution ([time and worklog §3](time-and-worklog.md#3-two-precedence-axes-and-operation-scope)). Corrections of human time never reattribute agent sessions, usage or cost (AT-10). Timer behavior across sleep, clock changes and simultaneous starts follows a stated policy (TW-11, OQ-P-25). |
| M3 | **Claude Code and Codex import**: backfill from history plus live observation where a documented channel is available, with producer-version and coverage indicators. Recovery and reconciliation against the durable ingest cursor after start or restart, detected notification loss or overflow, rotation, atomic replacement or truncation, resume from suspend, and periodically (EV-11). Archived, resumed and older provider history stays eligible for backfill. | Metadata + usage by default: payload `never_stored`, retained fact body kept ([event model §3](event-model.md#3-retained-facts-and-replay-guarantees)). Prompt/tool content off by default. Live notifications are hints, not a complete ledger. Intervals that recovery cannot complete are visible coverage gaps with a reason (EV-11 to EV-13; [event model §10](event-model.md#10-ingestion-completeness-and-recovery)). Mechanism and schedule stay open (OQ-A-07). Reverse-engineered sources are read-only, version-gated, and fail visibly. |
| M4 | **Normalized events and three-clock separation**: session span, agent execution, human timeline. Agent timelines of concurrent sessions overlap and each is individually visible. | Symmetric provider handling. Sessions are never collapsed into one active session (AC-13). Actor identity is capability-limited; an unidentified actor is shown as unknown (AT-09). |
| M5 | **Conservative automatic human segments**: engagement instants, same-context continuation, idle trimming proposals, lock/suspend ending automatic inference, unknown windows, conflicts shown | Categorical evidence grades only. No numeric confidence. Inferred minutes are labeled inferred and provisional. Unknown windows stay `unknown` and uncounted until the user acts. Window policies only pre-fill suggestions in review (OQ-P-03, OQ-P-18). |
| M6 | **Explicit project/issue binding**: bind a session, worktree or timer to a Project / Huly issue (workspace-scoped) / GitHub issue | Binding is the main attribution path. Bindings affect attribution only; they never create human time (TW-09). |
| M7 | **Minimal inferred attribution**: worktree → repository → project via user mappings; issue suggestions from branch key, and from PR closing relationships read from GitHub context, shown in review | Suggestions only unless the `inferred` rule in [attribution §4](attribution.md#4-evidence-ladder-for-issue-attribution) holds; rejections remembered. PR closing relationships are suggestion evidence only. Persisted, confirmable External Link records are Later (AT-07). |
| M8 | **Review flow** turning drafts into reviewed worklog entries; unknown windows offered for resolution with a finite choice list; discrepancy display for late evidence | Finalization optional (see OQ-P-07). Leaving a window unresolved keeps it unknown and uncounted (AC-10). |
| M9 | **Usage and estimated API cost**, deduplicated, with pricing source/revision and unpriced coverage. Provider-reported cost figures, where an imported source contains them, retained and labeled as their own kind, never shown as spend. | No actual-expense import; no subscription allocation. Usage per session, provider and actor; unknown actor shown as such (RP-09). Usage keeps its temporal extent (EV-14). |
| M10 | **Daily timeline and report** with provenance classes and quality footer; local export | Daily only. Weekly/monthly views are Later (RP-05). Counted human total = per-target exclusive + joint + counted work without a resolved target (`unattributed`, `ambiguous`, `conflicting`); unknown and non-work shown separately (RP-07; [reporting](reporting.md#human-time-totals-and-parallel-sessions)). No generated narrative. |
| M11 | **Read-only context** from Git (local), GitHub and Huly (explicit workspace) where verified | Scheduled pulls, no webhooks |
| M12 | **OS idle/lock/suspend sensor** | No foreground-app, window title, editor or terminal sensors |

## 3. Explicitly deferred

| Deferred capability | Why |
|---|---|
| **Huly time writeback** | `log_time` has no advertised idempotency, interval, readback or update. Duplicate-hour risk is unresolved (research, huly). Needs the fork source/handler evidence first. |
| Bidirectional sync of anything | Requires remote readback with stable IDs |
| **Subscription allocation** and actual-expense ledger | Depends on a trusted reviewed-time ledger and a chosen allocation policy |
| Weekly/monthly report views | Later (RP-05). Daily semantics must be trusted first; any later weekly/monthly total is the sum of the same daily semantics. |
| Window policies that count unknown time | Whether any policy may count unknown time is **Unresolved** (OQ-P-18). In MVP, window policies only pre-fill suggestions. |
| Streaks and contribution calendar | Depend on a qualifying-day policy and trusted daily totals |
| Automatic issue-level splitting within a session; numeric confidence | Needs calibration against corrected diaries (research, activity-tracking validation protocol) |
| Editor, terminal-pane and foreground-app sensors | Separate permission/privacy review; cross-OS parity unknown |
| Generated narrative diary | Needs fact-bundle design and privacy boundary |
| Project/agent setup diagnostics and Canon | Not researched in depth; Canon undefined ([setup](project-and-agent-setup.md)) |
| Multi-device merge, team use | Identity, conflict and privacy model not designed |
| GitHub webhooks, local MCP query surface | Need a relay design / local access-control design |
| Billing imports, currency conversion, labor-rate/feature-cost analytics | No billing evidence inspected |
| OS-parity claims | No parity claim without per-OS validation (PL-02). The target stays macOS and Windows (§4). |

## 4. Platform target and delivery staging

**Principle:** the product target is **macOS and Windows** (PL-01; [research/README.md](../research/README.md), [research/desktop.md](../research/desktop.md)). v0.2 makes no deliberate scope change.

**Unresolved:** whether MVP delivery is staged by OS (PL-03, [OQ-P-26](open-questions.md#product-questions)). This specification does not choose an order.

The desktop framework or shell is not chosen ([OQ-A-06](open-questions.md#architecture-questions)). The MVP specification is platform-neutral.

**Principle:** no OS-parity claim without per-OS validation (PL-02). Behavior such as idle/lock/suspend sensing, background lifecycle and permissions is claimed for an OS only after it has been validated there ([research/desktop.md](../research/desktop.md)).

Validation constraint: heavy platform builds and per-OS validation run in cloud CI where possible. Local simulators/emulators are not booted, and heavy local builds are avoided (developer machine constraint; [research/desktop.md](../research/desktop.md)).

## 5. Why these boundaries

1. **The ledger comes first.** Streaks, calendars, allocation and feature-cost analytics are all functions of reviewed time. If the ledger is wrong, everything built on it is wrong in a way the user cannot see.
2. **Read before write.** Remote writes without idempotency can corrupt the user's systems of record. Reads fail safely.
3. **Explicit before inferred.** Binding plus review gives correct attribution today. Inference quality can then be measured against the user's corrections.
4. **Documented before reverse-engineered.** Unstable formats are accepted for backfill only with visible version gating.
5. **Honest uncertainty is a feature.** Unknown windows add review work. They stay `unknown` and uncounted until the user acts; a window policy may only pre-fill a suggestion in review. No default suggestion policy is adopted until real-day data on corrections shows which suggestions users accept (§6; [OQ-P-03](open-questions.md#product-questions)). Whether any policy may count unknown time is a separate question and **Unresolved** ([OQ-P-18](open-questions.md#product-questions)).
6. **Parallel sessions are the normal case.** The target user runs several agent sessions at once ([product §2](product.md#2-the-user-problem)). Showing every session and counting human time once are therefore MVP properties, not later refinements.

## 6. MVP exit criteria (Proposed)

The expected results are traced in [specification walkthroughs](walkthroughs.md) A–H. They are specification walkthroughs, not implementation tests.

- For scripted A→B→A, interleaved, idle, lock, overnight-agent, crash and midnight scenarios (research, activity-tracking validation protocol), counted human time never exceeds the period's length, counts each instant at most once, and never includes agent-only periods (AC-01, AC-02).
- Unknown windows are never counted. In the A→B→A scenario the unresolved outcome counts 87 inferred minutes and shows 20 unknown minutes separately. Marking both windows as break yields 87 counted and 20 non-work. Only the user's confirmation of both windows as work yields 107 ([walkthrough B](walkthroughs.md#b-a--b--a-with-a-possible-break)). Break exclusion is tested explicitly, not merely by a total below the enclosing span.
- A scripted parallel-session scenario with three overlapping sessions and two providers keeps every session visible with its own agent timeline and usage, and counts jointly confirmed time once: 55 counted minutes after review, with per-target figures never summed to 58 ([walkthrough C](walkthroughs.md#c-parallel-sessions)).
- Missed-notification (detected or found only by periodic reconciliation), partial-write, rotation/replacement/truncation and restart scenarios produce either recovered records, with no record that carries a dedup identity counted twice, or visible coverage gaps with a reason. Records without a dedup identity that a re-read after rotation, atomic replacement or truncation may duplicate stay separate and flagged `unidentified`, never merged by similarity; how they count is open ([event model §10](event-model.md#10-ingestion-completeness-and-recovery); EV-06, [OQ-P-24](open-questions.md#product-questions)). A partial trailing record is kept (`incomplete_trailing`), not counted as usage until it parses, and ingested once when re-read. Ingestion is never reported as complete for an unrecovered interval ([walkthrough F](walkthroughs.md#f-missed-watcher-notification); EV-11, EV-12).
- Usage with an unknown actor keeps provider and session totals valid and appears as an unknown-actor row, never as `main` ([walkthrough G](walkthroughs.md#g-unknown-actor-identity); AT-09).
- A metadata-only rebuild behaves as [event model §3](event-model.md#3-retained-facts-and-replay-guarantees) states: what retained facts support is rebuilt; content-dependent derivations are reported as unavailable ([walkthrough E](walkthroughs.md#e-metadata-only-rebuild)).
- A changed derivation keeps prior corrections. Reviewed revisions do not change; differences appear as discrepancies. Corrections that cannot be matched to the new derivation are shown for review, never dropped silently ([walkthrough H](walkthroughs.md#h-changed-derivation-with-a-prior-correction)).
- Replaying the same evidence yields identical totals. Re-import of observations with a dedup identity does not change totals. Records without a dedup identity that a re-read may duplicate stay flagged `unidentified` and visible ([event model §8](event-model.md#8-deduplication); [OQ-P-24](open-questions.md#product-questions)).
- Every reported minute and every cost figure can be traced to evidence or a user record, with its provenance class. Every counted minute carries its duration basis (AC-15).
- A collector outage shows as a gap, not as zero activity.
- Estimated cost is never labeled as spend. Provider-reported figures are labeled as their own kind. Unpriced usage is always visible.
- Correction burden and boundary error are measured on real days before any default suggestion policy or threshold is adopted.
