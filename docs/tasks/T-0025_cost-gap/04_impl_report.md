# T-0025 Investigation: estimate below Claude Code's own cost (TRACR-11)

Question: why is tracr's API-equivalent estimate 5–15 % below Claude Code's `totalCostUSD` on larger sessions?

## Method (2026-11-16, read-only, copies of the live data)
- Derived store copy: per session, sum of `cost_estimate.amount_nano` vs the latest `reported_cost` (`totalCostUSD`), 13 sessions with a reported figure.
- Transcripts read locally with small scripts (no content printed, only usage numbers and record keys).

## Findings
Sessions are labelled S1…S7 and amounts are left out on purpose: the figures come from private usage, and only their relation matters here.

| Session | Model | Records | Gap (estimate vs reported) | Compactions |
|---|---|---:|---:|---:|
| S1 (largest) | opus-5-5 | 584 | −12.5 % | (transcript not found) |
| S2 | sonnet-5-5 | 593 | −8.2 % | 4 |
| S3 | opus-5-5 | 391 | −2.6 % | 2 |
| S4 | sonnet-5-5 | 257 | −9.2 % | 1 |
| S5 | opus-5-5 | 65 | −7.7 % | – |
| S6 | sonnet-5-5 | 10 | −15.8 % | 0 |
| S7 | sonnet-5-5 | 6 | −6.0 % | 0 |
| 6 more | | ≤ 22 | 0 to −5.7 % | 0 |

The estimate is **never above** the reported figure: the gap is missing cost, not mispricing upward.

**Ruled out:**
1. *Streaming re-emissions with partial usage.* In 8,124 messages that Claude Code writes as several lines, all lines carry identical usage, so picking any line is right.
2. *Partial `output_tokens`.* Output counts are final (median 413 characters at 100–1000 tokens; only 2 of 10,679 messages had < 20 tokens with > 400 characters).
3. *Unpriced records.* All records are priced except 3 `<synthetic>` ones (no model, no cost).

**Main cause (strong evidence): API calls that never reach the transcript.**
- **Compaction.** Each `compact_boundary` system record states `preTokens` (context summarized) and `postTokens` (summary). The summarizing request is not written as an assistant record with usage. Pricing those requests at catalog prices (input read from cache … fresh, output = `postTokens`) gives a range per session; compared with the observed gap:
  - S2 (4 compactions): the gap lies inside the range.
  - S3 (2): the gap is below the range (the summary may run on a cheaper model or mostly from cache).
  - S4 (1): the gap is above the range (more than compaction is missing).
- **Other helper requests** (title, command checks and the like): sessions without any compaction still miss a small, steady amount per request (S6, S7).

## Conclusion
The gap is real, comes from requests the transcript does not contain, and cannot be priced from local evidence: compaction states token counts but not the model, cache split or exact request, and helper requests leave no trace at all. tracr must therefore **not** add an invented figure (canon: *Unpriced is never zero*; *Cost is labelled as estimated API cost, not spend*). The honest fix is to **say** it: per session, "N compactions (X tokens summarized) are not in this estimate", and a general note that Claude Code's own figure includes requests the transcript does not record.

## Follow-up (not done here)
- `context_compacted` events need fixtures that validate the mapping for the Claude Code version range first (architecture: *only after fixtures validate the mapping*, OQ-I-07; manifest `fixture_gated: compaction_mapping`). Then: normalizer emits `context_compacted` with `pre_tokens`/`post_tokens`, and the session cost view states the omission.
- The extractor already keeps the record (system event, subtype `compact_boundary`, `compact_metadata_present`), so history can be shown once the mapping is validated (only `preTokens`/`postTokens` would need a new extractor version, and that applies to new lines only).
