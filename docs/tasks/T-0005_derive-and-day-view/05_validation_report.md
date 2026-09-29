# Validation report

## Milestone 1: S1.3 (2026-09-29)

Each item is marked **tested** (automated, run locally), **manually verified** or **unverified**.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass: 93 tests (64 before).

| Criterion | Status | Evidence |
|---|---|---|
| D: 910, not 1,510 | Tested | `tracr-provider-claude/tests/normalize.rs::d_streaming_reemissions_elect_the_last_record_910_not_1510` |
| E-1: prompt rule from retained markers agrees with extraction | Tested | `e1_renormalization_from_retained_facts_agrees_with_extraction_on_fixtures` (all fixture versions, >100 records). The existing 17 extraction tests, including the reference cross-check, pass unchanged after the extractor was switched to the shared marker rule. |
| Automated entrypoint | Tested | `automated_entrypoint_prompts_run_the_agent_but_are_never_human` |
| Task notifications start automated turns | Tested | `task_notifications_start_automated_turns` |
| Crashed session | Tested | `tracr-core::agent::crashed_session_ends_at_last_evidence_bounded` |
| Quiet-gap split | Tested | `a_long_quiet_gap_splits_the_segment_but_not_the_turn` |
| Copied context: resume, original never continued | Tested | `tracr-app/tests/derive.rs::resume_original_never_continued_…`. Unit test with the copy created first: `copy::resume_original_never_continued_is_contained_and_canonical`. |
| Copied context: fork, copy continued first | Tested | `fork_copy_continued_first_…` |
| Parallel sessions never merged | Tested | `parallel_sessions_overlap_and_are_never_merged` |
| INV-6 (rebuild half) | Tested | `rebuild_is_deterministic_and_reimport_changes_nothing` (canonical dump equal after rebuild and after re-import + rebuild) |
| Real sessions listed individually | Manually verified | Below |
| INV-6 (incremental equals fresh) | **Unverified**: no incremental derivation yet (S1.4) | |

### Real history (scratch data directory, release build)

- **Import:** 497 files seen, 489 claimed, 95,758 observations. 56 key conflicts: the same re-appended subagent lines as in T-0004.
- **Rebuild:** 48 sessions, 488 actors, 1,289 turns, 29,359 tool calls, 1,317 execution segments, 666 engagement instants, 121,403 events (7,688 copied), 27,570 usage records (1,830 duplicates, 0 unidentified).
- **Prompts reconcile:** 757 human prompts in the ledger = 666 engagement instants + 91 copied prompt events.
- **`tracr sessions --day 2026-09-28`:** 12 sessions, each listed on its own. Subagents appear as `known` actors. Main-chain activity is actor `unknown` (AD-21). Tokens are per actor and model. The human, device, coverage and cost lanes are printed as "not derived yet".
- **Quiet-gap split on real data:** the longest remaining segment is 155 minutes of continuous activity; before the split it was 1,301 minutes.
