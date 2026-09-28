# Best practices for tracr

Research date: **2026-09-28**. “Strong evidence” means supported by the cited implementations/docs; it does not mean tracr's behavior has been validated. Recommendations are research conclusions, not architecture decisions.

## Strong evidence

| Practice | Evidence | Consequence for tracr |
|---|---|---|
| Deduplicate usage at provider request/message identity | T2, T3, T5, T6 | Do not sum every transcript line or repeated snapshot |
| Separate source data and rebuildable analytics | T2 cache, T4 projections, T5 scanner | Reparse after format changes without losing user corrections |
| Make provider semantics explicit | T3 adapters, T4 connector contract | Portable vocabulary plus provider-specific extensions/capabilities |
| Distinguish presence, agent work and elapsed session | A1/A2 sensing; C1/C2 lifecycle | Separate human and agent intervals; expose uncertainty |
| Recover after missed watcher events | T2 overflow/rescan and replacement handling | Watch notifications trigger reconciliation, not sole truth |
| Give manual entries clear precedence/revision rules | Timing; Kimai exported-record semantics | Preserve evidence and reviewed worklog separately |
| Respect remote API budgets and sync identity | GitHub official limits/webhook docs | Backoff, caching, dedup and reconciliation |
| Model price estimates separately from paid charges | Official pricing plus tool estimators | Never label API-equivalent subscription usage as a bill |

Evidence: [tools](tools.md), [activity](activity-tracking.md), [time](time-tracking.md), [GitHub](github.md), [costs](costs.md).

## Promising

- **Metadata-first local event history:** enough provenance for corrections while limiting prompt retention. Tradeoff: less replay after payload deletion. [Candidate model](data-model.md)
- **Confidence with reasons:** show manual binding, editor evidence, recent interaction, conflict or missing sensor. Test whether users can correct it quickly; do not invent calibrated probabilities. [Activity](activity-tracking.md)
- **Optional PR/external-link projections:** normalize relations without forcing every provider to implement GitHub. [T4](SOURCES.md#t4--claudescope-multi-agent-explorer)
- **Priced/unpriced coverage and scaled arithmetic:** zJay26's estimator makes missing pricing visible. Extend the principle to source/attribution coverage. [Costs](costs.md)
- **Finalized daily snapshots over editable history:** stable shared reports alongside revisable analytics. Do not treat finalized user content as disposable cache. [Reporting](reporting.md)
- **A local read-only MCP reporting surface:** could let coding agents query their history, with explicit scope and redaction. Native Claudoscope claims this pattern; tracr's implementation and need remain undecided. [T2](SOURCES.md#t2--claudoscope-native-macos)
- **GitHub/Huly identity bridge:** inspect actual external references, then confirm ambiguous mappings. This is more reliable than issue-title similarity. [Huly](huly.md)

## Risky / avoid

| Approach | Why it fails |
|---|---|
| Open session = working time | Inactive terminals and background work overcount |
| Sum independent session durations | Double-counts concurrent intervals |
| Hard-coded gap threshold presented as truth | Bridges breaks or discards real quiet work |
| Exact switch times inferred from sparse logs | Observation cannot identify the boundary |
| Global OS input means focused on one agent | Input is not session-local attribution |
| Screen capture or key-content logging | Outside brief; unnecessary collection of sensitive content |
| Universal transcript/DB schema assumption | Agent formats and internal database versions change |
| Read provider state DB as a daily cost ledger | Lifetime totals and archived filters lose temporal detail |
| Unknown model priced silently as a familiar model | False precision and potentially large cost error |
| Subscription tokens × API price called actual spend | Confuses a counterfactual with a paid expense |
| Retry Huly log_time after ambiguous response | No advertised idempotency key; duplicate hours possible |
| Public localhost server without access controls | “Local” does not imply safe from other local/browser clients |
| Daily snapshots as the only stored evidence | Corrections and rule changes become irreproducible |
| Fork a broad agent viewer as the default product base | Inherits unrelated scope and provider assumptions |

Source-backed failure examples: [T1](SOURCES.md#t1--claude-code-timelog), [T7](SOURCES.md#t7--codex-usage-sqlite-reader); API boundaries: [C1/C2](SOURCES.md#c1--claude-interfaces), [HULY-LIVE](SOURCES.md#huly-live). Privacy and product-base judgments are **P** derived from the brief and those boundaries.

## Maintainability gates for the next phase

Keep sanitized fixtures by producer version; test incomplete lines, repeated usage, resume/fork, counter resets and deleted/archived sources. Parser failure must create coverage diagnostics. Keep actor/session/request identities distinct. Build deterministic replay tests for corrections and cross-session ownership. Before choosing a shell, measure background resource use and OS permission behavior on macOS and Windows.

Any future runtime claim needs an executed path, not only a successful build. This package provides research evidence and acceptance scenarios; it does not claim implemented integration QA. No product architecture or technology has been locked in.
