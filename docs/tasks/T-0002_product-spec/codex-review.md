# Codex review — Product + Data Specification v0.1

Date: 2026-09-28. Review basis: the working-tree specification, including its untracked files, and the repository research snapshot dated 2026-09-27–28.

## 1. Executive assessment

**Not ready to serve as the architecture baseline yet.** The specification is substantially faithful to the research and has sound product boundaries. However, its central worked example turns uncertainty about whether work happened into counted human time. That contradicts the stated separation between activity and attribution and must be corrected before architecture.

Findings: **1 Blocker, 7 Major, 4 Minor**. These are documentation findings, not evidence of implemented defects. Recommendations below request semantic corrections and explicit decision boundaries, not a redesigned product or an implementation.

Read in full: all 18 `research/` documents, including `README.md`'s Executive Summary and `SOURCES.md`; all 14 top-level `docs/` documents; the six T-0002 handoff files; and the six T-0001 handoff files. Source citations below refer to those local documents and their named sections; selected line ranges identify the reviewed snapshot. External APIs and third-party implementations were not revalidated: this is a review **against the supplied research**, not a new research phase.

Only this review file is delivered. No specification/research/source edits, stack choices, remote writes, commits or pushes.

## 2. Blockers

### B-01 — Unknown activity is silently promoted to human effort

- **Severity:** Blocker.
- **Affected documents / concept:** [activity-model.md](../../activity-model.md) §§5, 9; [time-and-worklog.md](../../time-and-worklog.md) §7; unknown activity versus unattributed work; concurrent-session reconciliation.
- **Specification evidence:** Activity lines 174–180 identify 7 + 13 minutes as unknown, explicitly allowing a break, then state “Total human time 107 min.” Line 207 defines “Leave unresolved” as “counted as unattributed human time.” Time/worklog lines 112–116 say the total remains 107 and “Only ownership changes.” Yet activity §9 offers **break** and **exclude** as resolutions.
- **Research evidence:** [research/activity-tracking.md](../../../research/activity-tracking.md), “The A→B→A example” describes 107 minutes as the **desired** result requiring ownership evidence; its invariants, lines 88–94, impose an upper bound, not equality. Its signal table says a prompt does not establish continuous attention. [research/README.md](../../../research/README.md), Executive Summary Q6, says precise boundaries may be unknowable.
- **Explanation:** In the spec's candidate derivation, 57 + 30 = **87 policy-inferred minutes**, with **20 additional minutes of unknown activity/ownership**. The 107-minute enclosing span does not prove 107 minutes of effort. Resolving both windows as breaks would leave 87, whereas the documented reading/review confirmation yields 107. Neither outcome is established beforehand. Even the 87 minutes depend on the explicitly assumed continuation rule; they are not measured attention.
- **Related contradiction:** Activity §9's interleaved variant asserts 25 minutes under every option. But §5 forbids bridging same-context engagement across other-context engagement. With A at 09:00/09:20 and B at 09:10/09:25, both proposed bridges fail. The research's 35-versus-25 calculation demonstrates overcounting in Timelog's candidate slices; it does not validate 25 human minutes or continuous conflicting engagement during 09:10–09:20.
- **Recommended action:** Distinguish **known/asserted work with unknown target**, **unknown whether work happened**, and **explicitly policy-estimated work** throughout examples and totals. Rework both examples under their stated rules; include a break outcome and retain the user-confirmed 64 + 43 = 107 outcome as conditional. Do not select a window policy in this review. Acceptance must test exclusion of non-work, not merely a total below the enclosing span.

## 3. Major findings

### M-01 — The two precedence orders are sound in principle but not consistently specified

- **Severity:** Major.
- **Affected documents / concept:** [time-and-worklog.md](../../time-and-worklog.md) §3; [attribution.md](../../attribution.md) §§1, 5–7; [open-questions.md](../../open-questions.md); correction scope and independent dimensions.
- **Evidence:** Attribution §5 gives correction → manual entry → binding → inference → unattributed. Time §3 instead gives a five-step “whether … counts, and as what” order that **includes binding** at step 3, despite saying it creates no time. Both use a latest-correction rule without specifying whether operations supersede only the fields they change. Research [time-tracking.md](../../../research/time-tracking.md), “Candidate entities and precedence,” offers one candidate ladder; [github.md](../../../research/github.md), “Attribution ladder,” ranks issue evidence, not time existence.
- **Explanation:** Keep the two questions separate. A binding can identify unattended agent work without creating human minutes; a manual meeting can establish minutes without agent evidence. A reattribution following an exclusion should not implicitly re-enable time. Further, attribution §1 includes Token Usage and Agent Sessions as subjects: applying a coincident manual human entry to those subjects could incorrectly move background-agent usage/cost to the meeting's issue.
- **Recommended action:** State the time-counting order as **correction → manual entry → inferred activity → unknown**, with bindings referenced only on the attribution axis. Specify operation/subject scope and the invariant that human-time overrides do not automatically reattribute independent agent usage. Register overlapping corrections, undo/composition and competing session/worktree bindings as unresolved where necessary. No conflict-resolution algorithm is selected here. Update README's explanation of how the research ladders were interpreted.

### M-02 — Observed endpoints are mislabeled as observed work duration

- **Severity:** Major.
- **Affected documents / concept:** [time-and-worklog.md](../../time-and-worklog.md) §1; [activity-model.md](../../activity-model.md) §§4–6, 9; [reporting.md](../../reporting.md) §1; evidence versus inference.
- **Evidence:** Time §1 calls a span between engagement points and an interval closed by idle “Observed time.” Activity §4 calls the core “fully supported”; §6 gives `observed_interaction` to a core, while §5 correctly labels bridged time `inferred_continuity`. Its example mixes the two grades. Reporting calls a manual timer “Measured” but manual entries “User-asserted.” Research [activity-tracking.md](../../../research/activity-tracking.md), signal and confidence sections, distinguishes engagement instants, inferred continuity and user confirmation.
- **Explanation:** Two observed prompts do not measure the attention between them. Observing last input establishes an input timestamp; choosing it as the end of work is a policy, especially for quiet reading. A timer measures an elapsed interval while its interpretation as work remains a user assertion. The present labels can communicate greater certainty than the evidence supports.
- **Recommended action:** Separate timestamp/boundary provenance from the basis for counting a duration. Keep bridging explicitly inferred and idle trimming explicitly proposed. Clarify how mixed provenance is displayed without upgrading inferred minutes to measured work. This does not require choosing confidence thresholds or numeric scores.

### M-03 — Replay and preservation guarantees do not cover metadata-only operation

- **Severity:** Major.
- **Affected documents / concept:** [event-model.md](../../event-model.md) §§1–3; [concepts.md](../../concepts.md) §§2, 4; [requirements.md](../../requirements.md) EV-03/08; durable facts versus rebuildable records.
- **Evidence:** EV-08 and MVP M3 disable payload content by default. The observation envelope lists identity, time, hash and an optional payload, but not the retained usage/context fact body. EV-03 says normalized events are rebuildable from observations. Event §1 acknowledges replay loss when payload is deleted, while the envelope also permits `never_stored`. Concepts §2 says every entity has exactly one class and Worklog is the sole hybrid, although Repository, Attribution, Cost Record, Diary Entry and finalized Daily Report entries already mix origins or preservation states. Research [data-model.md](../../../research/data-model.md), lines 15–25, explicitly permits normalized authoritative facts and rejects full replay after deletion; [reporting.md](../../../research/reporting.md), “Raw events versus snapshots,” requires preserving allowed minimal facts.
- **Explanation:** A hash and source locator cannot reconstruct usage once the original log is gone. The document does not establish which non-content facts remain durable when payload was never retained, or which rebuild operations remain possible. The record-class contradiction matters because that class is said to determine deletion/rebuild behavior. The existing deletion caveat is good, but insufficient for the default operating mode.
- **Recommended action:** Define the minimum retained evidence/facts and the guarantee for `present`, `redacted`, `deleted` and `never_stored`; distinguish renormalization from recomputing activity/reports from retained facts. Make preservation depend unambiguously on origin/revision/finalization. Cover corrections, confirmed links, manual mappings, finalized reports and allocations. Keep OQ-A-04's storage alternatives open; no database or event-store design is required.

### M-04 — Watcher recovery is a research conclusion missing from the behavioral contract

- **Severity:** Major.
- **Affected documents / concept:** [event-model.md](../../event-model.md) §§2, 7–9; [integrations.md](../../integrations.md) §2; [requirements.md](../../requirements.md) EV; ingestion completeness.
- **Evidence:** The spec specifies idempotence, backfill, partial-line rereading and coverage reporting, but no explicit recovery obligation after missed notifications, overflow or atomic file replacement. A `source_rotated` event alone does not establish recovery. Research [activity-tracking.md](../../../research/activity-tracking.md), line 93, calls for periodic reconciliation and recovery after overflow/restart/rotation/replacement; [tools.md](../../../research/tools.md) §2 gives the source-inspected precedent; [README.md](../../../research/README.md) lists it as a material design finding.
- **Explanation:** Replaying known records safely does not ensure discovering records that a watcher missed. An apparently healthy collector can otherwise report incomplete usage or activity as complete. This is separate from OQ-A-07's open choice of primary channel and reconciliation mechanics.
- **Recommended action:** Carry forward a technology-neutral recovery/completeness requirement, visible unrecoverable gaps, and later acceptance cases for dropped notifications, partial writes, rotation/replacement and restart. Also preserve the research's requirement that archived/resumed Codex history remains eligible for backfill ([research/codex.md](../../../research/codex.md), “Observed internal behavior”). Do not prescribe a watcher implementation or schedule.

### M-05 — Mandatory confirmed actor attribution contradicts the known identity gap

- **Severity:** Major.
- **Affected documents / concept:** [attribution.md](../../attribution.md) §10; [concepts.md](../../concepts.md) §4.1; [event-model.md](../../event-model.md) §§3, 4.1; actor/session identity and provider symmetry.
- **Evidence:** Attribution §10 says **every** Token Usage record and agent segment is attributed to its Agent and Provider by observation (`confirmed`). Event §3 allows an absent `agentRef`; §4.1 and OQ-I-06 acknowledge unestablished Codex child identity when hooks carry a parent session ID. Research [codex.md](../../../research/codex.md), capability map, requires preserving child identity and explicitly limits field availability.
- **Explanation:** Provider/session evidence can be known while the particular child actor is unknown. An unconditional actor guarantee invites assigning child usage to `main` or fabricating a child ID. The core is otherwise symmetric; this is an availability-contract contradiction, not a general Claude-first design.
- **Recommended action:** Qualify confirmed attribution by available evidence. Preserve unknown/ambiguous actor identity and associated coverage without inventing parentage or losing valid provider/session totals. Leave how Codex child identity is obtained open under OQ-I-06.

### M-06 — Monetary kinds overlap despite the exactly-one-kind rule

- **Severity:** Major.
- **Affected documents / concept:** [cost-model.md](../../cost-model.md) §§1, 6–7; [concepts.md](../../concepts.md) §4.5; [requirements.md](../../requirements.md) CO-01; expense classification.
- **Evidence:** Cost §1 requires exactly one kind, then includes a subscription charge in both **Actual expense** and **Subscription cost**. §7 classifies its €100 subscription charge as Actual expense. §6 opens “Costs attach to usage,” although its table correctly associates actual expenses and subscription allocations independently. Research [costs.md](../../../research/costs.md) separates Actual **API** cost from Subscription cost and warns against adding estimates to paid charges.
- **Explanation:** Broadening “actual API cost” to “actual expense” makes subscription cost both a separate kind and a member of another kind without defining the relationship. A later ledger cannot tell whether two records describe one charge. Also, assigning the monthly charge a daily share is not established merely by saying daily spend “is part of” €100.
- **Recommended action:** Clarify the relationship between a paid charge, its semantic category and analytical representations so a charge is recorded/counted once. Align the example and usage-association wording; leave daily allocation unknown without a declared basis. Preserve all five distinctions, especially provider-reported figures. Their addition is supported by the research's explicit `cost_usd` caveat and is **not** the problem.

### M-07 — Product platform scope is blurred with framework choice

- **Severity:** Major.
- **Affected documents / concept:** [mvp.md](../../mvp.md) §3; [product.md](../../product.md) §§1, 3; [open-questions.md](../../open-questions.md) OQ-A-06; target operating systems.
- **Evidence:** Research [README.md](../../../research/README.md) line 5 and [desktop.md](../../../research/desktop.md) frame the product as macOS/Windows; their uncertainty concerns shell selection and demonstrated parity. MVP §3 correctly defers parity **claims**, but adds “The platform decision is open.” Product does not restate the two-OS target; OQ-A-06 concerns architecture, not a product decision to drop or sequence an OS.
- **Explanation:** Refusing unsupported parity claims is correct. It does not make the target operating systems undecided. Architecture needs to know whether both remain required and whether MVP delivery is staged. This is a scope ambiguity, not evidence that either OS has deliberately been removed.
- **Recommended action:** Preserve the research's macOS/Windows product target or explicitly record any deliberate scope change. Separately identify MVP platform sequencing if unresolved and retain per-OS validation gates. Do not select a shell/framework or demand local heavy builds.

## 4. Minor findings

### N-01 — MVP phase tags disagree in a few places

- **Severity:** Minor.
- **Affected documents / concept:** [requirements.md](../../requirements.md) AT-07/RP-05/CO-01; [mvp.md](../../mvp.md) M7/M9/M10; phase traceability.
- **Evidence:** M7 uses PR closing links in MVP while AT-07 places PR↔issue External Links in Later. RP-05 places weekly/monthly totals in MVP, while M10 names only daily reporting. CO-01 explicitly includes provider-reported figures in MVP while M9 names usage/estimated cost only. Research [gaps.md](../../../research/gaps.md), MVP candidate, is a sequencing proposal centered on a daily ledger.
- **Recommended action:** Align the capability list and phase tags, distinguishing a general future consistency invariant from a feature required in MVP. No feature expansion is recommended.

### N-02 — Open-question wording partly contradicts declared defaults

- **Severity:** Minor.
- **Affected documents / concept:** [open-questions.md](../../open-questions.md) OQ-P-04/07/09; [requirements.md](../../requirements.md) EV-08; [attribution.md](../../attribution.md) §3; [time-and-worklog.md](../../time-and-worklog.md) §5.
- **Evidence:** OQ-P-09 leaves “off vs short window” open while EV-08 and MVP M3 explicitly choose off by default. OQ-P-04 asks whether inferred attribution counts in reports although attribution §3 already says yes, labeled inferred. OQ-P-07 asks who can reopen/with what reason although time §5 specifies user reopen and required reason. Research [data-model.md](../../../research/data-model.md) leaves retention options open; moving beyond those options is legitimate if labeled consistently.
- **Recommended action:** Distinguish a proposed MVP default from genuinely undecided policy. Clarify whether reviewing time also confirms its issue attribution; export eligibility may remain open. Do not resolve those questions here.

### N-03 — Two evidence claims are broader than the supplied research

- **Severity:** Minor.
- **Affected documents / concept:** [event-model.md](../../event-model.md) §4.1; [product.md](../../product.md) §§1–2; evidence strength.
- **Evidence:** The Claude compaction cell says “compaction seen in fixtures,” but the research records fixture testing as future work, not completed validation. OQ-I-07 combines Claude compaction with ccusage PR #1805, which the research discusses as a Codex accounting lead ([research/tools.md](../../../research/tools.md) §3). Product §1's “existing tools keep apart” and §2's claims about automatic/manual trackers exceed the bounded comparison carefully stated in product §6 and [research/comparison.md](../../../research/comparison.md).
- **Recommended action:** Remove or source the fixture claim, separate provider-specific compaction leads, and scope market statements to inspected tools. Do not infer unsupported provider capabilities.

### N-04 — Setup inspection needs a more precise privacy description

- **Severity:** Minor.
- **Affected document / concept:** [project-and-agent-setup.md](../../project-and-agent-setup.md) §§3–4; metadata versus bounded content inspection.
- **Evidence:** The principle says inspection reads “metadata and file presence,” while proposed checks read import/reference statements and configured server names. Research [desktop.md](../../../research/desktop.md), privacy section, requires limited access and protected credentials; the setup chapter itself admits this feature needs separate research.
- **Recommended action:** Describe the narrow configuration/instruction content required for these checks and what is excluded. Keep diagnostics, remediation and Canon deferred; this is not permission to broaden collection or implement them.

## 5. Research/spec discrepancies

| Decision or omission | Assessment | Evidence / action |
|---|---|---|
| Separate activity, attribution and reviewed worklog | Faithful and useful | Research data-model/time-tracking; preserve, correct B-01/M-01/M-02 |
| Fifth kind: provider-reported cost | Supported deliberate extension, Informational | Research costs explicitly leaves local `cost_usd` provider/tool-reported; preserve distinct provenance, fix M-06 |
| Huly writeback wholly outside MVP | Justified conservative narrowing, Informational | Research huly/gaps make it conditional on unverified semantics; no need to add it back |
| Categorical grades and continuation/window candidates | Legitimate proposals, not validated algorithms | Research activity; retain uncertainty and OQ-P-01/02/03/14, fix overstated examples |
| Setup diagnostics and Canon placeholder | Explicit later product extension, Informational | Setup acknowledges lack of research; no false research-backed implementation claim; retain deferral, fix N-04 |
| Watcher recovery and archived-history eligibility | Material conclusion incompletely carried forward | M-04 |
| Both desktop OSs versus open platform decision | Scope ambiguity | M-07 |
| Optional minimal content and bounded replay | Research position preserved in intention, incomplete default contract | M-03/N-02 |

The research's algorithms, entity shapes, retention durations and desktop candidates were proposals. The spec need not copy them literally. Findings above concern contradictions, missing guarantees or overstated evidence, not differences alone.

## 6. Internal consistency findings

| Boundary | Review conclusion |
|---|---|
| Observation → Normalized Event | Source/receipt timestamps, native identity and versioned derivation are separated correctly. Metadata-only replay is incomplete (M-03). |
| Normalized Event → Activity | Agent output does not establish attention; provider counters remain evidence. Examples and observed-duration labels undermine this (B-01/M-02). |
| Activity → Attribution | Structurally separate; unknown activity must not become merely unattributed work (B-01). |
| Attribution → Worklog | Reviewed history and manual overrides are well protected. Counting and attribution need distinct scoped precedence (M-01). |
| Derived records → durable user records | Append-only corrections and orphan visibility are good. The record-class policy has multiple undocumented hybrids (M-03). |
| External mirrors → published records | Remote state remains external; permission loss is not deletion; export intent/outcome is separate and per target. Preserve these boundaries. |
| Usage → costs | Unknown price is not zero; categories and price versions are retained; money is not human time. Expense taxonomy needs M-06. |

The model **can** express concurrent agents, quiet reading, uncertain boundaries, manual work without an agent, and unattended generation. The defect is in accounting/labeling some of those states, not the absence of lanes or manual records. Both examples being Claude-only is not itself a core asymmetry; a future mixed-provider acceptance example would demonstrate the intended shared rules.

## 7. Strong decisions worth preserving

All entries here are **Informational**; recommended action is to preserve the decision and its evidence limits.

- **Three clocks and global human reconciliation:** product §7, concepts §5 and AC-01/02/03 carry forward research activity-tracking. Background agents may overlap freely; human totals may not be sums of sessions.
- **Two independent precedence questions:** attribution §5 correctly asks “what for?” separately from whether time exists. Keep this separation when applying M-01.
- **Provider-specific accounting under a common vocabulary:** event §§4.1, 5, 7 and integration manifests preserve Claude streaming/copied-context behavior, Codex cumulative counters, hosted-tool gaps, nullable transcript availability and App Server's limited observation scope. Research Claude/Codex/tools supports these distinctions. No universal transcript schema or passive desktop-wide subscription is assumed.
- **Huly as external context:** concepts §4.3 and attribution §2 retain instance/workspace identity, distinct tracr Project mappings and Huly issue mirrors. Integrations §3 avoids dependence on a host's MCP namespace. Research huly supports that boundary; advertised `log_time` is not verified readback/idempotency. No writeback enters MVP.
- **Reviewed and published history:** time §§5–6, 9 and reporting §3 preserve revisions, discrepancies, remote receipts and unknown outcomes instead of blind retries. Research time/reporting/huly supports the policy.
- **Honest pricing:** cost §§2–5 and CO-02/03/05 retain currency, price revision, coverage, unknown rates, category overlap rules and API-equivalent labeling. Neither local provider figures nor estimates become billing truth.
- **Privacy:** PS-01/02/03/05 and EV-08 preserve metadata-first scope, optional prompt/tool content, deletion and exclusion of screen/keystroke-content surveillance. Corrections do not erase original evidence; user-directed privacy deletion remains a separate operation.
- **A meaningfully smaller MVP:** manual work, both providers, explicit bindings, review, daily export and read-only context remain useful without allocation, streaks, calendar, generated narrative, setup diagnostics, additional attention sensors or complex automation. OS idle/lock/suspend is explicitly included, not an accidental extra sensor. Hook installation is necessary collection setup, distinct from deferred general project diagnostics.

## 8. Open questions that are correctly unresolved

Do not turn these into errors merely because a solution could be proposed:

- **OQ-P-01/02/03/05/14:** calibrated confidence, thresholds, window policy, exact attribution algorithm and default treatment of quiet work. Fix the examples without selecting values or algorithms.
- **OQ-P-06 and OQ-A-03:** rejection scope and identity/remapping across derivation changes. The spec already identifies these, so deterministic IDs are not a settled architectural mandate. Same-input ID stability alone does not solve changed-input corrections; orphan visibility is an appropriate fallback to retain.
- **OQ-A-01/02/04/05/06/08/09:** reconciliation representation, persistence, desktop/process architecture and transport/security design. Clarify the required behavior without choosing a framework, database or IPC technology.
- **OQ-A-07:** live/backfill primary channels, duplicate reconciliation and late-event windows. These can remain architecture questions after the completeness requirements in M-04 are restored.
- **OQ-I-01–06/08/10–12:** Huly handler/source/response/mapping evidence, Codex child identity, server observation scope, OS permissions, hook preservation and missing imported instructions. The review has not supplied new live evidence for any of them.
- **OQ-P-07/08/10–13/15–17:** export-finalization policy, travel zones, allocation, multi-device/team scope, streak/calendar semantics, Canon, handoffs and time-varying cost attribution. Deferred capabilities need not be designed to approve the core.

Canon remains undefined. A neutral placeholder in a deferred chapter is appropriate, not a reason to invent a governance system. Missing imported context remains an explicit sign-off dependency, not proof that the current specification violates unknown instructions.

## 9. Open questions that are missing or need sharper boundaries

These additions would record uncertainty, not resolve it:

| Missing or incomplete question | Evidence and why it matters | Suggested disposition |
|---|---|---|
| When work itself is unknown, what may appear in a counted total? How is this distinct from known work with unknown issue? | B-01; research activity distinguishes observation from inference | Clarify the invariant before architecture; retain window policy as open |
| How do corrections compose by field/subject, and how do equal-priority bindings conflict? | M-01; OQ-A-03 addresses identity, not operation precedence | Add explicit product question and counterexamples; no silent last-write semantics |
| What is retained/replayable when content was never stored, and how do deletion obligations interact with finalized records/backups? | M-03; research data-model retention vocabulary includes backups; PS-05 lists payload/indexes/controlled exports | Define guarantees; keep retention periods/storage mechanism open |
| How are temporally coarse usage/cost records reported? | Event §5 allows session cumulative totals; cost §6 groups by occurred-at. Research tools §6 rejects lifetime totals as daily usage | A correct delta may span midnight or a coverage gap; do not silently assign all usage to its last snapshot or receipt day. Add temporal-coverage semantics without choosing allocation |
| What happens to unidentified usage in counted versus uncertain totals? | Event §7 preserves unidentified records; research tools §5 warns that retaining all can also duplicate usage | State uncertainty/accounting requirements; keep provider-specific election mechanics open |
| What is the manual timer's behavior through sleep, wall-clock changes and simultaneous starts? | Research time-tracking explicitly discusses monotonic elapsed time and open timer modes; spec preserves durable starts but does not settle these cases | Record product policies and validation obligations; no timer architecture choice |
| Is MVP delivery staged by OS while the product remains macOS/Windows? | M-07 | Separate target platforms from framework selection and demonstrated parity |

The first three intersect required semantic corrections. The remainder can remain explicitly tracked questions; they do not require premature implementation design.

## 10. Recommended corrections before architecture

1. **Resolve B-01 first:** correct totals and interleaved derivation; show a break outcome, a confirmed-work outcome and a clearly labeled unresolved state. Update the T-0002 validation report: correct arithmetic alone did not validate the semantics.
2. **Align the core contracts:** address M-01 through M-03 together so counting, attribution, evidence provenance and durable user intent remain separate through a rebuild.
3. **Restore ingestion/identity guarantees:** address M-04/M-05 with capability-limited behavior and recovery acceptance cases; do not manufacture unknown actor identities.
4. **Clarify money and platform scope:** address M-06/M-07 without adding billing integrations or selecting technology.
5. **Reconcile phase tags, defaults and evidence wording:** address N-01 through N-04 and register the missing questions above. Deferred research remains deferred.
6. **Re-review the corrected text:** trace one manual-only day, sparse A→B→A including a break, interleaved mixed-provider activity, a metadata-only rebuild, changed derivation with correction, and an unidentified child-usage record. These are specification walkthroughs, not runtime tests.

Documentation review checks performed here: complete package read, cross-document contradiction tracing, and independent arithmetic for 57 + 30 = 87, 7 + 13 = 20, conditional 87 + 20 = 107, confirmed 64 + 43 = 107, and 20 + 15 = 35 versus a 25-minute candidate union. These checks establish only the stated arithmetic and review conclusions. No product exists here to execute; no runtime behavior, provider compatibility, Huly mutation or OS parity is certified.

## 11. Final readiness assessment

**Research fidelity: strong overall, with material gaps. Internal consistency: not yet sufficient. Architecture readiness: blocked by B-01; M-01–M-07 should be corrected before adopting this as the baseline.**

The research-backed boundaries are worth keeping. No product redesign is needed. The next pass should correct the identified semantics and explicitly preserve the remaining open choices. A corrected specification can then support architecture work without first solving every deferred integration, calibration or reporting question.
