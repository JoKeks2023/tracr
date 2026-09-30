# CSV export

Pulled forward from S4 (reporting §5) at the user's request on 2026-09-30.

- CLI:
  - `tracr export timesheet --from YYYY-MM-DD --to YYYY-MM-DD [--out file.csv]`
  - `tracr export agent-sessions --from … --to … [--out file.csv]`
  - `tracr export all --out <folder>`
- App: **Export CSV** (header), choose what and the range, then save.

## Conventions

- UTF-8, comma-separated, RFC 4180 quoting, one header row, `\n` line ends.
- Times are local ISO 8601 in the reporting timezone, with offset (e.g. `2026-09-30T14:05:00+02:00`).
- **An empty cell means unknown or not applicable, never zero.**
- IDs are stable across rebuilds:
  - `session_id` is tracr's derived ID of the session's natural key;
  - `native_session_id` is the provider's own ID.

## `timesheet` (human time)

One row per **human-time interval** (atom) of each day with data. Blank time has **no row**: no evidence, locked or asleep.

| Column | Meaning |
|---|---|
| `day`, `timezone`, `start`, `end` | The interval, split at local midnight |
| `status` | `work` (counted) or `unknown` (visible, **not counted**) |
| `counted_minutes` | Only on `work` rows |
| `unknown_minutes` | Only on `unknown` rows. Never add it to `counted_minutes`. |
| `counting_basis` | `inferred`: bridged between human prompts of one session (provisional until reviewed); `user_asserted` later (S2) |
| `unknown_reason` | `coverage_gap` (tracr was not collecting what the rule needs, e.g. before the device sensor ran), `conflicting_evidence` (prompts of several sessions interleave), `unresolved_window` (a gap the rule cannot bridge) |
| `session_id`, `native_session_id`, `project_label`, `cwd`, `git_branch` | The engaged session of a `work` row. This is evidence, not attribution: attribution comes later (M3/S2). |
| `start_boundary`, `end_boundary` | `observed` (a prompt), `device` (lock/sleep cut), `day_boundary` |
| `reasons` | All rule reasons, `;`-separated |

## `all` (everything)

A folder with one CSV per data set and a `README.txt`: `sessions`, `agents`, `agent_execution_segments`, `turns`, `tool_calls`, `token_usage`, `human_prompts`, `human_time` (+ `_contexts`, `_proposals`), `device`, `coverage`, `events`.

- All columns of the derived store (architecture §3.4). IDs link the files.
- Every `*_at` column is epoch milliseconds and is followed by `*_at_local` (ISO 8601, reporting timezone).
- In `token_usage.csv`, only rows with `canonical = 1` count. Rows with `canonical = 0` are copies of another stream's occurrence.
- The raw ledger (evidence envelopes and facts) is not part of this export.

## `agent-sessions`

One row per **reporting day × agent session × actor**. Only days with data are included. A session active on several days has one row per day, and its values are clipped to that day.

Agent execution is **agent time, not human time**. Rows of parallel sessions and subagents overlap in time and may sum to more than the day. The human-time timesheet export follows with the human timeline (S1.4 M2).

| Column | Meaning |
|---|---|
| `day` | Reporting day, `YYYY-MM-DD` |
| `timezone` | Reporting timezone (AD-14) |
| `provider` | e.g. `claude_code` |
| `session_id` | tracr session ID (stable) |
| `native_session_id` | The provider's session ID |
| `project_label` | Last path component of the session's working directory |
| `cwd` | Working directory at the session's first record |
| `git_branch` | Branch at the session's first record |
| `actor_kind` | `unknown` = main-chain agent. It is not claimed as `main`, because the transcripts do not document it (AD-21). `known` = subagent with its own ID. |
| `agent_id` | Subagent ID (empty for the main chain) |
| `first_activity`, `last_activity` | First and last execution instant of this actor on this day |
| `execution_minutes` | Union of this actor's execution segments on this day. Segments split after 10 quiet minutes (agent-segment@2). |
| `segments`, `turns`, `tool_calls` | Counts on this day |
| `human_prompts` | Human prompts in the session on this day. **Only on the main-chain row**, so the column can be summed. |
| `models` | Models seen in this actor's usage, `;`-separated |
| `input_tokens`, `cache_write_tokens`, `cache_read_tokens`, `output_tokens` | Sums over the records that state the category. Each API request is counted once: streamed repeats are elected, and copies in resumed or forked sessions are excluded. |
| `*_unknown_records` | Usage records that do not state that category |
| `unidentified_usage_records` | Usage records without a request identity (never merged, AD-17) |
| `derived_as_of` | When the derived data was built |

There is no cost column yet. Estimated API cost (S1.5) will be its own column, labeled as an estimate, not spend.
