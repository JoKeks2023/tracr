# Validation report

Date: 2026-09-29. Subject: [docs/architecture.md](../../architecture.md) against specification v0.2.

## Mechanical checks

| Check | Method | Result |
|---|---|---|
| Mermaid syntax | `mermaid@11` `mermaid.parse()` under jsdom, run on every ```` ```mermaid ```` block. The checker was first shown to reject two deliberately broken diagrams. | 4 of 4 diagrams parse (3 flowcharts, 1 erDiagram) |
| Links and anchors | Script over all `docs/**/*.md`: relative file targets + GitHub-slug heading anchors | All resolve (0 problems after this report was written) |
| Code fences, tables | Fence balance; column count per table row; unescaped `\|` inside code spans in table rows | Balanced; 0 column mismatches; 0 unescaped pipes |
| Decision IDs | AD-01…AD-21 contiguous, each defined once | OK |
| `research/` untouched | `git status --short research/`, `git diff research/` | Empty |

## Adversarial review (model-based, three rounds)

| Round | Setup | Raised | Confirmed / fixed |
|---|---|---|---|
| 1 | 4 lens reviewers (semantics; ingestion/identity/storage; cost/integrations/privacy/platform; implementability), each followed by a refute-first skeptic | 47 | 41 confirmed or partly confirmed (17 major, 24 minor), 6 refuted. All 41 fixed in a full rewrite. |
| 2 | Fix verifier (41 items) + fresh regression sweep with arithmetic re-derivation | 6 incomplete fixes + 17 new (5 major) | All addressed by targeted edits |
| 3 | One verifier on the 23 round-2 items + the edited sections | 1 partial + 9 new (2 major) | All addressed by targeted edits |

The main defects found and fixed:

| Area | Defect found | Fix |
|---|---|---|
| Day span | The ±max load window lost unknown windows crossing midnight | Neighbouring-instant span, no look-back limit |
| Bridging | Coverage was checked on one channel only | All engagement-capable kinds and streams, degraded extents and removed-evidence extents |
| Rejections | They cut attribution resolution short | A suppression set, ordered with set operations; `R_run` for suggestions |
| Copied context | Created phantom engagement instants | Excluded for all agent facts; containment → birth time election |
| Usage IDs | Copies collided | Per-occurrence IDs |
| Observation identity | The content hash was missing | Content hash added to the identity |
| Gaps | Lost on a crash mid re-read | Committed with the cursor reset |
| Completeness | Read from a mutable cursor | Derived from `sweep_completed` / `reconcile_run` / `discovery_completed` records |
| Deleted transcripts | Froze the watermark or created month-long gaps | Ended sources; gap bounded by `mtime_at_read` |
| Provider-reported cost | Figures were summable | No `Add`; latest snapshot only |
| Unpriced aggregates | Could read as 0 | `Option` amount; CHECK constraint |
| Repository probes | Ran without approval | Approved repository roots |
| Producer versions | No fallback for unvalidated versions | AD-19, stream-level version |
| `main` actor | Assumed from C-level structure | AD-21: documented semantics only |
| Test harness | No way to inject coverage | Scenario-time collector records + explicit `as_of` |
| Walkthrough G | Fixture had no S1 ingestion path | G-r in S1, G proper with spool (S4) |

## Arithmetic re-derived from the written rules (round 2 sweep, re-checked in round 3)

| Scenario | Result |
|---|---|
| B unresolved (cmax 17) | 87 counted / 20 unknown |
| C before review (cmax 15) | 52 / 13; S1 structural targets tracr-app 25, server 27 |
| Interleaved | 0 / 25 `conflicting_evidence` |
| Manual-timer variant | 97 (57 inferred + 40 timer) / 10 unknown |
| F restart branch | 14:06–14:14 `coverage_gap`, 8 min |
| F3 | 7-minute gap, 9 unknown minutes |
| Lock scenario | 11:55–12:00 and 13:00–13:05 unknown; the lock period blank |
| H | r1 = 100 (A 57, B 43, non-work 7); pure re-derivation 90 counted / 17 unknown; resolved 100 (never 90 or 103); H2 keeps 10:02–10:15 on B |

## Final checks against the request

| # | Check | Result |
|---|---|---|
| 1 | v0.2 semantics | Preserved; open product questions only get labeled 0.9 defaults ([architecture §0](../../architecture.md#0-decisions-at-a-glance)) |
| 2 | Walkthroughs A–H | All mapped with owner, fixture, must-hold and slice (§12.2) |
| 3 | Parallel sessions first-class | No singleton; every agent record keyed by session + actor |
| 4 | Human time not double-counted | One timeline, non-overlapping atoms, joint once, report identity |
| 5 | Unknown not counted | Type-level: `Unknown` has no route to `CountedMinutes`; bridging needs complete coverage |
| 6 | Counting ≠ attribution | Separate axes and resolvers; bindings attribution-only; AT-10 by signature |
| 7 | Corrections durable | Append-only with triggers (`recursive_triggers` on); range/natural-key anchors; visible orphans |
| 8 | Raw evidence not rewritten | Insert-only, tombstone-first; same key + new content is inserted, never dropped |
| 9 | Usage/cost provenance | Per-record estimates with pricing revision and price basis; election rules; kinds as types; no total |
| 10 | macOS-first without Windows lock-in | macOS code only in `tracr-platform::macos`; Windows `cargo check` in CI |
| 11 | S1 implementable | T-0004 scope, files and done-when are stated |

## Not verified

- **Model-based review only.** It raises confidence in internal consistency against the spec. It does not prove correctness against live systems.
- **No code exists yet.** The Rust toolchain is not installed, and no schema, trigger or algorithm has been executed. The SQLite `REPLACE`/`recursive_triggers` behavior was checked by a round-1 verifier with `sqlite3`; nothing else was run.
- **Provider-format claims** rest on key and type names in local files (2026-09-29). Classification rules (human prompt, interactive source, copy markers, Codex subagent linkage) still need the fixture validation planned in S1.2/S1.3/S1.6.
- **Platform APIs** (`HIDIdleTime`, `CGSessionCopyCurrentDictionary`, IOKit power notifications, `kern.sleeptime`/`kern.waketime`) are named from general platform knowledge. They are not tested on this machine (OQ-I-10).
