# Claude Code transcript fixtures

Structure-only fixtures reduced from real local Claude Code transcripts on 2026-09-29 (architecture §12.1). One directory per producer version: 2.1.231, 2.1.236, 2.1.267 and 2.1.281. Together they validate the manifest range `2.1.231..=2.1.281` (AD-19).

| File | Contents |
|---|---|
| `main.jsonl` | Contiguous prefix (≤ 200 lines) of one real session transcript |
| `records.jsonl` | Up to 2 samples per structural signature (record type, content shape and leading tag, meta/origin markers, entrypoint, …) across all sessions of that version |
| `subagent.jsonl` | Prefix (≤ 60 lines) of one subagent or workflow-agent transcript |
| `expected.json` | Outcome counts from the reference classifier in `sanitize.py`, a Python re-implementation of the documented rule (a check that the Rust code implements the rule, not an independent oracle) |

`sanitize.py` produced these files. It remaps IDs consistently and replaces paths and branch names with fixture values. It keeps enum-like fields, model names, builtin tool names, token counts and timestamps. Every other string becomes a unique identifier-shaped sentinel `cx<n>zz`, preserving only a leading envelope tag from the adapter's allowlist. The sentinel passes the extractor's identifier validators on purpose, so a leak through a marker, link or tool-name channel would be caught. `tests/extract.rs` asserts that no retained fact contains a sentinel.

To regenerate: `python3 sanitize.py ~/.claude/projects .` (reads local transcripts; review the diff before committing).
