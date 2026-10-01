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

One row per **human-time interval** of each day with data. A **reviewed** day exports its latest reviewed revision (worklog time); a draft day exports its current derivation. Blank time (no evidence, locked, asleep) has **no row**.

| Column | Meaning |
|---|---|
| `day`, `timezone` | Reporting day and zone |
| `worklog_state` | `draft`, or `reviewed rN` (with `+ discrepancy` if newer evidence differs) |
| `start`, `end` | The interval, split at local midnight |
| `status` | `work` (counted), `unknown` (visible, **not counted**) or `non_work` (break you marked) |
| `counted_minutes`, `unknown_minutes`, `non_work_minutes` | Only on rows of that status. Never add them together. |
| `counting_basis` | `inferred` (bridged between your prompts; provisional until reviewed) or `user_asserted` (you confirmed it, entered it or timed it) |
| `duration_basis` | `inferred_continuity`, `user_asserted` or `timer_measured` |
| `unknown_reason` | `coverage_gap`, `conflicting_evidence` or `unresolved_window` |
| `target`, `target_key` | What the time was for: your explicit target (category, "Other…", repository), your manual entry's target, or the repository from git context |
| `attribution_state` | `confirmed`, `conflicting` or `unattributed` (work rows) |
| `repository` | Repository key, if the target is a repository |
| `session_id`, `native_session_id`, `project_label`, `cwd`, `git_branch` | The engaged agent session of inferred work (evidence, not attribution) |

## `all` (everything)

A folder with one CSV per data set and a `README.txt`: `sessions`, `agents`, `agent_execution_segments`, `turns`, `tool_calls`, `token_usage`, `estimated_api_cost`, `provider_reported_cost`, `human_prompts`, `human_time` (+ `_contexts`, `_proposals`), `device`, `coverage`, `events`, plus `pricing_catalog.toml` (the prices the estimates used).

- All columns of the derived store (architecture §3.4). IDs link the files.
- Every `*_at` column is epoch milliseconds and is followed by `*_at_local` (ISO 8601, reporting timezone).
- In `token_usage.csv`, only rows with `canonical = 1` count. Rows with `canonical = 0` are copies of another stream's occurrence.
- `estimated_api_cost.csv` has one estimate per usage record (`usage_id`), in nano-units (`amount_nano`, 1e-9 of the currency). Empty = unpriced, never zero. Join with `canonical = 1` usage rows before summing.
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
| `estimated_api_cost` | API-equivalent estimate at list prices, **not spend**: exact decimal of the priced part. Empty = unpriced (never zero) |
| `estimated_api_cost_currency` | Currency of the estimate (the catalog's) |
| `estimate_status` | `priced`, `partially_priced` (the amount is a floor) or `unpriced` |
| `unpriced_tokens` | Tokens with no price: unknown model, cache writes without a duration, … |
| `pricing_revision` | Catalog version + content hash; the catalog text is in the ledger and in `all` exports |
| `provider_reported_cost` | The provider's own running total (Claude Code `totalCostUSD`), as written, main-chain row only. Not spend; can carry over into a resumed session or restart. Never sum it, never add it to the estimate |
| `provider_reported_cost_currency` | As stated by the provider |
| `provider_reported_cost_recorded_at` | When tracr recorded the figure (the provider states no time) |
| `derived_as_of` | When the derived data was built |

Estimates are per usage record, from `pricing/catalog.toml` (exact model match, no family guessing). Speed (fast mode) and inference region are not retained from transcripts yet; the estimate assumes standard speed and global routing and says so in its assumptions.
