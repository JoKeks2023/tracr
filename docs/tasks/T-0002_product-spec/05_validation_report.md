# Specification validation report

Scope: documentation consistency. No product exists to run, so there is no runtime evidence. Nothing here certifies runtime behavior, provider compatibility, Huly semantics or OS parity.

## v0.1 (2026-09-28)

| Check | Method / result |
|---|---|
| Links and anchors | Ephemeral Python checker over docs/ and task files: all relative file targets and heading anchors (GitHub slug rules) resolve |
| Code fences | Balanced in all files |
| Open-question IDs | Every `OQ-*` referenced in the documents is defined in open-questions.md |
| Stack neutrality | grep for DB/framework names: only hits are Codex's internal `state_N.sqlite` as a *source*, and README's note about research wording |
| Terminology | Legacy research entity names appear only in concepts.md §6 mapping table |
| A→B→A arithmetic | Supported A 25+32=57, B 30, unresolved 7+13=20. Union 09:00–10:47 = 107. Session spans 115+58=173. After resolution A 64 + B 43 = 107. |
| Interleaved variant | Per-session 20+15=35 vs union 25 (matches research) |
| Allocation example | Illustrative €37.50 + €62.50 = €100.00 |
| Research untouched | `git status` shows only additions under docs/ |

**Correction (v0.2):** the v0.1 A→B→A and interleaved rows checked arithmetic only. Correct arithmetic did not validate the semantics. v0.1 presented 107 and 25 as human totals although 20 and 25 of those minutes were not established as work (review finding B-01). The v0.2 rows below replace them.

## v0.2 (2026-09-28)

| Check | Method / result |
|---|---|
| Links and anchors | Checker script (relative targets + GitHub-slug heading anchors) over all 29 Markdown files in `docs/` incl. task files: all relative file targets and heading anchors resolve (0 problems) |
| Code fences | Balanced in all files: yes, 0 unbalanced |
| Open-question IDs | Every referenced `OQ-*` is defined exactly once (50 defined: 27 product, 10 architecture, 13 integration) |
| Requirement IDs | Every referenced requirement ID (EV/AC/AT/TW/CO/IN/RP/PS/UI/PL) is defined exactly once in requirements.md |
| Semantic review | Two adversarial rounds with six lenses: B-01 regression, parallel sessions, M-01/M-02, M-03/M-04/M-05, money/platform/minor, consistency and preserved decisions. Every finding went to independent skeptics before a per-file fix. Round 1 (one skeptic per finding): 76 raised, 76 confirmed and fixed (6 major, 70 minor). Round 2 (two skeptics, both must confirm): 33 raised, 19 confirmed and fixed. Of the 14 split votes, one duplicated a confirmed finding. The other 13 were conservative clarifications, applied by hand (e.g. per-target sums with joint minutes, the walkthrough F restart branch, qualifying parent/child sessions). Final read-only pass (three readers, two skeptics each): the B-01 reader found 0 defects. The other readers found 2 minor defects: the attribution subject list, and the missing "immediately" in the Codex session-end claim. Both were confirmed and fixed. |
| B-01 sweep | Final grep and read pass over every "107", "25 min", "unattributed", "unknown", "upper bound" and totals table in `docs/`: no example, table, requirement or report layout counts unknown time. Every remaining "107" is labeled conditional on user assertions or as an upper bound. The v0.1 phrases ("only ownership changes", "counted as unattributed human time", "25 min under every option", "observed time", "one owner per instant", "exactly one class") no longer occur |
| A→B→A (semantic) | Unresolved: counted 87 (A 57 = 25+32, B 30), unknown 20 (7+13), not counted. Break: 87, non-work 20. User-confirmed: 107 (A 64 = 57+7, B 43 = 30+13), conditional on two user assertions. Partial: 94 (64+30), non-work 13. Evidence span 107 is an upper bound only. Session spans 115+58=173 never effort. |
| Interleaved (semantic) | No same-context bridge exists. 0 inferred, 25 unknown. The Timelog per-session sum of 35 overcounts, and the 25 union is an upper bound, not a total. |
| Manual timer variant | 25 + 40 + 32 = 97 counted, 5 + 5 = 10 unknown |
| Parallel sessions (walkthrough C) | Agent execution 38 + 37 + 59 = 134 min within 80 wall-clock minutes. Session spans 45 + 44 + 60 = 149. Human before review 25 + 27 = 52 counted, 10 + 3 = 13 unknown. After review 25 + 27 + 3 joint = 55, not 58. |
| Walkthrough G / H | Session B usage 0.6 + 0.2 = 0.8 M. H revision r1 = A 57 (25+7+25) + B 43 = 100, plus 7 non-work from the trim. |
| Allocation example | Illustrative €37.50 + €62.50 = €100.00. It partitions one charge and is not an additional expense. |
| Stack neutrality | No framework, database, shell, IPC, watcher technology, schedule, threshold, window policy or attribution algorithm is chosen: grep finds technology names only as provider sources (Codex `state_N.sqlite`) or in the README note on research wording |
| Research untouched | `git status --short research/` and `git diff research/` are empty |

Not verified: Mermaid diagrams were not rendered. Their syntax was reviewed by hand and by the reviewers only. The adversarial review is model-based. It raises confidence in internal consistency, not correctness against live systems.
