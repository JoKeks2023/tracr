# Architecture delivery report

Date: 2026-09-29. Documentation only: no product code.

## Created or changed

- **Created** [docs/architecture.md](../../architecture.md). It covers the 14 requested sections plus §0 (decisions), §15 (validation against v0.2) and "Implementation Start".
- **Created** this task folder (`00`–`05`).
- **Changed** [docs/README.md](../../README.md): one "Architecture" section and one task-record line. No other spec document was touched.

## Main decisions

AD-01 to AD-21, listed in [architecture §0](../../architecture.md#0-decisions-at-a-glance):

| Area | Decision |
|---|---|
| Stack | Rust workspace, SQLite ledger + derived store, Tauri 2 + React/TypeScript |
| Process | One user-session process |
| Ingestion | Transcript files as the primary channel, read only through `reconcile(source)` with a continuity proof |
| Derivation | Pure per-session and per-day units with explicit `as_of` |
| Corrections | Range and natural-key correction anchors; latest-per-field composition |
| Money | Kinds as separate types with no total |

## Deviations and scope notes (explicit)

- **Folder name.** It is `T-0003_architecture`, not the requested `T-0003-architecture`. This follows the repository convention, as in `T-0002_product-spec`.
- **0.9 defaults for open product questions.** The architecture sets labeled defaults for questions that code must answer: OQ-P-04, OQ-P-08, OQ-P-19, OQ-P-22 (partly), OQ-P-23, OQ-P-24 and OQ-P-25, plus provisional OQ-P-02 values. These are defaults for implementation. Product review can change them. The spec documents still list them as open, because v0.2 was not rewritten.
- **Codex subagent linkage.** Locally observed rollouts carry `parent_thread_id`. The architecture uses it only behind a fixture gate (OQ-I-06 stays open). Subagent *detection* is ungated, because that is the conservative direction.
- **Walkthrough H, k6.** In H, k6 is range-anchored and therefore never orphaned. The spec allows this, since it makes the orphan conditional on the identity approach. Orphans remain possible for corrections anchored to an agent segment.
- **Walkthrough G.** The spec's G shape (hook-only evidence) has no ingestion path until the spool channel (S4). S1 tests a rollout-only variant, G-r.
- **Actor `main`.** Main-chain records of both providers get actor `unknown` unless the manifest cites documented semantics (AD-21). This applies AT-09 strictly: transcript structure is C-level evidence, not documentation. Per-actor views therefore show an "unknown actor" row for main-chain activity. Session and provider totals are unchanged.
- **Local data inspection.** Only file listings and record key names or type values were read. No prompt, message or tool content was read.
- **Nothing else.** `research/` is unchanged. There was no commit, push, PR, Huly or GitHub write.
