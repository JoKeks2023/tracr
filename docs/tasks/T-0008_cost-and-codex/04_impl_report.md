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
