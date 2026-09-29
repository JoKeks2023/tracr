# T-0003 — tracr 0.9 architecture

Date: 2026-09-29. Request: move from specification to implementation. Codex's final v0.2 gate review returned `VERDICT: PROCEED`, with no blocker or major finding left. Create the architecture for **tracr 0.9** so that implementation can start right after. Move fast: make practical, defensible decisions; do not reopen settled product decisions or open a new research phase.

Required coverage: system boundaries, data flow, storage, live ingestion and backfill, parallel sessions, identity and remapping (including correction composition), activity/counting/attribution/worklog, usage and cost, Git/GitHub/Huly context, desktop/platform (macOS first, Windows later), privacy, acceptance test structure for walkthroughs A–H, the first vertical slice, the extension path, a final validation, and an "Implementation Start" section.

Deliverable: [docs/architecture.md](../../architecture.md). No product implementation, no change to `research/`, no rewrite of v0.2.

Note on naming: the request names the folder `T-0003-architecture`. The repository convention (and the global task convention) is `T-xxxx_<shortname>`, as in `T-0002_product-spec`, so this folder is `T-0003_architecture`.
