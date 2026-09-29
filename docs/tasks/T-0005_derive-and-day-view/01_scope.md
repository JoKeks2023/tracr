# Scope

In scope:

- `tracr-core`:
  - normalized events and the `Normalizer` trait (`normalize.rs`);
  - copy election (`copy.rs`);
  - agent timelines (`agent.rs`);
  - `derived_id`.
- `tracr-provider-claude`:
  - normalizer (`normalize.rs`);
  - the human-prompt rule decided from retained markers (`rules.rs`), shared by the extractor and the normalizer.
- `tracr-store`: `derived.sqlite` schema (`schema/derived.sql`), create-next / install-by-rename, typed writer and readers, canonical dump (INV-6).
- `tracr-app`: full rebuild (`derive.rs`), day queries and DTOs (`day.rs`).
- CLI: `tracr rebuild`, `tracr days`, `tracr sessions [--day] [--json]`.
- `apps/desktop`: Tauri Day view shell (second milestone of this task).

Deviations from the architecture text, explicit:

1. **Order.** The Day view comes before S1.4–S1.6 (user decision, [00_brief](00_brief.md)). The UI depends only on provider-free `tracr-app` DTOs, so Codex (S1.6) plugs in without UI changes.
2. **`tracr-git` probes deferred** from S1.3 to S1.4, next to structural attribution, their first consumer. Nothing in S1.3 reads git context.
3. **Incremental derivation (dirty units) not built.** S1.3 does full rebuilds only. Dirty units need the runtime (S1.4). INV-6 is tested as "rebuild is deterministic and re-import changes no derived row". The "incremental equals fresh rebuild" half follows with S1.4.
4. **Reported cost figures** are not written to `derived.sqlite` yet (S1.5, with the cost lane).

Out of scope: human timeline, counting, device sensor, coverage (S1.4); pricing and cost (S1.5); Codex (S1.6); every S2+ feature.
