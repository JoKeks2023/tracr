# Implementation report

State: 2026-09-29, uncommitted working tree. Validation evidence: [05_validation_report.md](05_validation_report.md).

## Delivered (per Implementation Start)

| Deliverable | Where |
|---|---|
| Workspace, toolchain pin, CI (Linux fmt + clippy `-D warnings` + tests; macOS build + tests; Windows `cargo check` of non-host crates, §1/§10) | `Cargo.toml`, `rust-toolchain.toml` (1.98.1), `.github/workflows/ci.yml` |
| Evidence vocabulary, validators, manifest selection, adapter trait; provider-name lint (INV-8) | `crates/tracr-core/src/*`, `crates/tracr-core/tests/no_provider_names.rs` |
| Ledger: connection factory (WAL, `synchronous=FULL`, `foreign_keys`, `recursive_triggers`), forward-only migration with `VACUUM INTO` backup, immutability and append-only triggers, tombstone-first delete | `crates/tracr-store/src/*`, `migrations/ledger/0001_init.sql`, `tests/immutability.rs` |
| `reconcile(source)`: single open, continuity proof (identity, size ≥ offset, prefix-length `head_fp`, `last_line_fp`), tail read / full re-read, gap committed with cursor reset and batch 1, `INSERT OR IGNORE` by (source, key, hash), key-conflict diagnostic, `incomplete_trailing`, ended sources, `reconcile_run` records | `crates/tracr-ingest/src/{reconcile,cursor,reader}.rs` |
| Discovery over approved roots (locators, locator conflicts, unreadable dirs counted), `import_started`, `discovery_completed`, stat sweep with `sweep_completed` | `crates/tracr-ingest/src/{discovery,source,sweep}.rs` |
| Claude Code adapter: path classification (main, subagent, workflow agent; never `journal.jsonl`), allowlist extractor with structural markers, human-prompt rule, bounded quarantine, manifest `2.1.231..=2.1.281`, shape checks | `crates/tracr-provider-claude/src/*` |
| Fixture corpus (4 producer versions, structure only) and its generator | `crates/tracr-provider-claude/fixtures/` |
| `Clock` (system, manual), `FileIdentity` (unix `dev:ino`), macOS `AppPaths`, `unsupported` fallbacks | `crates/tracr-platform/src/*` |
| Test kit: provider lines, scenario time, file manipulations (append, partial, atomic replace, rotate) | `crates/tracr-testkit/src/*` |
| CLI `tracr roots add --provider|--repo`, `import`, `sources`, `stats [--json]`; `--data-dir` / `TRACR_DATA_DIR` | `apps/cli/src/main.rs` |

Nothing derives time or money. `stats` shows record counts only, including usage-bearing record counts, but no token sums: streaming re-emissions are elected in S1.3.

## Deviations from the architecture text

1. **`tracr-app` crate** (not in the T-0004 file list). §1.2 requires hosts to talk only to `tracr-app`, so the CLI goes through it. The crate owns the single-writer lock (`tracr.lock`), root approval and the import use case. User-approved.
2. **`source_cursor_occurrence` table** in `0001_init.sql` (not in §3.2). Lines without a native ID get the key `h:<hash>#<occurrence>`. The occurrence counts are operational state like the cursor and are written in the same transaction. Without them, a re-read that crashed and resumed as a tail read would restart the numbering and insert duplicates as new observations. Test: `crash_during_a_full_reread_neither_doubles_nor_invents_a_gap`.
3. **"File gone, no locator found by discovery" is an explicit confirmation** (`ReconcileOptions.discovery_confirmed`). Without it, a missing file makes the source `missing` (`ReadMode::Missing`) and nothing irreversible is written. The sweep never confirms. `tracr import` confirms only if discovery walked every approved root completely: no missing root (for example an unmounted volume) and no unreadable directory. Tests: `moved_file_is_missing_for_the_sweep_and_continues_at_its_new_path`, `a_missing_root_never_ends_its_sources`.

## Interpretations where the text is not explicit

- **Gap end, "records not seen before".** Judged by record key. A rewritten version of a record already held is no evidence about the unread period. Test: `rewritten_last_record_is_no_proof_of_history`.
- **Gap start without a record time.** `last_read_at` is used, for `deleted_before_read` as well. A source that was never read and is then deleted is ended with `unread_bytes_lost: true` and no gap, because there is no start time.
- **No record ingested yet.** History is never proven, per the literal §4.3 rule, even if the new file starts with the same ignored lines.
- **Human-prompt rule (§13.2) details, all in the manifest:**
  - `command-message` is a human tag: current producers lead slash commands and skills with it.
  - `promptSource: queued` is human.
  - Each of `origin.kind`, `turnOrigin` and `promptSource` alone can mark a record automated.
  - The only interactive entrypoint is `cli`. `claude-vscode` exists locally, but no fixture covers it (AD-19).
- **Record types.**
  - Extracted: `worktree-state` (including exit, `worktreeSession: null`), `relocated`, `continued-in`, `pr-link`, `queue-operation`.
  - Ignored: 12 UI or content types.
  - Quarantined: unknown types, for example `fork-context-ref`.
- **Stream-level producer version (AD-19).** Records before the first versioned record of a stream have no version and are labeled `unvalidated(none)`.

## Findings for later tasks (not implemented)

- **`fork-context-ref`** (39 records in the local history) is quarantined as an unknown type. Its key names (`parentSessionId`, `parentLastUuid`, `contextLength`) mark copied context, which is input for S1.3 copy election.
- **Copied records.** Forked or resumed sessions repeat earlier records: two local sessions share a first record time. The ledger keeps both copies, and election is S1.3.
- **Resumed subagents re-append their earlier records.** Identical lines dedupe. Changed ones (mostly `tool_finished`) become key conflicts; 56 were seen live. S1.3 must elect per (source, record key), or tool events are counted twice.
- **A no-op import of about 450 sources takes about 25 s.** Each source is at least one `synchronous=FULL` transaction. This is not a T-0004 requirement.
