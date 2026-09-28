# Contribution graph and calendar heatmap

Research date: **2026-09-28**. Primary evidence: [G2](SOURCES.md#g2--contributions). The visual metaphor is useful; GitHub's contribution rules are not automatically tracr's work rules.

## GitHub behavior and data

GitHub contributions include several event categories subject to eligibility rules; commit credit depends on identity/repository/branch criteria. Private contributions may expose counts without details. The detailed reference uses commit timestamp timezone for commits and browser/API timezone for issue/PR creation, while the overview summarizes contributions in UTC. Preserve GitHub-supplied calendar dates rather than recreating them with a blanket timezone assumption. [Detailed reference](https://docs.github.com/en/account-and-profile/reference/profile-contributions-reference), [overview](https://docs.github.com/en/account-and-profile/concepts/contributions-on-your-profile)

GraphQL exposes calendar totals, colors, weeks/months and day records including date/count/intensity-related fields. Use returned intensity for a faithful GitHub view; do not assume fixed public count thresholds. A tracr work heatmap needs its own explicitly named metric. [Users schema](https://docs.github.com/en/graphql/reference/users)

## Candidate contribution measures

| Measure | What it communicates | Caveat |
|---|---|---|
| Resolved development minutes | Effort continuity | Depends on reviewed activity semantics |
| Commits | Local/GitHub delivery activity | Small commits/gameable; not total work |
| Completed issues | Outcomes | Size varies; reopened issues need semantics |
| PRs opened/merged/reviewed | Collaboration | Distinguish event categories |
| Confirmed tracked work entries | User-recorded effort | Count changes when entries are split |
| Qualifying development sessions | Interaction frequency | Session granularity is provider-specific |
| Separate selectable metrics | Honest comparisons | More UI choices |

**P:** Prefer selectable metrics over adding minutes, commits and tokens into an unexplained score. A mixed activity score is an option only with disclosed weights and component details. Finite metric choices belong in a dropdown. Final contribution and streak rules remain open.

## Calendar and intensity options

**P:** Use one date cell per day, week columns and seven day rows, with month/day labels and a configurable locale/week start. Pad outside-range cells distinctly; distinguish future, zero and unknown-data cells. Leap day and a year intersecting 53 or 54 partial calendar weeks should not break layout.

Intensity can use fixed meaningful duration bins (comparable across years), quantiles (uses color range well but changes meaning with data), or logarithmic scaling (handles outliers but needs explanation). Record metric, unit, thresholds and scale version. A no-activity day must not look identical to a failed collector day.

## Interaction and accessibility

GitHub's contribution calendar offers a visual overview and activity detail. **P:** tracr should open a day's diary on activation; details should show date, metric value, projects, manual/automatic provenance, confidence and missing data. Tooltip content must also be available on keyboard focus and touch/click.

Color must not be the only information carrier. Provide visible numeric/detail alternatives, a text/table view, meaningful accessible names and focus indication. Test high contrast, dark mode, screen readers and zoom; avoid hundreds of unstructured tab stops by evaluating a keyboard-navigable grid. [W3C color guidance](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html)

## Privacy and consistency

**P:** Public sharing, if ever offered, should use explicit aggregate-only export with preview. Private issue titles and project names must not leak through hover text or accessible labels. A day's work total must agree with the diary and streak qualification inputs under the same timezone/policy revision. GitHub-imported contributions may retain GitHub dates beside tracr local work dates; label that difference instead of silently shifting source history.
