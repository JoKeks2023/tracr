# Validation report

## S1.5 (2026-10-01)

**Tests:** `cargo test --workspace --exclude tracr-desktop`: 151 passed, 0 failed; clippy clean; `pnpm typecheck` clean.
New: `tracr-core` money (3) and pricing (9) units; `tracr-app` `bundled_catalog_is_valid_and_exact`; `tests/cost.rs` (3): per-record estimate with exact nano amounts, unpriced ≠ 0, untiered cache writes named, assumptions stated, latest reported figure only, ledger catalog copy, CSV columns, incremental update adds exactly the new record's estimate.

**Dogfood: copy of the real ledger** (`tracr rebuild` on a `.backup` snapshot, 28,621 usage records):

| Model | Status | Records | Est. (USD) |
|---|---|---|---|
| claude-opus-5-5 | priced | 18,261 | 1,918.15 |
| claude-sonnet-5 | priced | 7,569 | 609.68 |
| claude-opus-5 | priced | 764 | 252.10 |
| claude-opus-4-8 | priced | 75 | 18.09 |
| claude-fable-5-1 | priced | 48 | 16.31 |
| `<synthetic>` | unpriced (`no_price_rule`) | 68 | — |

Hand check: one 219-record opus-5-5 line (in 438, cache writes 1.94 M at 1 h, cache reads 121.11 M, out 351.5 k) = 0.0018 + 15.52 + 24.22 + 7.03 = **$46.78**, as shown.

**Cross-check against Claude Code's own `totalCostUSD`** (26 sessions with a figure):
- Simple sessions agree closely: 0.5681 vs 0.5671 (sonnet-5), 0.7249 vs 0.7080, 3.246 vs 3.189.
- Larger sessions: tracr's estimate is 5–15 % lower (e.g. 11.97 vs 10.48). Not explained yet; candidates are API calls that never reach the transcript (e.g. compaction or side calls). Not investigated further: the estimate counts what the transcript shows.
- The figure's scope is **not** the transcript session: two resumed sessions carry the identical 574.35 (one has only copied usage, estimate 0), and one shows 4.47 against an estimate of 345. The label was changed to "provider's own running total" accordingly.

**Runtime (deployed app):**
- Ledger migrated v2 → v3 with backup `backups/ledger-premigration-v2-2026-10-01T10-22-52.779Z.sqlite`.
- `pricing_catalog` holds `2026-10-01.1+f127175cfa9a`.
- `coverage_started` at 10:23:02Z; derived store rebuilt to v6 by the app: 28,614 priced, 84 unpriced.
- The day summary shows "$75.03 · est. API cost · not spend" (screenshot `s15.png`).

**Not verified by hand:** the session sheet's Est. cost column and provider-reported section, and the tooltip. They are type-checked only and have not been clicked in the running app.

## S1.6 (2026-10-01)

**Tests:** 166 passed, 0 failed; clippy clean; `pnpm typecheck` clean.
New: `tracr-provider-codex` discovery (2), extraction (5: kind mapping, no content retained, usage fields, session origin and parent, quarantine); `tracr-app/tests/codex.rs` (8): D cumulative deltas, D record precedence, G-r gate on, G-r gate off, F5 archive, INV-8 symmetry, late child incremental update, repeated item IDs.

**Dogfood: copy of the real ledger + local Codex rollouts** (roots approved on the copy only):
- Import: 533 sources, 37 new, 7,661 observations, **0 quarantined, 0 key conflicts**.
  - First run: 8 key conflicts, from two `turn_context` records in one turn. Fixed: the key includes the ordinal.
  - First run: 10 alpha versions reported as unvalidated. Fixed: gating uses the numeric core.
- Rebuild: 20 Codex sessions (37 threads; 17 children linked as known actors, 13 with activity), 0 `unlinked_subagent`.
- Usage equals a direct count from the files:
  - `token_usage_record`: 49,326,796 input / 234,404 output.
  - cumulative deltas: 183,219,113 / 505,384.
  - 13 unchanged totals suppressed, 2 resets.
- Engagement: 130 instants, equal to the 130 `UserMessage` items in interactive threads (after the item-key fix; the first run had 114).
- Copied context: 1 Codex event (the forked child's copy of its parent's turn).

**Not verified at runtime:** the approval banner and the Codex sessions in the running app. They need the user to approve the Codex root.

## S1 closure items (2026-10-01)

- Tests: 169 passed, 0 failed; clippy clean; typecheck clean.
- `tracr day 2026-09-16` on the real-data copy: renders human, agent, cost and quality blocks. That day: 6 sessions (2 Codex), ≥ $37.50 identified estimate, 9% of tokens unpriced (Codex).
- Deployed. After the forced rebuild, all 20 live Codex sessions have a cwd and 7 a repository (the rest are outside approved roots or not git). The quality footer renders (screenshot `s17.png`); the root banner is gone because the user approved the Codex roots.
- **Not verified by hand:** the menu-bar rows (tray menu not opened by me).
