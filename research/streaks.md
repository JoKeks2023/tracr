# Streaks based on meaningful development

Research date: **2026-09-28**. Implementation precedent: [Loop Habit Tracker](SOURCES.md#s1--streaks). tracr's qualifying rule is intentionally undecided.

## What the inspected implementation shows

Loop's `StreakList.recompute` clears/rebuilds streaks from computed entries in a calendar interval. Numeric habits compare values against a target; qualifying dates are grouped through calendar-day adjacency. `getBest` selects by length and then orders the selected streaks by recency. This separates qualifying-day computation from streak grouping. [Source](https://github.com/iSoron/uhabits/blob/7e993e17b2b674d4b5b1291ebd18677b74810df2/uhabits-core/src/commonMain/kotlin/org/isoron/uhabits/core/models/StreakList.kt)

The maintainer FAQ distinguishes skipped days and a smoothed habit score. A discussion shows confusion between current, best and recent streaks. Thus a streak, a grace policy and a habit score should have distinct names. These are product precedents, not rules tracr must inherit. [FAQ/discussion](SOURCES.md#s1--streaks)

## Qualifying-day options — proposals

| Rule | Benefit | Failure mode |
|---|---|---|
| At least N resolved human-work minutes | Reflects development effort, including uncommitted work | Sensitive to attribution/idle policy |
| At least one verified delivery event | Simple commits/PRs/issue completion | Rewards tiny commits; excludes research/debugging |
| Minimum effort plus development evidence | Resists app-open and unattended-agent inflation | Can miss valuable planning work |
| User-confirmed development day | Inclusive and transparent | Less automatic |
| Weighted composite | Flexible | Harder to explain or calibrate |

Evaluate thresholds such as 10, 20 and 30 minutes as experiments, not evidence-backed defaults. Opening tracr, opening a terminal or leaving an agent running does not alone qualify. Manual work may qualify with visible provenance. Weekends, vacations and recovery should not be hidden penalties in an unexplained productivity score.

## Candidate calendar algorithm

For each reporting date, compute `qualified`, `not-qualified`, `unknown`, or optional `excused`, using the policy revision and resolved evidence. Sort dates; group consecutive qualifying calendar dates. Longest streak is the maximum run; current streak is the run ending today, or yesterday while today's qualification window remains open. A missed completed day ends a strict streak. An unknown-data day needs an explicit provisional state, not automatic failure.

Do not calculate adjacency by adding 86,400 seconds: DST and timezone changes require calendar dates. Store the timezone/rule revision used for daily qualification. Decide whether travel uses a fixed home zone or recorded local zones; avoid allowing timezone toggles to manufacture duplicate days.

An excused-day policy must specify whether it extends the numerical length, merely preserves continuity or is excluded from the schedule. Keep a strict-history view if grace days are offered. Never silently transform old missed days into successes after a threshold change.

## History and corrections

Retain daily qualifying facts and policy versions; streak intervals are derived. A corrected worklog can legitimately join two runs, so recompute affected history and label the revision. Current and longest values are caches, not authoritative user data. Show historical intervals with start/end/count rather than only one lifetime maximum.

## Later acceptance examples

- Work at 23:55 and 00:05: qualify dates from split intervals, not session start.
- Yesterday qualified; today morning empty: yesterday's streak remains pending today.
- Collector offline: day unknown until backfill or manual confirmation.
- Agent generates all night: no automatic human-development qualification.
- Correction removes threshold minutes: affected streak updates visibly.
- DST transition: consecutive calendar dates remain adjacent.

No streak rule, scoring system or implementation has been selected. Contribution-graph intensity can show more than a binary streak without changing qualification semantics. [Contribution graph](contribution-graph.md)
