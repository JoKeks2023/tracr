# Scope

## S1.5 in scope
- `pricing/catalog.toml`: exact-match rules (provider, model, service tier, speed, inference region), source URL and `fetched_at`, prices entered from the official page.
- Ledger v3 `pricing_catalog` (insert-only, by content hash), copied on app open.
- Core money kinds (`EstimatedApiCost`, `ProviderReportedCost` without `Add`), per-record `estimate`.
- Derived v6 `cost_estimate` (per usage record × pricing revision) and `reported_cost` (latest per session and field).
- Day DTO, session sheet, day summary, CLI `sessions`, CSV `agent-sessions` and `all`.

## S1.5 out of scope (unchanged phase plan)
Actual expense, subscriptions, allocation (after S1); billing imports; currency conversion; finalized-report pricing snapshots (review snapshots do not carry cost yet).

## S1.6 in scope
- New crate `tracr-provider-codex`: discovery (`sessions/YYYY/MM/DD/rollout-*.jsonl`, `archived_sessions/rollout-*.jsonl`; never `session_index.jsonl` or `transcription-history.jsonl`), allowlist extractor, manifest, normalizer.
- Turns, human prompts (interactive source only), tools by call ID, usage (`token_usage_record`, else cumulative `token_count` deltas), subagent linkage gate, copy detection by global native IDs.
- Incremental derivation keeps a parent thread and its children in one scope.
- Desktop: a banner offers proposed but unapproved provider roots (PS-02: the user approves).

## S1.6 out of scope
Codex pricing (no OpenAI rules in the catalog: Codex usage is unpriced, not zero); hooks / App Server live observation (S4); `state_N.sqlite` reading.
