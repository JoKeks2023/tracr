# T-0004 — S1.0–S1.2: workspace, ledger, Claude Code reconcile

Date: 2026-09-29. Request: implement exactly the T-0004 scope defined under "Implementation Start" in [docs/architecture.md](../../architecture.md#implementation-start). The architecture phase is complete and is not redesigned here.

Deliver:

- the Cargo workspace and CI;
- the `tracr-core` evidence vocabulary and adapter traits;
- the `tracr-store` ledger with connection factory, migration and immutability triggers;
- `tracr-ingest`: cursors, continuity proof, gap-first commits, ended sources, stat sweep with `sweep_completed`, `import_started`, `reconcile(source)`;
- `tracr-provider-claude`: discovery, allowlist extractor, manifest ranges, shape checks, fixture corpus;
- `tracr-platform`: `Clock`, `FileIdentity` (unix), macOS `AppPaths`;
- the `tracr` CLI: `roots add --provider | --repo`, `import`, `sources`, `stats`.

Dogfood the importer on the real local Claude Code history. No commit or push.

User additions during the task:

- Include workflow agent transcripts (`<session>/subagents/workflows/wf_*/agent-*.jsonl`) in discovery, never the adjacent `journal.jsonl`.
- Keep the human-prompt rule using `origin.kind`, `turnOrigin` and `promptSource`.
- Keep `tracr-app` as a documented deviation from the T-0004 file list.
