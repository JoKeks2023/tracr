-- tracr derived store (architecture §3.4). Everything here is a pure
-- function of the ledger, the rule versions and `as_of`. It is never
-- migrated: on a schema-version mismatch it is rebuilt (§3.1).
-- S1.3 subset: normalized events, agent timelines, usage records.

CREATE TABLE derived_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE derivation_run (id TEXT PRIMARY KEY, unit TEXT NOT NULL, unit_key TEXT NOT NULL,
  as_of INTEGER NOT NULL, obs_watermark INTEGER, action_watermark INTEGER, versions TEXT NOT NULL,
  input_fingerprint BLOB NOT NULL, output_fingerprint BLOB, started_at INTEGER, finished_at INTEGER);

CREATE TABLE norm_event (id TEXT PRIMARY KEY, kind TEXT NOT NULL, subtype TEXT, provider TEXT NOT NULL,
  session_id TEXT, actor_id TEXT, turn_id TEXT, occurred_at INTEGER, time_quality TEXT NOT NULL,
  observed_at INTEGER NOT NULL, native_id TEXT, context TEXT, model TEXT, model_source TEXT,
  normalizer_version TEXT NOT NULL,
  copied_from TEXT,                                    -- canonical occurrence if this is copied context (§8.1)
  quality TEXT, extension TEXT, derivation_run_id TEXT NOT NULL);
CREATE INDEX norm_event_session ON norm_event (session_id, occurred_at);
CREATE TABLE norm_event_source (event_id TEXT NOT NULL, observation_id TEXT NOT NULL,
  PRIMARY KEY (event_id, observation_id));

CREATE TABLE agent_session (id TEXT PRIMARY KEY, provider TEXT NOT NULL, machine_id TEXT NOT NULL,
  native_session_id TEXT NOT NULL, parent_session_id TEXT, first_evidence_at INTEGER,
  last_evidence_at INTEGER, end_observed_at INTEGER, end_basis TEXT,
  is_subagent_thread INTEGER NOT NULL DEFAULT 0, diagnostics TEXT, label TEXT,
  cwd TEXT, git_branch TEXT,                           -- event-time context of the first canonical record
  derivation_run_id TEXT NOT NULL,
  UNIQUE (provider, machine_id, native_session_id));

CREATE TABLE agent_actor (id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('known','main','unknown')),
  native_agent_id TEXT, parent_actor_id TEXT, derivation_run_id TEXT NOT NULL);
CREATE INDEX agent_actor_session ON agent_actor (session_id);

CREATE TABLE turn (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, actor_id TEXT, native_turn_id TEXT,
  ordinal INTEGER, origin TEXT, started_at INTEGER, ended_at INTEGER, end_kind TEXT, end_basis TEXT,
  context TEXT, derivation_run_id TEXT NOT NULL);
CREATE INDEX turn_session ON turn (session_id, started_at);

CREATE TABLE tool_call (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, actor_id TEXT, turn_id TEXT,
  native_id TEXT, tool_name TEXT, tool_kind TEXT, started_at INTEGER, finished_at INTEGER,
  derivation_run_id TEXT NOT NULL);
CREATE INDEX tool_call_session ON tool_call (session_id, started_at);

CREATE TABLE agent_segment (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, actor_id TEXT NOT NULL,
  turn_id TEXT, start_at INTEGER NOT NULL, end_at INTEGER NOT NULL, start_basis TEXT NOT NULL,
  end_basis TEXT NOT NULL, kind TEXT NOT NULL, rule_ref TEXT NOT NULL, derivation_run_id TEXT NOT NULL,
  CHECK (end_at > start_at));
CREATE INDEX agent_segment_time ON agent_segment (start_at, end_at);

CREATE TABLE engagement_instant (id TEXT PRIMARY KEY, at INTEGER NOT NULL, session_id TEXT NOT NULL,
  event_id TEXT NOT NULL, subtype TEXT, context TEXT, derivation_run_id TEXT NOT NULL);
CREATE INDEX engagement_instant_at ON engagement_instant (at);

CREATE TABLE usage_record (id TEXT PRIMARY KEY,        -- BLAKE3(provider, stream key, dedup identity or record key)
  provider TEXT NOT NULL, stream_key TEXT NOT NULL, dedup_identity TEXT,
  identified INTEGER NOT NULL, canonical INTEGER NOT NULL, duplicate_of TEXT,
  session_id TEXT, actor_id TEXT, turn_id TEXT, model TEXT, model_source TEXT, service_tier TEXT,
  semantics TEXT NOT NULL, scope TEXT NOT NULL,
  input_fresh INTEGER, input_total INTEGER, cache_read INTEGER, cache_creation INTEGER,
  cache_creation_tiers TEXT, output INTEGER, reasoning_output INTEGER, tool_charges TEXT,
  extent_start INTEGER, extent_end INTEGER, election_rule TEXT NOT NULL, quality TEXT,
  derivation_run_id TEXT NOT NULL,
  CHECK ((canonical = 1) = (duplicate_of IS NULL)));   -- NULL token column = unknown, 0 = observed zero
CREATE INDEX usage_record_extent ON usage_record (extent_start);
CREATE TABLE usage_record_source (usage_id TEXT NOT NULL, observation_id TEXT NOT NULL,
  PRIMARY KEY (usage_id, observation_id));
