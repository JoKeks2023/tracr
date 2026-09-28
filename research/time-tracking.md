# Manual and automatic time together

Research date: **2026-09-28**. Professional-product evidence: [Timing and Kimai](SOURCES.md#m1--professional-time-entry-semantics). Candidate semantics below are **P**.

## Observed precedents

Timing treats a manual time entry as an explicit statement of the work performed; it overrides automatic app usage attribution for that period. This is a useful answer to overlapping automatic evidence and manual intent. It is not permission to destroy the underlying observations. [Timing](https://timingapp.com/help/time-entries)

Kimai separates begin/end, running/stopped state, project/activity, billability and exported status. It supports editing and protects exported entries through locks/permissions. These provide precedents for a worklog's lifecycle beyond a single stopwatch. Audit-log internals were not verified. [Kimai](https://www.kimai.org/documentation/timesheet.html)

## Candidate entities and precedence

Keep automatic observations, user-entered intervals and the approved worklog as separate layers. A manual timer is not an agent session and should work without Claude/Codex, Git or network access.

| Layer | Editable? | Candidate meaning |
|---|---|---|
| Source event | No; may be redacted/deleted under retention policy | What a source observed |
| Automatic segment | Recomputed; user override recorded separately | Inferred task and interval |
| Manual entry | Yes, revisioned | User assertion, timer or retroactive entry |
| Resolved worklog | Reviewable/finalizable | Chosen attribution used in effort reports/export |
| Export record | Append/reconcile, not silently replaced | What a remote system accepted |

Possible precedence: confirmed correction → manual entry → explicit task binding → inferred activity → unknown. This is an option for product design, not a final policy. Retain why one source won.

## Overlap cases that need a product rule

| Situation | Sensible candidate behavior |
|---|---|
| Manual A 09:00–10:00, detected A 09:10–09:45 | One 60-minute resolved interval; preserve both origins |
| Manual A 09:00–10:00, detected B 09:30–09:40 | Manual attribution initially wins; show disagreement for correction |
| Manual A and manual B overlap | Prompt for split/override or explicit weighted allocation |
| Automatic A and B overlap | Reconcile by evidence, mark ambiguous parts; never silently sum |
| Timer left overnight | Offer a correction based on last evidence; do not silently alter the user's record |
| Manual meeting without desktop activity | Count as user-asserted work if chosen policy permits |
| Offline timer and later Huly import refer to same work | Join through export/source identity rather than add twice |

Single-running-timer mode reduces accidental overlap, but multiple open timers with one active owner could support the intended workflow. Parallel billable timers should be an explicit business option; they are different from exclusive personal effort.

## Corrections, merging and audit

Candidate revision record: entry ID, prior revision, actor, timestamp, operation, old/new interval/attribution, reason, source links. Splitting preserves lineage; merging creates a new revision with all source references. “Undo” restores intent without editing raw observations. Deletion may tombstone the worklog while separately purging sensitive raw payload.

Preserve original timestamps and duration precision. Apply invoice rounding only at a declared export/report boundary. A rounded display must not rewrite measured seconds. Changing an hourly rate must not silently reprice an already finalized historical report.

## Clock and crash behavior

Persist timer start and status immediately; calculate elapsed time from durable boundaries rather than a UI counter. Use monotonic elapsed time for a live local timer and UTC instants for storage. On restart, show unresolved running timers with last-known evidence. Sleep handling and travel timezone handling need visible policies.

Use dropdowns for project, issue, provenance, rounding policy and other finite options, with an explicit “Other…” escape only when needed. Free text remains suitable for notes. This follows the user-supplied global instructions. [CONTEXT](SOURCES.md#context)

## Export implications

Huly's current `log_time` accepts hours and a description, not interval bounds or an idempotency key. The external worklog might therefore be less expressive than tracr's ledger. Keep local segment detail and a synchronization receipt; ambiguous write outcomes cannot be retried blindly. No live time entry was created. [Huly](huly.md)

Later tests: start/switch/stop, accidental double-click, crash before/after persistence, backdated edit, split/merge, cross-midnight entry, sleep, competing device edits, repeated export and remote response loss. These are design-validation scenarios, not tests of an implemented product.
