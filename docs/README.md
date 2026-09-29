# tracr documentation

## Product + data specification (v0.2 draft, 2026-09-28)

The specification of tracr, derived from the reviewed research package in [research/](../research/README.md). It is intended for review (e.g. by Codex) before architecture and implementation work. **No technology stack, framework, database, desktop shell or UI architecture is chosen here.**

**v0.2 revision.** v0.2 applies the [Codex review of v0.1](tasks/T-0002_product-spec/codex-review.md) (1 Blocker, 7 Major, 4 Minor findings). The central correction: when it is unknown whether work happened, that time is never counted as human work, and `unknown` is not `unattributed`. Counting (work status) is now an explicit layer between activity and attribution. Parallel agent sessions are first-class. Finding-by-finding status: [v0.2 revision report](tasks/T-0002_product-spec/06_v0.2_revision_report.md).

| Document | Contents |
|---|---|
| [product.md](product.md) | What tracr is and is not, user problem, boundary, central workflow, principles |
| [concepts.md](concepts.md) | Processing chain with the counting layer, record origin and lifecycle, identifiers, entity catalogue, three clocks, concurrent sessions and contexts, research-term mapping |
| [event-model.md](event-model.md) | Observation envelope, retained facts and replay guarantees, normalized events, Claude/Codex mapping, usage semantics, deduplication, ingestion completeness and recovery, coverage |
| [activity-model.md](activity-model.md) | Agent timelines and the human timeline, evidence signals, boundary and duration basis, work status, continuation/idle, grades, invariants, A→B→A worked example |
| [attribution.md](attribution.md) | Context targets, attribution states and shapes (including joint), evidence ladder, precedence, rejection memory, Huly workspace scoping, agent/provider attribution |
| [time-and-worklog.md](time-and-worklog.md) | Time quantities, two precedence axes and operation scope, reconciliation cases, worklog lifecycle, concurrency example, timers, export constraints |
| [cost-model.md](cost-model.md) | Charges, monetary kinds and analytical representations, provenance, estimation, pricing sources, subscription allocation, cost association |
| [integrations.md](integrations.md) | Adapter roles, per-integration capabilities/stability/unavailability, transport-independent Huly port |
| [project-and-agent-setup.md](project-and-agent-setup.md) | Later-phase setup diagnostics: facts vs diagnostics vs actions; Canon left undefined |
| [reporting.md](reporting.md) | Daily diary/report content, provenance classes, human-time totals with parallel sessions, finalization, streaks, contribution calendar |
| [walkthroughs.md](walkthroughs.md) | Specification walkthroughs: manual-only work, A→B→A with a possible break, parallel sessions, mixed providers, metadata-only rebuild, missed watcher notification, unknown actor identity, changed derivation with a prior correction |
| [mvp.md](mvp.md) | MVP scope, deferred capabilities and reasons, platform target and delivery staging, exit criteria |
| [requirements.md](requirements.md) | Requirement IDs with phase tags |
| [open-questions.md](open-questions.md) | Unresolved product, architecture and integration questions (OQ-P / OQ-A / OQ-I), with declared defaults marked |

Suggested reading order: product → concepts → event model → activity model → attribution → time and worklog → cost model → integrations → walkthroughs → mvp → open questions.

The core chain in one line:

**Observation** (evidence) → **Normalized Event** (derived) → **Activity** (agent timelines + human timeline) → **Work status** (counting) → **Attribution** (target) → **Worklog** (reviewed) → Export Record.

Counting and attribution are independent axes ([concepts §1](concepts.md#1-the-processing-chain)).

## Architecture (tracr 0.9, 2026-09-29)

[architecture.md](architecture.md) turns this specification into the tracr 0.9 architecture. It covers the stack and storage choice, components, ingestion and recovery, parallel sessions, identity and correction composition, usage and cost, platform split, acceptance-test structure, the first vertical slice and the implementation start. It resolves the architecture questions (OQ-A-*) and sets labeled 0.9 defaults for the few open product questions that code needs answered. It does not change the specification's semantics. Task record: [tasks/T-0003_architecture](tasks/T-0003_architecture/00_brief.md).

## Status labels

| Label | Meaning |
|---|---|
| **Principle** | Binding rule carried over from the research findings and the product brief. It changes only through explicit review. |
| **Proposed** | Recommended specification, awaiting review |
| **Declared default** | A default this spec states for the MVP. It can be changed by review. It is not an open question. |
| **Candidate approach** | One of several viable options. The choice is open. |
| **Unresolved** | A decision is needed. Listed in [open-questions.md](open-questions.md). |
| **Deferred** | Intentionally outside the current scope/phase |

Evidence labels **D/C/L/R/P/U** (documented, source-inspected, live, maintainer claim, proposal, unresolved) are the research package's; see [research/SOURCES.md](../research/SOURCES.md).

## Notes on the research base

Points in the research that needed interpretation or later review. The research itself was not modified.

1. **Precedence ladders answer two different questions.** [research/time-tracking.md](../research/time-tracking.md) offers one candidate ladder: confirmed correction → manual entry → explicit task binding → inferred activity → unknown. [research/github.md](../research/github.md) ranks issue evidence and calls *user binding* the highest attribution intent. v0.1 kept the binding step in the time ladder. v0.2 interprets the time-tracking ladder as the **counting axis without the binding step** (correction → manual entry → inferred activity → unknown), because a binding never creates human minutes. The github ladder is issue evidence for the **attribution axis** (correction → manual entry → binding → inference → unattributed). See [time and worklog §3](time-and-worklog.md#3-two-precedence-axes-and-operation-scope) and [attribution §5](attribution.md#5-precedence). How corrections compose and how competing bindings are resolved stay open (OQ-P-19, OQ-P-20). Reviewers should confirm this reading.
2. **Huly MCP naming is host-dependent.** The research documents `mcp__huly_joris__*`. The Claude Code session that wrote this spec exposes the same tools as `mcp__huly__*`. The research's "exact inventory" is therefore tied to one host configuration. The spec keeps the Huly port transport-independent.
3. **Provider-reported cost figures.** [research/costs.md](../research/costs.md) defines four monetary kinds but separately says a `cost_usd` field "remains provider/tool-reported". The spec adds that as a fifth kind to keep it distinct ([cost §1](cost-model.md#1-monetary-kinds)). This is a deliberate extension. The spec also broadens the research's *Actual API cost* (provider billing) to *actual expense*: any paid charge, including subscription payments and extra purchased usage. The same section separates the real charge (recorded once), its semantic kind and analytical representations. The research's *subscription cost* therefore classifies that one actual expense and is never a second amount.
4. **Huly writeback timing.** The research's MVP candidate makes Huly writeback *conditional* on verified semantics. This spec is stricter and defers it out of the MVP entirely, because none of those semantics have been verified yet.
5. **Codex subagent identity.** [research/codex.md](../research/codex.md) states that subagent hooks carry the parent session ID, yet requires preserving the child ID. Where the child ID comes from is not established (OQ-I-06). Until it is, the actor of such records is `unknown` (AT-09).
6. **Storage leaning.** [research/desktop.md](../research/desktop.md) phrases some needs in SQLite terms ("Core owns schema") while stating that no stack is chosen. This spec keeps persistence open (OQ-A-05).
7. **Imported instructions still missing.** The research recorded that the intended imported CLAUDE.md/workflow/MCP documentation was absent. It is still absent: the repository has no `CLAUDE.md` or `AGENTS.md` (OQ-I-12). "Canon" is not defined anywhere available (OQ-P-13).
8. **The A→B→A 107 minutes are a desired result.** [research/activity-tracking.md](../research/activity-tracking.md) presents 107 minutes as the *desired* exclusive human total. Counting 107 requires evidence that both switch windows were work (the counting axis). Which context each window served is a separate attribution question and adds no minutes (TW-09): the research's per-owner split (A 64, B 43) additionally needs attribution of those windows. With only the sparse evidence of the spec's example, v0.2 treats the unresolved windows (7 + 13 minutes) as `unknown`: visible, not counted. The inferred total is 87 minutes. 107 results only when the user confirms both windows as work ([activity model §11](activity-model.md#11-worked-example-a--b--a-with-sparse-evidence), [walkthrough B](walkthroughs.md#b-a--b--a-with-a-possible-break)). Likewise, the research's Timelog calculation (35 vs 25 minutes) shows overcounting. It does not validate 25 human minutes.

## Operational task records

- [tasks/T-0001_research](tasks/T-0001_research/00_brief.md): research phase
- [tasks/T-0002_product-spec](tasks/T-0002_product-spec/00_brief.md): this specification, including the [Codex review](tasks/T-0002_product-spec/codex-review.md) and the [v0.2 revision report](tasks/T-0002_product-spec/06_v0.2_revision_report.md)
- [tasks/T-0003_architecture](tasks/T-0003_architecture/00_brief.md): the tracr 0.9 architecture
