# Implementation report

- `crates/tracr-core/src/human.rs`:
  - `HUMAN_RULE` is now `human-timeline@3`, with `reply_window_ms` and `Evidence::agent_ends`.
  - `assess` computes `Gap::reply`.
  - `push_work` merges adjacent work pieces of the same session.
  - The reply case removes lock and sleep time with `minus_away`.
- `crates/tracr-app/src/derive_human.rs`:
  - Segment ends are passed in, sorted.
  - The ignored record types of both providers are passed to `degraded_extents`.
- `crates/tracr-store/src/ledger.rs`: `degraded_extents(conn, ignored_types)` skips quarantined records whose native type is now ignored.
- `crates/tracr-provider-claude/src/manifest.rs`: 6 new ignored record types.
- **Tests:** 5 new tests in `human.rs` and an ignore test for the 6 types in `tests/extract.rs`.
- **Changed test expectation in `tests/runtime.rs`**, `a_collected_day_counts_bridged_minutes_and_leaves_cross_session_gaps_unknown`:
  - B's prompt was at 09:15, 4 min after A finished. That gap now counts under the new rule.
  - B's prompt moved to 09:17, 6 min after A finished, so the test still checks the "unknown" path. It expects 10 min counted and 7 min unknown.
- `docs/architecture.md` §7.3 now describes rule 2a.
