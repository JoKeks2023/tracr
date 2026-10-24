# Implementation report

## S1.5 cost estimation (2026-10-01)

| Part | Where |
|---|---|
| Pricing catalog: 20 rules (17 standard, 3 fast mode), source https://platform.claude.com/docs/en/about-claude/pricing fetched 2026-10-01 | `pricing/catalog.toml` |
| `Money` (nano-units), `EstimatedApiCost` (`checked_add`, same currency only), `ProviderReportedCost` (no `Add`), exact decimal parsing | `crates/tracr-core/src/money.rs` |
| `Catalog` + `validate` (exact prices, known sources, unique keys); `estimate` per record: categories input / cache read / cache write 5m·1h / output, tool charges per unit, `priced`/`partially_priced`/`unpriced`, reasons, assumptions, `price_basis` | `crates/tracr-core/src/pricing.rs` |
| Ledger v3 `pricing_catalog` (insert-only) + `ensure_pricing_catalog` | `crates/tracr-store/migrations/ledger/0003_pricing.sql`, `ledger.rs` |
| Derived v6: `cost_estimate`, `reported_cost`; readers `usage_sums_in` (with cost), `spanning_usage`, `reported_costs` | `crates/tracr-store/schema/derived.sql`, `derived.rs` |
| Bundled catalog, revision `<version>+<blake3 prefix>`, copy into ledger on open | `crates/tracr-app/src/pricing.rs`, `lib.rs` |
| Derivation: one estimate per usage row; latest reported figure per session; `pricing_revision` meta, a new catalog forces a full rebuild | `crates/tracr-app/src/derive.rs` |
| Day DTO: `UsageLine.cost`, `SessionDay.cost`/`reported`, `DayAgents.cost` (identified, unidentified, spanning, pricing info); cost lane no longer pending | `crates/tracr-app/src/day.rs` |
| CSV: 8 new `agent-sessions` columns; `all` adds `estimated_api_cost.csv`, `provider_reported_cost.csv`, `pricing_catalog.toml` | `crates/tracr-app/src/export.rs`, `docs/export.md` |
| CLI `sessions`: estimate per usage line, reported figure, day estimate with catalog | `apps/cli/src/main.rs` |
| UI: day metric "est. API cost · not spend" (+ unpriced share; tooltip with catalog, reasons, assumptions, unidentified and spanning lines); session sheet "Est. cost" column, note, provider-reported section | `apps/desktop/ui/src/{App,SessionSheet,format,types}.ts(x)` |

### Decisions (conservative defaults, documented)
- **Speed and inference region are not retained** by the Claude extractor (`claude-extract/2`). Changing the retained facts needs an extractor-version change, and old observations cannot be re-extracted (the ledger keeps facts, not raw lines). The estimate therefore prices them as `standard`/`global` and **states the assumption** on every affected estimate. Evidence: all 26,214 `speed` fields in local transcripts are `"standard"`, all `inference_geo` are `"not_available"`. Follow-up issue: retain `speed` and `inference_geo`.
- **Partial estimates show as "≥ $x"**: the priced part is a floor, never the whole.
- **No `effective_from`** in the catalog: the page states current prices, not since when, so estimates carry `catalog_as_of:2026-10-01`.
- **Model IDs only where verified**: Claude Sonnet 5.5 is on the pricing page, but its model ID is not verified, so it has no rule (unpriced if it appears).
- **Reported figure label corrected** after dogfooding: Claude Code's `totalCostUSD` is its own running total, not the transcript session's cost (see validation).

## S1.6 Codex adapter (2026-10-01)

| Part | Where |
|---|---|
| Discovery: rollout file name → stream `codex:<machine>:<thread>`; live and archived are one source | `crates/tracr-provider-codex/src/discovery.rs` |
| Manifest: validated 0.145–0.158 (alpha builds by numeric core), interactive source `vscode`, gated `subagent_linkage` | `manifest.rs` |
| Extractor (`codex-extract/1`): session_meta, turn_context, task_started/complete, turn_aborted, item_completed (UserMessage, AgentMessage), tool calls and outputs by call ID, token_usage_record, token_count, thread settings; content kinds ignored | `extract.rs` |
| Normalizer (`codex-normalize/1`): turns with origin, prompts only in interactive threads, tools, model per turn, usage election (`codex.last_record_per_response@1`, `codex.cumulative_delta@1` with suppression and reset detection), subagent gate | `normalize.rs` |
| Core: `Normalizer::session_of` also sees the stream's records (a child names its parent) | `crates/tracr-core/src/normalize.rs` |
| Incremental scope: `ledger::parent_links`; a child joins its parent's scope | `crates/tracr-store/src/ledger.rs`, `crates/tracr-app/src/derive.rs` |
| Registration of adapter and normalizer | `crates/tracr-app/src/{lib,derive}.rs` |
| Fixtures: `CodexThread` builder with the profiled shapes, a placeholder in every content field | `crates/tracr-testkit/src/codex.rs` |
| Desktop: banner for proposed, unapproved provider roots | `apps/desktop/ui/src/App.tsx` |

### Findings from local rollouts (structure-only profile, 37 threads)
- **Prompts** are `event_msg/item_completed` with `item.type = "UserMessage"`, not `response_item` user messages (those also carry injected context).
- **`subagent_history_start_ordinal` is not a copy marker**: in all 17 child threads it equals the thread's own line count. Treating it as one would have discarded every child record. Retained as a fact, unused. *This contradicts architecture §8.1 step 3, which lists it as a fixture-validated copy marker.*
- **Item IDs are not global**: 33 `item_completed` IDs repeat across unrelated threads. Record keys for items are scoped by thread; tool records use call IDs (0 repeats); usage uses `response_id` (0 repeats). The first dogfood run (unscoped) lost 16 real prompts as false copies; a regression test covers it.
- **Usage semantics**: `total = input + output` on every record, so cached ⊂ input and reasoning ⊂ output. `cache_write_input_tokens` is always 0 locally; its relation to input is not established, so cache writes are kept as a marker, not a category.
- **Children**: 16 `guardian` reviewer threads and 1 spawned (forked) thread, all naming a parent that is present locally. Children report their own usage (no `response_id` shared with the parent).
- 18 older threads have only cumulative `token_count`; 2 of them reset their totals (after compaction).

### Decisions (conservative defaults)
- Interactive source allowlist is `vscode` only (the only value in the corpus); `cli`/`exec` prompts start `automated` turns until fixtures cover them.
- A `UserMessage` in a non-interactive or child thread is counted as `prompt_from_non_interactive_source`, never engagement.
- No OpenAI prices in the catalog: Codex usage shows "unpriced", which never means zero.
- The live app does not approve `~/.codex/sessions` by itself: a banner offers it (PS-02).

## S1 closure items (2026-10-01, after S1.6)

| Part | Where |
|---|---|
| Architecture §8.1 / §13.2 corrected: Codex copy markers and prompt rule (TRACR-9) | `docs/architecture.md` (`8c9c352`) |
| Walkthrough C before review across providers; Codex events carry the thread's cwd and branch (fix: Codex sessions had no cwd/repository); a normalizer version change forces a rebuild | `crates/tracr-app/tests/walkthrough_c.rs`, `crates/tracr-provider-codex/src/normalize.rs`, `crates/tracr-app/src/derive.rs` (`7293706`) |
| INV-7 DTO test | `crates/tracr-app/tests/cost.rs` |
| `tracr day [date]`: text day report with quality footer | `crates/tracr-app/src/report.rs`, `apps/cli/src/main.rs` (`7aa4e0d`) |
| Menu bar (S1.7): today's counted and unknown minutes, collector state | `apps/desktop/src-tauri/src/main.rs` |
| Desktop quality footer (§13.4); `DayAgents.ledger_counts` | `apps/desktop/ui/src/QualityFooter.tsx`, `crates/tracr-app/src/day.rs` |

**Walkthrough C execution minutes:** 60 · 41 · 40 instead of the walkthrough's illustrative 59 · 38 · 37. A segment opens at its turn's start (the prompt), while the walkthrough starts each run one minute after its prompt. Human results match exactly: 52 counted, 13 unknown, targets 25 / 27, C 0.

## Compressed Codex rollouts (2026-10-24, GitHub #2 / TRACR-13)

Codex now stores rollouts as `rollout-….jsonl.zst`; the plain files are gone locally.

| Part | Where |
|---|---|
| Discovery claims `*.jsonl.zst`; the thread ID in the name is the stream, so a compressed thread is the same source as its plain name | `crates/tracr-provider-codex/src/discovery.rs`, `crates/tracr-ingest/src/discovery.rs` |
| `reconcile` reads a compressed file as its decompressed content (anonymous temp file, `ruzstd`); offsets and fingerprints are those of the content, identity and mtime those of the compressed file | `crates/tracr-ingest/src/reader.rs` (`decompress`), `reconcile.rs` |
| An unchanged compressed file (same path, identity, mtime, cursor at end) is not decompressed again | `reconcile.rs` (`compressed_unchanged`) |
| A frame cut off by a running writer keeps the bytes decoded so far (partial last line); a file that yields nothing is an error | `reader.rs` |
| Live watcher hints include `.zst` | `crates/tracr-app/src/runtime.rs` |

**Decision:** plain → compressed changes the file identity, so continuity breaks (`identity_changed`) and the file is re-read once; the dedup identity keeps that at 0 new observations, and the history check finds the last record, so no gap.

**Tests:** `a_compressed_rollout_is_a_source`, `compressing_a_thread_that_was_read_plain_adds_nothing_and_no_gap` (`crates/tracr-app/tests/codex.rs`), `a_compressed_rollout_is_the_same_stream_as_its_plain_name` (codex discovery).

## Codex prices in the catalog (2026-10-24, TRACR-17)

- `pricing/catalog.toml` `2026-10-24.1`: source `openai-pricing` (`https://developers.openai.com/api/docs/pricing`, fetched 2026-10-24 by a fetch agent, not by hand) and four rules, provider `codex`, tier `default`, speed `standard`, geo `global`: `gpt-5.6-luna` 0.20 / 0.02 / 1.20, `gpt-5.6-sol` 4 / 0.40 / 20, `gpt-5.6-terra` 2 / 0.20 / 12, `gpt-6-astra` 10 / 1 / 50 (input / cached input / output, USD per MTok, standard short context).
- Not modeled and stated in the catalog header: input above 272K tokens (priced higher on the page), Fast/Flex/Batch, cache-write prices. Largest local Codex input per response is 244,947, so no local record is affected.
- The page notes promotional pricing for `gpt-5.6-sol` through at least 2026-11-21; whether the listed number is the promotional one is not stated in what was read.
- `codex-auto-review` is not on the page: stays unpriced.
- The service tier comes from `thread_settings_applied` (value `default`); the testkit emits it when `CodexThread::service_tier` is set.
- Codex estimates are floors (`category_not_reported:cache_write`): Codex reports cache writes only as a marker.
- Test `bundled_catalog_is_valid_and_exact` now requires the cache-write tiers only for Claude rules.
