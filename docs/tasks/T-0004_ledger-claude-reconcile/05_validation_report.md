# Validation report

State: 2026-09-29, uncommitted working tree, Rust 1.98.1, macOS (M1). Each item says whether it was **tested** (automated, run locally), **manually verified** (run and observed) or **unverified**.

## Done-when

| Criterion | Status | Evidence |
|---|---|---|
| F main: 4 ingested, 1 skipped, non-live trigger | Tested | `f_main_lost_cursor_write_then_restart_recovers_4_and_skips_1` |
| F2 | Tested | `f2_atomic_replacement_is_not_deletion_and_doubles_nothing` (including CRLF) |
| F3: 7-minute gap surviving a crash after batch 1 | Tested | `f3_rotation_before_read_is_a_7_minute_gap_that_survives_a_crash` |
| F4 | Tested | `f4_partial_trailing_line_is_ingested_once_when_complete` |
| Trigger tests including `REPLACE` | Tested | `tracr-store/tests/immutability.rs` (7 tests), including a negative control with `recursive_triggers=OFF` |
| Real import of `~/.claude/projects` with per-type counts | Manually verified | Below |
| Second import adds 0 provider-source observations | Manually verified | +0 on a frozen snapshot. On the live directory +63, all appended by running sessions (below) |
| CI green on Linux, macOS, Windows | **Unverified** | Nothing pushed. Workflow YAML parses. Windows `cargo check` never ran anywhere, so the `cfg(not(unix))` branches have never been compiled |

## Automated (local)

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: all pass, 64 tests.

| Suite | Tests |
|---|---|
| `tracr-core` unit + `no_provider_names` | 12 + 2 |
| `tracr-store/tests/immutability.rs` | 7 |
| `tracr-ingest/tests/reconcile_f.rs` | 18 |
| `tracr-provider-claude` unit + `tests/extract.rs` | 5 + 17 |
| `tracr-app/tests/import.rs` | 3 |

Beyond F/F2/F3/F4, `reconcile_f.rs` covers:

- restart and replay from offset 0;
- occurrence keys;
- key conflicts;
- a crash during a full re-read;
- ended vs missing sources and moved files;
- a sweep finishing an unfinished read;
- in-place rewrites (head fingerprint, truncation);
- a rewrite with no ingested record;
- an unreadable directory;
- parallel streams kept apart;
- workflow agents claimed and journals never claimed.

`extract.rs` covers:

- fixture coverage of the validated range;
- the fixtures matching the reference classification;
- no content in retained facts (sentinels, one-word content, quarantine);
- the human-prompt rule and each origin marker;
- usage, reported cost and worktree facts.

**Mutation check:** 21 mutants, each breaking one invariant (trigger, gap-first commit, record anchor, occurrence seeding, sweep, discovery confirmation, origin markers, partial line, CRLF, head fingerprint, size check, journals, gap end, trigger, entrypoint allowlist, human tags). Each must be caught by a failing test. Result: **21 of 21 killed**, after one survivor (`promptSource` alone) got its own test case.

## Manually verified (dogfooding)

Release binary, scratch data directories via `TRACR_DATA_DIR`, not the default data directory.

**Live `~/.claude/projects`, first import (62.6 s, max RSS 12 MB):**

- 461 files seen, 453 claimed (main, subagent and workflow agent transcripts), 8 unclaimed (`journal.jsonl` and similar).
- 85,094 inserted, 0 duplicates, 39 quarantined, 0 key conflicts, 0 full re-reads, 0 gaps.
- By kind:

  | Kind | Count |
  |---|---|
  | `agent_message` | 49,858 |
  | `tool_finished` | 27,863 |
  | `pr_observed` | 2,229 |
  | `system_event` | 1,950 |
  | `queue_operation` | 1,216 |
  | `agent_input` | 1,131 |
  | `user_interaction` | 688 |
  | `reported_cost_snapshot` | 59 |
  | `quarantined` | 39 |
  | `turn_interrupted` | 30 |
  | `session_relocated` | 14 |
  | `worktree_observed` | 14 |
  | `session_continued` | 3 |

- Ignored lines by type are listed by `tracr stats`.
- Producer versions 2.1.231–2.1.281, all inside the validated range. 45 records have no version (39 `fork-context-ref`, 6 queue operations at stream start) and are labeled `unvalidated(none)`.

**Live, second import (24.5 s):**

- +63 observations.
- All 453 reconciles were `tail` with continuity holding.
- The 63 records come from 14 sources that concurrently running Claude Code sessions appended to between the two runs. Every new record lies beyond that source's end offset in run 1.

**Live, third import (about 40 minutes later, final binary):**

- +2,547 observations; 11 new sources.
- One resumed subagent transcript had its earlier records re-appended by the producer:
  - 256 byte-identical lines were skipped as duplicates;
  - 56 lines with a known uuid but different content (54 `tool_finished`, 2 `agent_input`) were kept, each with a `key_content_conflict` diagnostic (§4.3).
- The read was a normal `tail` with continuity holding.

**Frozen snapshot** (515 MB copy of `~/.claude/projects` from 2026-09-28):

- First import: 80,022 inserted.
- Second import: **+0**.
- Every cursor and occurrence count deleted from a copy of that ledger, then re-imported: 0 inserted, 80,022 duplicates, 0 gaps. Content-derived keys and occurrence numbering replay exactly.

**Real crash** (`kill -9` of `tracr import` on the snapshot, fresh data directory):

- The killed run had committed 26,456 observations at source 200 of 431.
- The next import added 53,566: exactly 80,022 in total, with 0 duplicates and 0 gaps.
- A further import added +0.

**Reference cross-check** (snapshot): 135,624 lines compared with the rule in `fixtures/sanitize.py`: **0 mismatches**, no stored observation without its line. This is a re-implementation of the same rule, not an independent oracle.

**Privacy audit of the live ledger** (key paths and sizes only, values not printed):

- 86,067 facts rows, largest 1,146 bytes.
- No newline or tab escapes, and no string value containing a space.
- Every string-valued key path is an allowlisted field: IDs, enums, `cwd`, branch, model, tool names, PR slugs, stated cost literals, collector fields.
- Secret patterns (`sk-ant`, `ghp_`, `AKIA`, `BEGIN `, `Bearer `, `password`): 188 hits, all git branch names containing the word "password". Branch names are an allowlisted context fact, not credential values.

**Parallel sessions:** 45 main sessions with timestamps. 42 overlap at least one other session (up to 21 at once), and each remains its own source and stream.

**CLI:**

- `roots add --repo <path>`: approved; a repeat reports "already approved" and writes no revision; a nonexistent path is rejected.
- `roots add` without a flag is rejected by argument parsing.
- `setting_rev` holds one revision per approval, each with its `user_action` (`actor=user`, `client=tracr-cli`).
- `stats --json` parses as JSON.

**Usage:** 49,901 usage-bearing records, 22,689 distinct message/request identities, 0 without identity. `stats` shows counts only, no token or cost sum.

## Unverified

- **CI** on GitHub (all three jobs) and any Windows compilation.
- **The default data directory** `~/Library/Application Support/tracr`. It was deliberately unused, so the pre-release ledger does not become permanent state.
- **Linux runtime behavior** (`FileIdentity` on Linux). Only macOS ran.
- **Migration of a real older ledger.** No schema version 2 exists yet. The backup-then-migrate path and the "ledger newer than build" error are covered only by `migrations_are_forward_only_with_backup_before_migrating`, with a synthetic migration 2.
