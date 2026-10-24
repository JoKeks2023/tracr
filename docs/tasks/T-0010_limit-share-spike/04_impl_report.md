# Findings (2026-10-24)

## Codex — available, provider-reported, evidenced locally
- Source: `rate_limits` inside rollout records (33 of 36 local `.jsonl.zst` rollouts carry it).
- Fields: `primary.{used_percent, window_minutes, resets_at}`, `secondary` (null everywhere locally), `plan_type`, `limit_id`, `credits.{has_credits, unlimited, balance}`, `rate_limit_reached_type`.
- Observed combinations (per file): `prolite/codex/10080` min (23 files, weekly window), `free/codex/43200` min (9 files, 30 days), `free/premium/null` (1).
- No 5-hour window seen locally. The window length is data (`window_minutes`), not a constant: never hard-code 5h/7d.
- Caveat: rollouts are now `.jsonl.zst`; the adapter does not claim them yet (GitHub issue #2). Newest rollout is 2026-09-29.
- The existing Codex fixture models only `limit_id`/`plan_type` (`crates/tracr-testkit/src/codex.rs:218`), so extraction, fixture and a usage-share kind are all new work.

## Claude Code — not in transcripts; one documented channel
- 0 structural hits for `rate_limits`/`five_hour`/`seven_day`/`used_percentage` in local transcripts (text matches only).
- Documented channel (docs: code.claude.com/docs/en/statusline, as reported by a docs lookup; not verified on this machine): the `statusLine` command receives on stdin `rate_limits.five_hour.{used_percentage, resets_at}` and `rate_limits.seven_day.{used_percentage, resets_at}`. Only for claude.ai Pro/Max, only after the first API response of a session, each window may be absent. Gateway variant: `rate_limits.spend_limit.*`.
- Not exposed per docs: hooks, OpenTelemetry metrics/events, `/usage` in machine-readable form, any file under `~/.claude`. (Hooks page only partly read.)
- Consequence: a tap must be our own statusLine wrapper that writes its stdin snapshot (percent + reset only, never other fields) to a tracr-owned spool. This machine already has a managed statusLine command (clawd); tracr would have to chain with it, not replace it. Changing settings needs the user's decision.
- Not verified: that the field appears on this account/version. Requires one real session with the tap.

## What can and cannot be computed
- Provider-reported percent: exact when present, at the snapshot time only. Between snapshots tracr knows nothing.
- Attribution of a percent to a session/project: not possible directly. Only the delta between two snapshots exists, and parallel sessions share one limit. At best: tokens of each session inside the window, shown beside the percent, never a derived percent per session.
- Self-calibrated percent (tokens ÷ guessed limit size): the limit size is undisclosed and plan-dependent. Only as an explicitly labeled estimate; not recommended before provider-reported data exists.

## Recommendation (for the user to decide)
1. New provider-reported kind `LimitUsage { window_minutes, used_percent, resets_at, observed_at, plan }`, separate from money (Canon: money kinds / provider-reported own kind). Display as "x % of weekly limit at HH:MM (provider-reported)".
2. Step 1: Codex extraction + fixture + day view row (data exists, no settings change). Needs issue #2 first, otherwise only history is visible.
3. Step 2: Claude via an opt-in statusLine tap, after the user agrees to the settings change; verify with one real session.
4. Per-session/project percent: not offered; show tokens in window instead.

## Open for the user
- Is "provider-reported snapshots + tokens in window" enough, or is a per-session percent a hard requirement (then it must be a labeled estimate)?
- OK to design the Claude statusLine tap (chained with the existing clawd command)?
