# Done when

## S1.5
- [x] Every canonical usage record has an estimate under the current pricing revision; unpriced records have no amount (NULL), never 0.
- [x] No substring or family matching; an unknown model is `unpriced` with reason `no_price_rule:<model>`.
- [x] Cache writes are priced by their TTL tier; untiered writes stay unpriced and are named.
- [x] Conditions the source does not record (speed, inference region) are stated as assumptions on the estimate.
- [x] The catalog used is stored in the ledger by content hash; a new catalog forces a rebuild.
- [x] Provider-reported figures: the latest per session and field, never summed, labeled as the provider's own running total.
- [x] Views label the estimate "API-equivalent estimate, not spend" and show catalog version, `fetched_at` and the unpriced share.
- [x] CSV carries amount, currency, status, unpriced tokens, revision, and the reported figure separately.
- [x] Tests: core estimator units, app end-to-end (estimate, CSV, ledger catalog, incremental update).
- [x] Dogfood on a copy of the real ledger; deployed app shows the cost.

## S1.6
- [x] Rollouts discovered live and archived; archiving keeps the source (F5).
- [x] No content retained (test with a placeholder in every content field); unknown kinds quarantined visibly.
- [x] Walkthrough D (Codex): cumulative 0.25/0.25/0.45/0.60 M → 0.60 M; `token_usage_record` wins over `token_count`.
- [x] G-r: gate on → child `known(child)` inside the parent session, parent stays `unknown`, totals 0.8 M; gate off → child its own session, `unlinked_subagent`, no engagement.
- [x] INV-8: identical prompts from Claude and Codex give identical human atoms.
- [x] Real local rollouts: 0 quarantined, 0 key conflicts, usage totals equal a direct count, every interactive prompt is an engagement instant.
- [ ] Codex root approved in the running app: needs the user (PS-02).
