# tracr — CLAUDE.md

## Working mode

Claude is the primary implementation agent for tracr.

Keep working through the current Huly task / implementation task until:
- the task is actually done,
- a genuine blocker exists,
- or a product-level decision is required.

Do not pause merely because a sub-question is uncertain.
Resolve implementation details conservatively from the existing architecture,
document genuine ambiguities, and continue.

After meaningful milestones:
- run tests/checks,
- inspect git diff/status,
- update the task handoff/report,
- continue with the next logical implementation step when appropriate.

Do not wait for external review unless explicitly requested.
