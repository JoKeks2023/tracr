# Completion criteria

## v0.1 (2026-09-28)

- The 13 specification files exist under `docs/` and are indexed in `docs/README.md`.
- Observation → Normalized Event → Activity → Attribution → Worklog is defined consistently across the documents.
- Claude Code and Codex are mapped symmetrically, with asymmetries recorded as capability differences.
- Huly references are workspace-scoped everywhere. Huly is a context provider, not tracr's store.
- Concurrent sessions never add up to human time (worked example with arithmetic).
- Actual expense, estimated API cost and allocated subscription cost remain distinct.
- Raw evidence is recoverable subject to explicit retention. Corrections are append-only.
- The MVP is substantially smaller than the long-term scope, with deferral reasons.
- Every open decision is labeled and listed in `open-questions.md`.
- No source or research files are modified.

## v0.2 revision (2026-09-28)

- B-01: unknown work status is never counted. `unknown` and `unattributed` are separate states. The A→B→A example shows the break, user-confirmed (conditional) and unresolved outcomes. The interleaved example no longer claims 25 counted minutes.
- Parallel agent sessions are first-class: agent timelines overlap and stay individually visible. Human time is reconciled once. A human interval may reference several contexts without an invented split.
- M-01: two precedence axes with explicit operation scope. Bindings never create human time. Human-time corrections never reattribute agent usage or cost.
- M-02 to M-07 and N-01 to N-04 are addressed in the affected documents.
- The review's missing open questions are registered without being answered.
- Walkthroughs A to H exist in `docs/walkthroughs.md`.
- All internal links, anchors, code fences, open-question IDs and requirement IDs resolve.
- A final pass over the full package finds no reintroduction of the B-01 mistake.
- `research/` unchanged. No commit, push or remote write.
