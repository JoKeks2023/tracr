# Implementation report

## #32 review save (commit 1e36b86)
Cause: not the job queue. `TRACR_JOB_TRACE=1` showed the review job itself took 13–24 s with nothing queued before it; `sample` showed the time in `pread` under five ledger queries of `derive::update` and `derive_human`, each scanning the observation table (184k rows, 300 MB): `sources_sharing_records` 40 %, `parent_links` 18 %, `degraded_extents` 12 + 22 %, `recovered_record_times` 39 % (second sample), `collector_and_sensor_rows` 8 %. Real time far above CPU time = waiting on page reads, which matches "idle CPU".

Change:
- Ledger migration **v4** `0004_derive_indexes.sql`: five partial indexes, additive (+~20 MB on the real ledger; the usual pre-migration backup is written).
- Derived schema **v7**: `session_id` indexes on `usage_record`, `agent_segment`, `engagement_instant` (their per-session deletes scanned the tables). One full rebuild on first start.
- `TRACR_JOB_TRACE=1` prints each collector job's run time (stderr); `examples/time_review` times reviews against a copied data dir.
- Test `tracr-store/tests/query_plans.rs`: query plans of the five queries contain no table scan (fails without v4).

Rejected: skipping the rebuild before a review when nothing is dirty. While agents run, the store is almost always dirty, so it would not help in practice.

Left as is: the rest (3–4.6 s) is re-deriving the dirty session itself: `delete_session` + rewrite of all its rows, large for a long active session. `collector_and_sensor_rows` reads all collector rows on every update and grows with history. Both would need a change to the update model.

## #31 frozen figures (commit 75cb986)
- `App.tsx`: `shownWindows` counts the snapshot's unknown intervals on a reviewed day; the live count still drives the resolve action.
- `v2/Home.tsx`: same for the Today "Ungeklärt" note.
- `v2/DayTools.tsx` + i18n: on a reviewed day the panel header says "Aktueller Stand; Prüfung rN bleibt gespeichert" / "Current state; review rN stays as saved".
