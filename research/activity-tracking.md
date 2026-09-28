# Automatic activity and concurrent sessions

Research date: **2026-09-28**. Findings: [T1](SOURCES.md#t1--claude-code-timelog), [A1](SOURCES.md#a1--activitywatch), [A2](SOURCES.md#a2--wakatime), [C1](SOURCES.md#c1--claude-interfaces), [C2](SOURCES.md#c2--codex-interfaces). All algorithms proposed for tracr below are **P**, not implemented or empirically calibrated.

## Three clocks, three meanings

1. **Session elapsed time:** the span during which a session exists.
2. **Agent execution time:** intervals spent generating, running tools or waiting for resources.
3. **Human work time:** intervals attributed to the person, including manual work and optionally reading/planning.

These quantities overlap but are not interchangeable. Separate ledgers are necessary if a user wants both effort analytics and agent utilization. This inference follows directly from concurrent provider lifecycles and independent activity sources; none can certify the person's attention by itself.

## What existing implementations actually do

| Pattern | Evidence | Good at | Failure mode |
|---|---|---|---|
| Session-local event gaps | Timelog `buildSlices` | Cheap historical estimates | Counts two sessions simultaneously; ignores long gaps wholesale |
| Equal-data heartbeat merge | ActivityWatch `heartbeat_merge` | Compact intervals from repeated observations | Only as meaningful as the watcher data; heartbeat is not necessarily human input |
| Editor heartbeats + timeout | WakaTime FAQ | Editor-focused coding duration | Long timeout bridges real breaks; misses non-editor thinking |
| OS elapsed-input / lock signal | ActivityWatch AFK; Windows API | Detects absence without recording content | Input anywhere says nothing about a specific task |
| Agent user/CLI counters | Claude OTel | Separates some interaction and processing | Still requires cross-session reconciliation |
| Provider turn/lifecycle | Codex server/hooks | Running/waiting/completion evidence | Open thread or generated output is not human focus |

ActivityWatch only merges matching payloads within a bounded pulse window and extends duration without shortening it. WakaTime documents a default 15-minute joining threshold: two minutes typing, a 13-minute pause and one minute typing can become 16 minutes. This is a policy choice, not a measurement of 13 minutes of concentration. [A1](SOURCES.md#a1--activitywatch), [A2](SOURCES.md#a2--wakatime)

## The A→B→A example: identifiability first

Desired result:

| Owner | Interval | Duration |
|---|---|---:|
| PDF Export / A | 09:00–09:32 | 32 min |
| Server Fix / B | 09:32–10:15 | 43 min |
| PDF Export / A | 10:15–10:47 | 32 min |
| **Exclusive human total** | Union | **107 min** |

Knowing only that A existed from 09:00 to 10:47 and B from 09:32 to 10:15 gives **150 session-minutes**, not 150 human-minutes. Logs containing no interaction at 09:32 cannot reveal the exact switch time. A model that outputs those exact boundaries without evidence fabricates precision.

Timelog's source illustrates the gap issue independently: A events at 09:00 and 09:20, B at 09:10 and 09:25, threshold 30 minutes. It credits A=20 and B=15 minutes. Their union is 25 minutes; the summed 35 overcounts 10 minutes of concurrent human time. A 30-minute gap is excluded entirely because the comparison is strict `<`, while a 29-minute gap is fully included. [T1](SOURCES.md#t1--claude-code-timelog)

## Candidate layered approach

```text
Source observations (agent, editor, OS, manual)
             ↓
Provider/session-local state with provenance
             ↓
Cross-session ownership and overlap reconciliation
             ↓
Reviewable human segments + separate agent intervals
```

This separates source interpretation from product policy. Two candidate implementations:

- **Interval algebra:** derive candidate intervals per source, split at every boundary, rank applicable evidence within each atom, then union compatible neighbors. Transparent replay and exact overlap handling; potentially many boundaries.
- **State machine with replay:** process ordered normalized events into open/closed segments, emit revisions when delayed events arrive. Efficient live behavior; requires explicit late-event and ordering rules.

An initial state machine could distinguish `unknown`, `user-engaged`, `agent-running`, `waiting-for-user`, `inactive`, and `closed`. A parallel device state captures active/idle/locked/asleep. These are candidate derived states, not assertions that providers emit them all.

| Observation | Candidate effect | What it does not establish |
|---|---|---|
| Prompt submitted to B | Strong engagement point; propose owner B | Continuous attention until next prompt |
| Explicit manual switch to B | Authoritative user intent | Correctness of forgotten stop time |
| B is generating | Open/extend agent interval | Human time for B |
| Editor activity in B's worktree | Strong task evidence if mapping known | Issue identity when several issues share branch |
| Terminal process is foreground | App-level supporting evidence | Which tab/pane/session is focused |
| OS input absent | Lower confidence; after threshold propose idle | Whether user is reading or planning |
| Screen lock/suspend | Close automatic human segment | Agent process termination |
| B stop | End agent turn; possibly waiting state | Return to A |
| Prompt/activity in A | New A segment | That all intervening time belonged to B |

## Foreground process, terminal and editor signals

**P:** Prefer opt-in editor metadata (repository/file activity, no text content), explicit session binding, and app-level foreground identity. Terminal PID/TTY can bind a launched process to a session, but tmux panes, shared terminals, SSH and Windows Terminal tabs break naive one-window=one-session assumptions. Terminal-specific APIs may improve fidelity but create separate compatibility obligations.

OS input age gives an inactivity bound without storing keys. `GetLastInputInfo` is scoped to the calling interactive session, not all Windows users. The inspected macOS watcher uses CoreGraphics elapsed-input queries and a session dictionary for lock state. Cross-platform permission/behavior testing remains necessary. [D1](SOURCES.md#d1--desktop-candidates), [A1](SOURCES.md#a1--activitywatch)

No screen recording, screenshots, OCR, key contents, clipboard capture or accessibility-tree scraping is necessary for this candidate. Losing permissions should yield a visible coverage gap, not a false zero-activity day.

## Confidence and activity scores

**P:** Prefer categorical evidence grades initially: user-confirmed; strong observed interaction; inferred continuity; conflicting; unknown. Retain reason codes and source IDs per segment. If later using numeric scores, calibrate against voluntary corrected diaries. A score such as 0.9 must not be marketed as a 90% probability without validation.

Recent prompt + bound editor activity is stronger than filesystem growth. A burst of tokens can describe an unattended agent; a quiet reading interval can be real work. Avoid adding heterogeneous signals into an unexplained “productivity” number.

## Reconciliation and invariants

- For one person's exclusive work ledger, counted seconds cannot exceed the interval union. Agent compute may overlap freely.
- Manual confirmation wins attribution over inference, while raw observations remain available.
- Conflicting simultaneous manual entries require resolution or explicit allocation, not silent double billing.
- Use half-open intervals `[start,end)` and split at reporting-day boundaries.
- Keep occurred-at, observed-at, machine identity and monotonic elapsed measurement when available. Wall-clock corrections can reorder observations.
- Watchers are invalidation hints. Reconcile periodically and after overflow, restart, rotation or atomic replacement; retain incomplete trailing JSON until completion. T2's watcher is a useful concrete precedent. [T2](SOURCES.md#t2--claudoscope-native-macos)
- Retrying/replaying must not change totals. Unknown formats produce quarantine/coverage diagnostics, not discarded history disguised as no work.

## Later validation protocol

Use consented scripted tasks with hand-labeled switches: A→B→A, unattended build, 20-minute reading, lunch, sleep, session crash, two machines, remote agent, permission loss, midnight and DST. Compare boundary error, overcounted seconds, unattributed time, correction burden and missing-data detection. Evaluate several thresholds rather than choosing 5/15/30 minutes from convention. No production tracker or observation experiment was implemented here.
