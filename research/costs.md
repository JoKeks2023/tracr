# Tokens, API-equivalent estimates and paid costs

Research date: **2026-09-28**. Official rates are time-sensitive: [P1](SOURCES.md#p1--pricing). No invoices, subscriptions or paid-account data were accessed.

## Four separate monetary concepts

| Kind | Evidence | Meaning for tracr |
|---|---|---|
| Actual API cost | Provider billing/usage charge or reconciled invoice | Amount billed, with reconciliation scope and adjustments |
| Estimated API cost | Observed token/tool usage × applicable price rule | Counterfactual/list-price estimate; not an invoice |
| Subscription cost | User-entered or imported plan charge | Fixed/periodic expense, possibly including tax/discount |
| Allocated subscription cost | Explicit allocation formula | Analytical share of that expense, not a marginal token charge |

**P:** Store kind, source, currency, period, coverage, pricing revision and confidence with every amount. A locally recorded field called `cost_usd` remains provider/tool-reported until billing provenance is established. “Actual” must not be inferred from the field name.

## Token normalization

| Dimension | Why separate? | Evidence |
|---|---|---|
| Fresh input | Different price from cache reads | Provider pricing; T2/T3 calculators |
| Output | Different price and includes provider-specific subcategories | Provider pricing; T3 parser |
| Cache read | Can be a subset of total input | T3 normalization |
| Cache creation | Claude TTL tiers can differ | T2/T3 calculators |
| Reasoning output | Track as a component when reported | T3 Codex parser; do not add twice |
| Model/provider/service tier | Same display family can have distinct rates | T2/T3 provider handling |
| Tool charges | May be separate from tokens | Official pricing categories |

Sources: [T2](SOURCES.md#t2--claudoscope-native-macos), [T3](SOURCES.md#t3--ccusage), [pricing](SOURCES.md#p1--pricing). **P:** preserve original counts and their semantics; normalize into disjoint billable categories only when the provider definition supports it. Unknown cache/reasoning breakdown is not zero.

For a provider where cached input is included in input total, an illustrative formula is `(input_total − cached_input) × fresh_rate + cached_input × cache_rate + output_total × output_rate`. Reasoning already contained in output is not charged again. For Claude's disjoint input/cache categories, add each category once. Apply per-request tier rules before aggregation; daily totals cannot reconstruct which requests crossed a context threshold.

## What source inspection teaches

ccusage distinguishes stored-cost display from calculation and automatic fallback. Its Codex parser handles repeated totals and per-request deltas. Native Claudoscope deals with repeated message records and copied parent history. These are necessary accounting controls, but neither proves invoice equality. [T2](SOURCES.md#t2--claudoscope-native-macos), [T3](SOURCES.md#t3--ccusage)

qianhaoq's estimator assumes a 70/30 input/output mixture over one total; it cannot model actual cache usage. Treat it as a rough scenario, unsuitable as the only cost ledger. [T7](SOURCES.md#t7--codex-usage-sqlite-reader)

The newer zJay26 estimator uses integer nano-USD, checks multiplication overflow, applies explicit rounding and returns priced/unpriced token coverage with reasons. This is a concrete transparency pattern; the entire pricing catalogue was not validated against provider bills. [Estimator source](https://github.com/zJay26/codex-usage/blob/dc4e1a8d038385b33f1baeaf995422990cbcc710/internal/pricing/estimator.go)

## Pricing history candidates

**P:** Keep a versioned PriceRule with exact model/provider/tier, effective-from/to, fetched-at, source URL, original currency, token-unit scale and special conditions. Unknown effective dates remain unknown. A newly fetched table must not silently reprice past reports.

Maintain two views if useful: “estimated using prices applicable then” and “what this usage would cost now.” Preserve an immutable finalized-report price revision. Offline caches need age/source visibility and safe fallback to unpriced usage. Avoid guessing the price of a new model from a substring unless explicitly labeled a scenario.

Use decimal or scaled-integer arithmetic with declared rounding boundaries. Sum precise values before display rounding; retain original currency. Currency conversion needs rate source, date and method; tax/refunds/credits are separate adjustments. No exchange-rate or tax policy is selected here.

## Subscription allocation options

For a €100 period with project weights A=60, B=40, proportional allocation yields €60/€40. The weights could be confirmed effort, estimated API-equivalent usage, or user-assigned shares. This is arithmetic, not evidence that A consumed €60 of incremental cost.

| Weight | Useful interpretation | Limitation |
|---|---|---|
| Confirmed work time | Cost supporting development effort | Background agent-heavy work may look cheap |
| API-equivalent cost | Relative resource intensity | Relies on estimated rates and complete usage |
| Token count | Simple | Different models/cache categories are incomparable |
| Manual project shares | Explicit business intent | Requires maintenance |

Include unallocated/non-work usage and zero-activity periods. Allocated shares plus unallocated amount must equal the subscription charge after declared rounding. Do not add the API-equivalent estimate on top of the subscription expense as though both were paid. Distinguish extra purchased usage from included allowance.

## Historical effort/pricing analytics

**P:** Separate internal cost, labor-rate scenario, quoted price and realized revenue. Feature cost may combine confirmed effort × versioned labor rate plus explicitly allocated expenses. Record whether a feature is complete, partial or reopened. Comparisons need sample size, range and attribution coverage; one historical feature is not a reliable future estimate.

Later checks should cover duplicate streams, counter reset, mixed models, cache TTL, unknown model, missing categories, refunds, split billing periods, zero denominator, overflow and rounding. No billing integration or pricing engine was built.
