# Research validation report

Date: 2026-09-28. Scope: documentation integrity, source traceability and worked-example arithmetic. No product runtime exists to validate.

## Checks and evidence

| Check | Result / method |
|---|---|
| Requested deliverables | All 18 Markdown documents present under research; exact filename-set assertion |
| Executive summary | Ten numbered answers, in the final level-two section of research/README.md |
| Local navigation | 170 relative Markdown file targets/heading anchors passed the ephemeral Python validator across 18 research and 6 task files |
| Markdown structure | Balanced fenced blocks and consistent column counts in 31 tables passed |
| Source traceability | Chapters link to official URLs, pinned source files or source-register entries; claims distinguish documented, inspected, live, inferred and unresolved evidence |
| Original repository preserved | Root README byte-compared with HEAD; tracked diff empty; additions restricted to research and this task handoff |
| Huly read path | Explicit Jokeks2023 workspace: project listing, TRACR detail, issue listing and teamspace listing succeeded; no write test |
| A→B→A example | Arithmetic check: 107 exclusive minutes versus 150 summed session-minutes |
| Concurrent gap example | Arithmetic check: 35 summed minutes versus 25-minute interval union |
| Threshold boundary | Checked 1,799 seconds included and 1,800 excluded under Timelog's inspected strict comparison |
| Repeated token fixture | Arithmetic check: cumulative increments total 3,300; naïve repeated last-usage sum totals 5,500, matching the reported #1288 fixture shape |
| Allocation conservation | Illustrative €100 × 60% + €100 × 40% equals €100 exactly using decimal arithmetic |
| Calendar boundary | Europe/Berlin 2026-03-29 to 2026-03-30 is one calendar day but 23 elapsed hours |

The arithmetic checks independently verify examples and invariants. They do **not** execute or certify the third-party applications or a tracr algorithm. The first navigation pass identified the then-unwritten validation report; the final pass includes this file.

## Not verified / next-phase evidence

- Imported project instructions and deployed custom Huly fork source were unavailable in the inspected context.
- No macOS/Windows collector, installer, tray app, permissions flow, hook installation or provider integration was run.
- No time entry, invoice, remote mutation, billing-account reconciliation or Huly/GitHub TRACR sync round-trip was tested.
- Parser inspection covered selected relevant functions, not exhaustive source audits; README-only and discovery-only findings remain marked.
- Source URLs were retrieved during research; no exhaustive fresh HTTP status sweep or visual Markdown-rendering audit was performed at handoff.
- No builds or product tests were appropriate for this documentation-only change. No commit or push performed.

Result: the research package is ready for review with explicit evidence limits. It is not a claim that tracr integrations or automatic-time accuracy have been implemented or runtime-validated.
