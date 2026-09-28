# Gaps and tracr's potential differentiation

Research date: **2026-09-28**. This is a bounded comparison of the inspected tools, not proof that no product in the market has any listed feature. Evidence: [comparison](comparison.md), [tools](tools.md).

| Gap | What existing evidence covers | What tracr would need to establish |
|---|---|---|
| Concurrent Claude/Codex human effort | Parsers list sessions/usage; Timelog measures per-session gaps | Global interval arbitration, missing-data semantics and correction UX |
| GitHub + Huly correlation | Upstream Huly sync stores external identity | Readable deployed mappings and one canonical relationship model |
| Huly issue-level time | MCP advertises log_time; upstream has report entities | Idempotent export, remote entry readback and revision reconciliation |
| Automatic issue attribution | Branch regex and PR projections | Validated candidates, explicit user binding, rejection history |
| Manual + automatic work | Timing/Kimai offer useful entry semantics | One resolved effort ledger integrated with agent evidence |
| Development diary | Analytics/transcript viewers and daily reports | Evidence-backed narrative, unfinished work and user notes |
| Subscription expense allocation | API-equivalent estimates dominate inspected tools | Separate expense ledger, allocation policy, unallocated remainder |
| Historical feature pricing | Usage and time are usually separate | Reviewed effort + rate history + delivery outcome/uncertainty |
| Contribution graph and streaks | Existing calendar and habit patterns | Qualifying development policy; consistent dates and source coverage |
| Multi-agent history | Connectors and parent-context dedup exist | Stable actor/parent identity without double-counting human or tokens |
| Cross-platform desktop reliability | Native Mac viewer, portable CLIs and shell frameworks | Windows/macOS sensor parity, tray lifecycle, recovery and resource measurements |

## Why not simply combine or fork existing tools?

**P:** Combining outputs is attractive but their units differ. A provider's running time, editor coding duration, session lifetime, user-confirmed work and billed hours cannot be added directly. Neither can a request-level estimate be equated with an account subscription charge. The integration value lies in reconciliation and provenance, not another dashboard.

Forking an agent viewer could accelerate transcript browsing while delaying the essential worklog model. A macOS-native fork does not provide Windows parity; a CLI report parser does not supply timer corrections. Study focused components and fixtures, retain an independent product model, and review licenses before any later reuse. No source code has been copied into the repository.

## Smallest sensible MVP candidate

1. Local desktop diary with manual start/stop and retroactive corrections.
2. Claude and Codex session/usage import with source-version and coverage indicators.
3. Explicit project/issue binding; automatic time suggested conservatively and reviewed.
4. Exclusive human-time totals plus separate agent usage/estimated API cost.
5. Daily timeline/export and read-only Git/GitHub/Huly context where verified.

Huly time writeback is conditional on the unresolved receipt/idempotency/readback semantics. Streaks, heatmaps, subscription allocation and inferred issue ownership can follow after the underlying ledger is trusted. This is a sequencing recommendation, not an implementation authorization or final feature commitment.

## Hardest problem

The hardest problem is identifying and reconciling human attention across concurrent agent/editor work with sparse, asynchronous evidence while keeping corrections understandable. File watching and token arithmetic are solvable components; neither solves that inference problem. Exact task boundaries may be unknowable without explicit user interaction. tracr should be useful while admitting that limit.

## Evidence still needed before design sign-off

- Locate the intentionally imported CLAUDE.md and workflow/MCP documentation; neither the inspected repo nor the correct Huly workspace supplied it.
- Identify the deployed Huly fork/revision and inspect actual handlers/return payloads; verify TRACR's GitHub mapping.
- Test relevant provider versions and macOS/Windows sensor permission behavior with consented fixtures.
- Establish acceptable correction burden and idle policy with real workflows.
- Decide retention, travel timezone policy and what counts as development without rewarding unattended agent usage.

These unresolved questions limit design confidence; they do not justify inventing API capabilities or prematurely selecting a framework.
