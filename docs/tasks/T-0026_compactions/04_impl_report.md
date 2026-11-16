# T-0026 Compactions on the session (TRACR-77, follow-up of T-0025)

Claude Code's compaction requests are not in the transcript, so the estimate leaves them out (T-0025). tracr now **says** so per session instead of inventing a figure.

- **Gate (OQ-I-07):** the committed fixtures already contain `compact_boundary` records with `compactMetadata` for every producer version of the validated range (2.1.231, 2.1.236, 2.1.267, 2.1.281). The mapping applies to fixture-validated records only. Records from other versions keep the fallback (no event) and count a diagnostic `compaction_not_mapped_unvalidated_version`.
- **Normalizer `claude-normalize/2`:** `context_compacted` events (subtype `compact_boundary`) from system events. History counts too: the subtype was already retained. Changing the normalizer version forces one full rebuild.
- **Agent time is unchanged:** the agent timeline skips `context_compacted` events. A compaction marker is not evidence of activity at its time.
- **Extractor `claude-extract/4`:** keeps `compact_pre_tokens`, `compact_post_tokens` and `compact_trigger`, only for `auto`/`manual`. Other trigger strings are a flag only: the privacy test caught the free-text channel. Applies to new lines only.
- **Day view:** `SessionDay.compactions` comes from `norm_event` through the `(session_id, occurred_at)` index. TS type added; **no UI text yet**. Suggested copy: "1 compaction not included in this estimate".
- **Tests:** normalizer (validated → event; unvalidated → none plus diagnostic); app (`compactions` = 1, agent time and estimate identical to the same session without it); extraction privacy test passes with the fixtures.

Stacked on `t-0021-usage-speed-geo` (extractor /3 → /4).

## Checks
`cargo test --workspace --exclude tracr-desktop` (43 suites ok), clippy `-D warnings` clean, `pnpm typecheck` ok. Not verified: a real ledger after the full rebuild (machine busy), UI.
