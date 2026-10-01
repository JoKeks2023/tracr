# Scope

## S1.5 in scope
- `pricing/catalog.toml`: exact-match rules (provider, model, service tier, speed, inference region), source URL and `fetched_at`, prices entered from the official page.
- Ledger v3 `pricing_catalog` (insert-only, by content hash), copied on app open.
- Core money kinds (`EstimatedApiCost`, `ProviderReportedCost` without `Add`), per-record `estimate`.
- Derived v6 `cost_estimate` (per usage record × pricing revision) and `reported_cost` (latest per session and field).
- Day DTO, session sheet, day summary, CLI `sessions`, CSV `agent-sessions` and `all`.

## S1.5 out of scope (unchanged phase plan)
Actual expense, subscriptions, allocation (after S1); billing imports; currency conversion; finalized-report pricing snapshots (review snapshots do not carry cost yet).

## S1.6
See the S1.6 sections of the reports.
