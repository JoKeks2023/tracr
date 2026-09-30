# CSV export

Pulled forward from S4 (reporting §5) at the user's request on 2026-09-30.

- CLI: `tracr export agent-sessions --from YYYY-MM-DD --to YYYY-MM-DD [--out file.csv]`
- App: **Export CSV** (header), choose the range, then save.

## Conventions

- UTF-8, comma-separated, RFC 4180 quoting, one header row, `\n` line ends.
- Times are local ISO 8601 in the reporting timezone, with offset (e.g. `2026-09-30T14:05:00+02:00`).
- **An empty cell means unknown or not applicable, never zero.**
- IDs are stable across rebuilds:
  - `session_id` is tracr's derived ID of the session's natural key;
  - `native_session_id` is the provider's own ID.

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
