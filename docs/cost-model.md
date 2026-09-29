# Cost model

Status: **Draft specification v0.2 (2026-09-28)**. Evidence: [research/costs.md](../research/costs.md), pricing sources [P1](../research/SOURCES.md#p1--pricing), tool precedents T2/T3/T7/T8. No billing account, invoice or subscription data has been inspected. Every formula below is a **candidate**.

## 1. Monetary kinds

Money is described in three layers:

1. **Charge.** The real financial fact: money actually charged, e.g. an API bill line, an invoice, a subscription payment, extra purchased usage. A charge is recorded **once**, as an Expense ([concepts §4.5](concepts.md#45-money)).
2. **Semantic kind.** The classification of an amount. There are five kinds (table below).
3. **Analytical representation.** Allocations, per-target views and API-equivalent comparisons derived from charges or usage. They never create a new charge.

Every **amount record** has exactly one kind. A charge is recorded once. The *subscription cost* kind is never a record of its own: it exists only as a classification on a charge's actual-expense record. **Subscription cost** classifies a charge: the record of a subscription payment has kind *actual expense* and carries the classification *subscription cost*. It is never a second amount. An **allocation** partitions a charge. Its shares are not additional money.

**Principle:** kinds are never summed indiscriminately. Paid totals count each charge once. Estimated API cost and provider-reported figures are never added to paid totals. Allocated shares are never added to actual expense.

| Kind | What it is | Evidence required | Typical source |
|---|---|---|---|
| **Actual expense** | Any paid charge: an API bill line, an invoice line, a subscription payment, extra purchased usage. Recorded once. | Billing import or user-entered expense with reference | User entry; billing export (**deferred**) |
| **Estimated API cost** | Observed usage × an applicable Price Rule. A counterfactual list-price estimate. Not paid. | Token Usage + Price Rule | tracr calculation |
| **Provider-reported cost figure** | A cost number a provider or tool displays locally, e.g. status-line cost or a `cost_usd` field. Not billing truth. | Observation of that figure | Provider surface |
| **Subscription cost** | The classification of a paid charge as a subscription-plan charge, linked to its Subscription. A view of the same charge, never a second amount. | An actual expense linked to a Subscription | User classification of an entered or imported charge |
| **Allocated subscription cost** | Analytical shares of one subscription charge under an explicit allocation basis, plus an unallocated remainder (§5) | One subscription charge + allocation policy with its basis | tracr calculation |

Notes:

- A **provider-reported cost figure** is kept as its own kind. It is treated as an estimate from that provider, not as actual expense, until billing provenance is established. A field name containing "cost" proves nothing (research, costs).
- A Subscription's declared terms, e.g. "€100/month", are configuration, not a charge. A charge exists only when it is entered or imported.
- A single €100 subscription payment is **one** €100 actual expense, classified as subscription cost. It is not two €100 expenses. "Total actual expense" counts it once. A subscription-cost breakdown of that total never adds to it.
- **Principle:** the estimated API cost of subscription-covered usage is **never** presented as spend. It is labeled, e.g., *"API-equivalent estimate; covered by subscription"*.
- **Principle:** estimated API cost is never added on top of subscription cost as if both were paid.
- Extra purchased usage beyond an included allowance is its own charge: an actual expense, distinct from the subscription charge.
- **Phase:** the kinds apply in all phases. MVP displays estimated API cost and provider-reported figures, where an imported source contains them. Actual expense, subscription cost and allocation are Later ([CO-01](requirements.md#cost-co)).

## 2. Provenance of every amount

**Proposed:** each Cost Record carries:

```pseudo
CostRecord {                          // conceptual
  kind                                // §1
  classification?                     // subscription_cost: on an actual expense linked to a Subscription
  chargeRef?                          // the one charge (Expense) this amount is or partitions
  amount { value (decimal/scaled integer), currency, scale }
  temporalExtent                      // instant, or interval: service period, usage-delta extent, allocation period
  source                              // billing import id | user | price rule revision | provider observation
  pricingRevision?                    // estimated kinds
  allocationPolicy? + policyVersion? + allocationBasis?   // allocated kind (§5)
  coverage { pricedUsage, unpricedUsage, unidentifiedUsage, reasons[] }
  confidence / quality flags          // fallback_model, partial_categories, temporally_coarse, …
  derivedFrom[]                       // usage records, subscription, expense
}
```

- **Principle:** a paid total counts each charge once, identified by `chargeRef`. The subscription-cost classification is carried on the charge's own actual-expense record (`classification`), not on a separate record. Allocation shares that partition the charge carry its `chargeRef` and never add to it. A ledger can always tell whether two records describe one charge.
- `temporalExtent` is kept as recorded. It is never narrowed silently to one day (§6).

## 3. Estimation from usage

- Estimation runs at **request/usage-record granularity** before aggregation, because tier and context-length rules apply per request. Daily totals cannot reconstruct those thresholds (research, costs).
- Categories are priced according to the provider's definition. Where cached input is part of the input total, fresh input = total − cached. Where the provider reports disjoint categories, each is priced once. Reasoning tokens contained in output are not charged again.
- **Principle:** unknown categories are unknown. An unknown model or missing price makes the usage **unpriced**, never zero-cost, and it is reported in coverage ("12% of tokens unpriced: model X has no price rule").
- A model is never priced by substring similarity to a known model, unless the user explicitly chooses a labeled *scenario*.
- Arithmetic uses decimal or scaled integers with declared rounding. Values are summed before display rounding, and the original currency is kept (zJay26 precedent, **C**).

## 4. Pricing sources and price rules

**Principle:** no pricing source is universal truth.

- A **Pricing Source** is, for example, a snapshot of an official price page (with fetched-at and URL), a bundled catalogue (with version), or a user override.
- A **Price Rule** specifies provider, exact model, service tier, token category, unit scale, currency, effective-from/to (unknown allowed) and special conditions (context-length tiers, cache TTL tiers).
- Two views are possible (**Proposed**): *"estimated at prices applicable then"* and *"at current prices"*. Both are labeled.
- A newly fetched price table never silently reprices a **finalized** report. Finalized reports reference the pricing revision they used.
- Offline or stale pricing shows its age and source. Missing rules fall back to *unpriced*.

## 5. Subscriptions and allocation

A **Subscription** has a plan label, period, declared terms (charge amount, currency, tax/discount as entered), and optionally the providers and agents it covers. The declared terms are configuration (§1). Each payment is a charge: one actual expense, classified as subscription cost.

An **Allocation** partitions one period's charge across targets under an explicit **allocation basis**: a weight basis and an allocation period, recorded with the policy.

| Weight basis (candidates) | Interpretation | Limitation |
|---|---|---|
| Reviewed human work time | Expense supporting development effort | Agent-heavy background work looks cheap |
| Estimated API-equivalent usage | Relative resource intensity | Depends on price rules and usage completeness |
| Manual shares | Explicit business intent | Needs maintenance |
| Token counts | Simple | Categories/models are not comparable, so not recommended as the only basis |

**Principles for allocation:**

- **Principle ([CO-09](requirements.md#cost-co)):** assigning part of a periodic charge to a day, project or issue requires an explicit allocation basis. Without one, that part is **unallocated**. tracr never derives a share implicitly, e.g. by dividing a monthly charge by the days of the month.
- Allocated shares + **unallocated remainder** = the one charge, exactly, after declared rounding. Idle periods and non-work usage go to the remainder.
- Shares partition the charge. They are never added to actual expense. Each share carries the `chargeRef` of its charge (§2).
- A basis built on human time uses counted human time only. `unknown` minutes are never weights ([activity §5](activity-model.md#5-work-status-counting-is-separate-from-activity)). Joint minutes are never weighted in full for each of their targets. How a basis treats joint minutes is part of [OQ-P-10](open-questions.md#product-questions) (see also [OQ-P-27](open-questions.md#product-questions)).
- An allocation is arithmetic. It does not claim that target A *caused* its share as a marginal cost.
- Each allocation run records its policy, version, basis, inputs and period, and can be finalized.

The default allocation basis and period are **Unresolved** ([OQ-P-10](open-questions.md#product-questions)). Allocation is **not** in the MVP.

## 6. Association of costs

Each kind attaches to its own subject:

- **Estimated API cost** and **provider-reported figures** attach to **usage**, which already has provider identity. They reach wider contexts through the attribution of the usage subject.
- **Actual expenses** attach to **charges**. They reach projects or issues only through user assignment or allocation.
- **Allocated subscription cost** attaches to the **allocation targets** of its run.

| Target | Estimated API cost | Actual expense | Allocated subscription cost |
|---|---|---|---|
| Request / turn | Direct | Only if billing provides request detail (unknown) | n/a |
| Agent / subagent (actor) | Sum over its usage. Usage with actor `unknown` stays in an unknown-actor row, never assigned to the main agent ([AT-09](requirements.md#attribution-at)) | Rarely available | Only by allocation policy |
| Agent session | Sum over its usage, including unknown-actor usage | Rarely available | Only by allocation policy |
| Agent segment | Usage inside that agent-timeline segment | n/a | By policy |
| Project / issue | Via the attribution of the usage subject (turn, session, or the session's binding): confirmed and inferred shown separately; joint, ambiguous, conflicting and unattributed shown as such | Via user-assigned expense attribution | By policy |
| Time period (day/week/month) | By the usage record's temporal extent. An instant belongs to its day. An extent spanning a day boundary or a coverage gap is not silently assigned to one day ([OQ-P-23](open-questions.md#product-questions)) | By charge date or service period (declared). A service period longer than the viewed period is not split across its days or weeks: a day's share of the charge is not established without an explicit allocation basis and stays unallocated ([CO-09](requirements.md#cost-co)) | By allocation period. A day's share only under an explicit daily basis (CO-09) |

**Principle:** human-timeline intervals are never priced by minutes. Counted human time does not move money.

**Principle:** cost reports show attributed, joint, ambiguous, conflicting and unattributed amounts. Unpriced usage is shown separately, never as zero.

**Concurrent sessions.** Usage and estimated cost of overlapping sessions are each counted in full. Each session's usage is its own record. Only human time is reconciled onto one timeline ([activity §9](activity-model.md#9-concurrent-agent-sessions-and-the-human-timeline)).

**Independent of human-time corrections ([AT-10](requirements.md#attribution-at)).** Usage and cost attribution follow the usage subject. Operations on human time (manual entries, confirm as work, mark break, trim, split) never reattribute agent sessions, agent segments, usage or cost records ([time §3](time-and-worklog.md#3-two-precedence-axes-and-operation-scope)). Example: the user records a manual meeting on TRACR-12 while a background agent works on SERV-7. The agent's usage and estimated cost stay attributed to SERV-7. They change only through an operation whose subject is that usage record or its session/worktree (binding, explicit correction).

**Multi-target usage.** Agent usage with joint attribution stays joint in cost views, or is allocated by an explicit policy. Usage for one unknown target among several stays ambiguous. There is no automatic equal split ([attribution §7](attribution.md#7-multiple-targets-joint-contexts-and-allocation)). How usage and cost of a session with several issue bindings over time are apportioned is **Unresolved** ([OQ-P-17](open-questions.md#product-questions)).

**Temporally coarse usage.** Some usage records cover an interval, not an instant: a cumulative-counter delta between two snapshots, or a session-level total ([event model §6](event-model.md#6-usage-semantics)). Such a record keeps its temporal extent ([EV-14](requirements.md#evidence-and-events-ev)). A delta spanning midnight or a coverage gap is not silently assigned to its snapshot day or receipt day. Its estimated cost inherits the same extent. How such records are reported per day is **Unresolved** ([OQ-P-23](open-questions.md#product-questions)).

**Unidentified usage.** Some usage records lack a dedup identity, or their session is unknown. They are kept separate and flagged, never collapsed by similarity ([event model §8](event-model.md#8-deduplication)). Cost reports show their share and estimated cost on their own line. Whether unidentified usage enters counted usage and cost totals, an uncertainty band, or both is **Unresolved** ([OQ-P-24](open-questions.md#product-questions)). Independently, actor identity is capability-limited: usage of a known session with actor `unknown` keeps provider and session totals valid and appears as an unknown-actor row ([AT-09](requirements.md#attribution-at); [walkthroughs G](walkthroughs.md#g-unknown-actor-identity)).

## 7. Worked illustration

A day with 2.0 M tokens from agent sessions attributed to TRACR-12. The user has a Subscription with declared terms €100/month covering that agent. One €100 payment for the month has been entered (expense entry is Later, CO-07).

| Line | Kind | Value | Label |
|---|---|---|---|
| Estimated API cost (priced 95% of tokens) | Estimated API cost | e.g. $4.20 | *API-equivalent estimate; covered by subscription* |
| Unpriced usage | — (coverage) | 5% of tokens (model X) | *no price rule; unpriced, not zero* |
| Subscription payment (month) | Actual expense, classified subscription cost | €100.00 | *paid; one charge, recorded once* |
| Day's share of the €100 charge | — | not established | *unallocated: no allocation basis* |

The values are illustrative only. Actual spend for the day is **not** $4.20. The day's usage was covered by the €100 monthly charge. The day's share of that charge is **not established** without an allocation with an explicit basis, which is not in the MVP. Until then it is *unallocated*, not €100/30 ([CO-09](requirements.md#cost-co)). Total actual expense for the month is €100.00. The charge appears once. Its subscription-cost classification is a label on that line, not a second line.

**Later-phase illustration: allocation.** An allocation run with the explicit basis *reviewed time, month* partitions the one €100 charge. The basis is chosen for illustration only. The default is **Unresolved** ([OQ-P-10](open-questions.md#product-questions)).

| Line | Kind | Value | Label |
|---|---|---|---|
| Allocated to TRACR (basis: reviewed time, month) | Allocated subscription cost | e.g. €37.50 | *share of the €100 charge; not a marginal cost* |
| Unallocated remainder | Allocated subscription cost (remainder) | e.g. €62.50 | *share of the same charge* |
| Sum of shares | — | €37.50 + €62.50 = €100.00 | *partitions the one charge; never added to actual expense* |

Month-level shares do not assign a share to this day. Adding the shares to the €100 actual expense would count the charge twice.

## 8. Deferred

Billing-account imports, invoice reconciliation, currency conversion policy, tax/refund/credit handling, labor-rate scenarios and feature-cost analytics (research, costs: "historical effort/pricing analytics") are **deferred**.
