-- tracr ledger schema v4: indexes for the in-place derived update (GH #32).
-- Additive; no table or row changes. Without them every update scanned the
-- whole observation table several times (copy peers, parent links, degraded
-- records): 10 s and more of page reads on a 300 MB ledger before each
-- review, correction or manual entry. Partial indexes keep the cost small.

-- sources_sharing_records: other transcript sources holding the same native record key.
CREATE INDEX observation_transcript_key ON observation (record_key, source_id)
  WHERE channel = 'transcript';

-- parent_links: child transcripts that name a parent session.
CREATE INDEX observation_transcript_parent ON observation (source_id, native_parent_session)
  WHERE channel = 'transcript' AND native_parent_session IS NOT NULL;

-- degraded_extents: transcript records that could not be extracted.
CREATE INDEX observation_transcript_degraded ON observation (source_id, seq, occurred_at)
  WHERE channel = 'transcript' AND parse_status <> 'ok';

-- degraded_extents: the timed records of an affected source, in seq order.
CREATE INDEX observation_source_timed ON observation (source_id, seq, occurred_at)
  WHERE occurred_at IS NOT NULL;

-- recovered_record_times: transcript records first read by a non-live trigger.
CREATE INDEX observation_transcript_recovered ON observation (occurred_at)
  WHERE channel = 'transcript' AND ingest_trigger NOT IN ('hint', 'dir_hint');
