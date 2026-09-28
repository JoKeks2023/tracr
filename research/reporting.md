# Development diary and reporting

Research date: **2026-09-28**. Evidence: [T1](SOURCES.md#t1--claude-code-timelog), [T2](SOURCES.md#t2--claudoscope-native-macos), [T4](SOURCES.md#t4--claudescope-multi-agent-explorer), [T5](SOURCES.md#t5--claude-usage), [T8](SOURCES.md#t8--recently-createdadditional-candidates).

## Existing patterns

| Pattern | Concrete precedent | Tradeoff |
|---|---|---|
| Recompute reports from events | Timelog report reads JSONL and builds slices | Replayable, but gaps/deleted logs affect history |
| Persistent derived index | Claudoscope summary cache; phuryn SQLite turns | Fast queries; needs schema/source invalidation |
| Normalized analytical projections | Multi-agent Claudescope/DuckDB | Cross-provider queries; central schema discipline |
| Daily snapshot warehouse | physcom dashboard README claim | Survives source deletion; cannot recover missing session detail |

The first three are source-inspected examples, not proof of a universal mature-system practice. The snapshot warehouse is discovery evidence only. No commercial developer-summary backend was reverse-engineered.

## Candidate report content

| Report section | Evidence needed | Honest wording |
|---|---|---|
| Projects/features | Confirmed attribution + unresolved candidates | Separate unattributed time |
| Human effort | Resolved interval union | Show automatic/manual/mixed provenance |
| Agent work | Session/turn execution and usage | Separate from human hours |
| Delivery | Commit/PR/issue status evidence | “PR opened/merged”; not “finished” from a prompt |
| AI usage/cost | Deduplicated usage + cost kind | Include unpriced usage and coverage |
| Unfinished work | Open issue/PR, interrupted turn, user note | Avoid equating silence with completion |
| Timeline | Segment boundaries and uncertainty | Mark estimated boundaries |
| Diary narrative | Facts plus user edits | Link each generated assertion to evidence |

**P:** A deterministic daily factual report is a sensible baseline. Optional narrative generation should receive a minimal fact bundle, not arbitrary transcripts by default. Store generated text, generator/version, input revision and user edits separately. A generated narrative is not authoritative issue state.

## Raw events versus snapshots

**P:** Evaluate a replayable event/index model with optional finalized snapshots:

`source evidence → normalized facts → resolved worklog → daily projection → optional finalized report`

A projection cache may be rebuilt. A finalized report or user diary edit cannot be discarded as cache. A snapshot should record reporting timezone, rule versions, source watermark, coverage, price revision and generation timestamp. Late-arriving data creates a revised report with visible changes rather than silently altering a published timesheet.

If raw payload is deleted, retain allowed minimal usage/interval facts and provenance summaries. Mark which details can no longer be replayed. Retention is a product policy, not an excuse to promise both complete deletion and unlimited reconstruction.

## Daily/weekly boundary rules

Split intervals at local calendar midnights under the selected timezone. Define week start explicitly. A weekly total must be the sum of the same resolved daily semantics, not a separate session-duration algorithm. Track ingestion watermark so “today” can show provisional data.

A 23:50–00:10 segment is two ten-minute day contributions. A DST day may have 23 or 25 hours. Commits, issue closures and session starts are point events; an open session is not counted once per day without qualifying activity. Travel policy and historical timezone changes must be explicit.

## Quality indicators and exports

Proposed report footer: collectors active, observation gaps, attributed percentage, manual corrections, usage completeness, cost kind and unpriced models. These indicators should survive CSV/JSON export. Include stable entity IDs and original precision so exports can be reconciled.

A private daily report can contain paths, issue titles or prompt-derived summaries; sharing is a separate deliberate action. No report was published to Huly/GitHub by this research. Future export should respect finalized-worklog locks and remote synchronization limits. [Time tracking](time-tracking.md), [Huly](huly.md)
