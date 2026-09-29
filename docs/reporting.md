# Daily diary and reporting

Status: **Draft specification v0.2 (2026-09-28)**. Evidence: [research/reporting.md](../research/reporting.md), [research/streaks.md](../research/streaks.md), [research/contribution-graph.md](../research/contribution-graph.md).

## 1. Provenance classes in reports

**Principle:** every figure or statement in a report carries one of these classes, visibly ([RP-02](requirements.md#reporting-rp)):

| Class | Examples |
|---|---|
| **Observed** | Source-reported instants and counts: commit created at 14:02; 1.2 M tokens reported; engagement instant (prompt at 09:12); lock event. An observed instant has no duration. |
| **Derived** | Deduplicated usage totals; interval unions; estimated API cost with pricing revision; counted totals computed from resolved intervals |
| **Inferred** | Automatic human-timeline intervals (`inferred_continuity`); inferred attribution; window-policy suggestions, labeled *estimated (policy)* |
| **User-asserted** | Manual timer 09:00–10:00, labeled *user-asserted (timer)* (duration basis `timer_measured`); retroactive manual entries; corrections; confirm-as-work; confirmed attributions; diary notes |
| **Generated** | Narrative text produced from a fact bundle *(later)* |
| **Unknown** | Unknown work windows (work status `unknown`); coverage gaps, including unrecovered ingestion gaps; unpriced usage; unknown actor |

- **Principle:** a class is never upgraded. Observed instants are not durations. Inferred minutes are never shown as observed or measured ([AC-15](requirements.md#activity-ac)).
- A timer measures the elapsed interval between durable start/stop actions. Classifying that interval as work is a user assertion. A timer is therefore reported as *user-asserted (timer)*.
- A result labeled *estimated (policy)* is a suggestion. It is never merged silently into counted totals ([OQ-P-18](open-questions.md#product-questions)).
- A derived total names the classes of its parts ([Mixed provenance](#mixed-provenance)).

## 2. Daily report content

| Section | Content | MVP? |
|---|---|---|
| Timeline | Human timeline (intervals with work status: work, unknown windows, non-work; conflicts; manual entries), one agent timeline per session (overlapping), device idle/lock, coverage gaps | **MVP** |
| Human time | Counted total, split user-asserted vs inferred (inferred is provisional until reviewed). Per target: exclusive, joint, unattributed, ambiguous/conflicting. Not counted, shown separately: unknown minutes, non-work. Reviewed vs provisional. | **MVP** |
| Agent activity | Every session individually, with overlapping timelines: provider, agent/actor (`unknown` actor shown as such), turns, agent execution time (explicitly not human time), usage per session ([RP-09](requirements.md#reporting-rp)) | **MVP** |
| Usage and cost | Tokens by provider/model/category; estimated API cost with coverage; unpriced usage; provider-reported figures, where an imported source contains them, labeled as their own kind and never shown as spend; unidentified and temporally coarse usage flagged ([cost §6](cost-model.md#6-association-of-costs)) | **MVP** |
| Delivery | Commits (local Git) in the day | **MVP** |
| Remote context | GitHub PRs/issues and Huly issues referenced by the day's attributions, read-only | **MVP**, where verified |
| Diary notes | User-authored notes per day/item | **MVP** |
| Subscription allocation | Allocated share per project for the period | Later |
| Huly activity | Status changes, comments on attributed issues | Later |
| Streak / contribution | Whether the day qualifies; contribution values per metric | Later |
| Generated narrative | Summary from a minimal fact bundle, each assertion linked to evidence | Later |

Wording follows the research: *"PR opened/merged"*, not *"feature finished"*; silence is not completion.

### Human-time totals and parallel sessions

**Principle ([RP-07](requirements.md#reporting-rp)):** the counted human total is reconciled on the one human timeline. Each instant counts at most once, however many sessions or contexts are active ([AC-02](requirements.md#activity-ac)).

```text
counted human total = Σ per-target exclusive + joint + unattributed + ambiguous/conflicting
```

- A user-stated allocation contributes its weighted shares to the targets' exclusive figures.
- Per-target exclusive figures, allocation shares and joint minutes counted once never sum above the counted total. Joint minutes appear in each target's view as "joint with …". They are never added in full to each target's exclusive figure, and joint figures shown under several targets are never summed across targets.
- Unknown minutes are shown next to the counted total, never inside it. Non-work is shown separately.
- The evidence span is shown only as an upper bound, labeled as such. It is never a total.
- Agent execution time and session spans appear in the agent section. They are never summed into human time.

Example: parallel sessions ([walkthroughs C](walkthroughs.md#c-parallel-sessions)). Sessions A (TRACR-12), B (server#41) and C ("Personal docs") overlap. In review, the user confirms 09:25–09:28 as work with joint attribution {TRACR-12, server#41}, no split, and leaves 08:50–09:00 unresolved.

| Line | Before review | After review |
|---|---:|---:|
| TRACR-12, exclusive | 25 | 25 |
| server#41, exclusive | 27 | 27 |
| Joint {TRACR-12, server#41} | 0 | 3 |
| Unattributed / ambiguous / conflicting | 0 | 0 |
| **Counted human total** | **52** (inferred) | **55** (52 inferred + 3 user-asserted) |
| Unknown, not counted | 13 (10 + 3 conflicting evidence) | 10 |
| Non-work | 0 | 0 |
| Human evidence span 08:50–09:55 (upper bound only) | 65 | 65 |

- Per-target view after review: TRACR-12 25 + 3 joint with server#41; server#41 27 + 3 joint with TRACR-12. Reading these as 28 + 30 = **58** is wrong: it counts 09:25–09:28 twice.
- Agent section, same day: execution C 59 + A 38 + B 37 = 134 minutes within 80 minutes of wall clock (08:50–10:10). Session spans sum to 149. Neither figure is human time.
- "Personal docs" (session C): 0 human minutes. Its 59 agent minutes and its usage are fully logged.

### Mixed provenance

A counted interval or worklog entry can combine sub-intervals with different duration bases ([RP-08](requirements.md#reporting-rp)). Reports explain it by minutes per duration basis. They show each endpoint's boundary basis (`observed`, `bounded`, `policy`, `user`; [activity §4](activity-model.md#4-boundaries-and-duration-basis)).

- **Principle:** inferred minutes are never upgraded to observed or measured. A total that mixes bases shows the minutes of each basis.
- An endpoint placed by idle trimming shows `policy`, anchored at the observed last-input instant. It is a proposal, not an observed end of work.

Example: A→B→A, user-confirmed outcome ([walkthroughs B](walkthroughs.md#b-a--b--a-with-a-possible-break)). The user confirmed 09:25–09:32 as A (reading output) and 10:02–10:15 as B (reviewing result).

```text
PDF Export (A): 64 min counted = 57 inferred + 7 user-asserted
  09:00–09:25   25  inferred (inferred_continuity)   boundaries: observed / observed
  09:25–09:32    7  user-asserted (confirmed work)   boundaries: observed / observed
  10:15–10:47   32  inferred (inferred_continuity)   boundaries: observed / observed
Server Fix (B): 43 min counted = 30 inferred + 13 user-asserted
  09:32–10:02   30  inferred (inferred_continuity)   boundaries: observed / observed
  10:02–10:15   13  user-asserted (confirmed work)   boundaries: observed / observed
Counted human total: 107 = 87 inferred + 20 user-asserted
```

The 107 is conditional on the user's confirmations. Without them, the day shows 87 inferred minutes and 20 unknown minutes, not counted. The evidence span 09:00–10:47 (107 min) does not by itself prove 107 minutes of work.

## 3. Day boundaries, provisional data, finalization

- Days are local calendar dates in the reporting timezone. Intervals are split at midnight. Point events (commits, PR merges) belong to the day of their timestamp in the reporting timezone, except GitHub-supplied contribution dates, which keep GitHub's date and are labeled. Usage records whose temporal extent spans midnight are not silently assigned to one day ([OQ-P-23](open-questions.md#product-questions)).
- A day is **provisional** while the ingestion watermark has not passed it, while it contains unreviewed drafts, while its ingestion completeness is unverified (reconciliation pending, [EV-11](requirements.md#evidence-and-events-ev)), or while it contains coverage gaps that recovery has not yet resolved ([EV-12](requirements.md#evidence-and-events-ev)). A gap that recovery cannot close stays visible with its reason in every revision. The report never claims complete ingestion for it.
- A **finalized** daily report is a preserved revision recording reporting timezone, rule versions, source watermark, coverage (including completeness state and gaps), pricing revision and generation time. Late data never alters a finalized revision. It updates the draft projection, and any difference from the finalized revision is shown as a visible discrepancy ([time and worklog §6](time-and-worklog.md#6-reconciliation-and-late-evidence)). A new revision is created only by an explicit user action ([time and worklog §5](time-and-worklog.md#5-worklog-lifecycle)). Finalized revisions are never silently altered (research, reporting).
- **Principle ([RP-05](requirements.md#reporting-rp)):** any weekly or monthly total is the sum of the same daily semantics, never a separate algorithm. Unknown and non-work minutes stay outside the counted total at every level. Weekly and monthly views themselves are **Later**.

## 4. Quality footer

Every report shows: collectors active and gaps; coverage completeness and recovery gaps with reasons ([event model §10](event-model.md#10-ingestion-completeness-and-recovery)); human time: counted minutes (user-asserted / inferred), joint minutes, shares attributed / ambiguous / conflicting / unattributed, unknown minutes and non-work (both not counted); count of corrections; usage completeness (unidentified records, unknown-actor usage, temporally coarse records, parse failures); cost kinds present and unpriced share; evidence retention state (periods whose payload is `never_stored`, `redacted` or `deleted`, content-dependent derivations unavailable, records `non_rebuildable` or with tombstoned evidence links; [event model §3](event-model.md#3-retained-facts-and-replay-guarantees)). These indicators survive export (CSV/JSON candidates; format **Unresolved**).

## 5. Export

**MVP:** a local export of the daily timeline and worklog with stable entity IDs, original precision, provenance classes and the quality footer. Unknown minutes are never exported as work ([AC-12](requirements.md#activity-ac)). Joint time stays joint; tracr does not invent a split for single-target formats ([OQ-P-27](open-questions.md#product-questions)). Sharing or publishing is always a separate, deliberate user action. Private paths, issue titles and prompt-derived text are not included in shared artifacts by default.

## 6. Streaks (later)

- A streak counts consecutive **qualifying days**. Qualification is a named, versioned **policy** over resolved daily facts. Candidates: minimum reviewed human minutes; delivery event; combination; user-confirmed day. The choice is **Unresolved** ([OQ-P-12](open-questions.md#product-questions)).
- Qualification uses counted human minutes only. Unknown minutes never count as work toward a qualifying day.
- Day states: `qualified`, `not_qualified`, `unknown` (coverage gap), optional `excused`. An unknown day is provisional, not a failure.
- Agent activity alone (e.g. unattended overnight runs) never qualifies a day.
- Adjacency uses calendar dates, not 86,400-second arithmetic. A policy change never silently rewrites history; recomputation is labeled.

## 7. Contribution calendar (later)

- One cell per day, with a **selectable metric** from a finite list: reviewed human minutes, commits, PRs, completed issues, confirmed entries. There is no unexplained composite score.
- Cells distinguish zero, unknown (coverage gap), future and out-of-range.
- The intensity scale (fixed bins, quantiles, log) is recorded with the metric and version.
- The GitHub contribution view, if shown, uses GitHub's returned values and dates, labeled as GitHub's metric.
- Accessibility: information is never carried by color alone; keyboard/touch access to details; a text/table alternative.

## 8. Consistency requirement

**Principle:** the day's counted human total, the diary, streak qualification and the contribution value for "minutes" all derive from the same resolved worklog (work status and attribution) under the same timezone and policy revision. Unknown minutes are excluded from all of them. None of them counts unknown or non-work time as work.
