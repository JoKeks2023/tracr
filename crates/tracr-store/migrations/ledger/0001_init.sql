-- tracr ledger schema v1 (architecture §3.2, §3.3).
-- The ledger holds evidence and user records. It is never rebuilt.
-- Forward-only, additive migrations; evidence columns are never dropped.

CREATE TABLE machine (
  id TEXT PRIMARY KEY,
  label TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE source (                        -- one logical evidence stream
  id TEXT PRIMARY KEY,                       -- UUIDv7
  kind TEXT NOT NULL,                        -- e.g. claude_transcript | collector
  machine_id TEXT NOT NULL REFERENCES machine(id),
  stream_key TEXT NOT NULL,                  -- provider-native stream identity, never a path
  adapter TEXT NOT NULL,
  adapter_version TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  stream_meta TEXT,                          -- JSON from discovery (no content)
  UNIQUE (kind, machine_id, stream_key)
);

CREATE TABLE source_locator (
  source_id TEXT NOT NULL REFERENCES source(id),
  path TEXT NOT NULL,
  file_identity TEXT,
  birth_time INTEGER,                        -- file creation time where the OS provides it
  first_seen INTEGER,
  last_seen INTEGER,
  PRIMARY KEY (source_id, path)
);

CREATE TABLE source_cursor (                 -- mutable operational state; never an input to derivation
  source_id TEXT PRIMARY KEY REFERENCES source(id),
  path TEXT,
  file_identity TEXT,
  byte_offset INTEGER NOT NULL,
  size_at_read INTEGER,
  mtime_at_read INTEGER,
  head_fp BLOB,
  head_len INTEGER,                          -- hash of the first min(4 KiB, size at read) bytes, and that length
  last_line_fp BLOB,
  last_record_key TEXT,
  last_record_hash BLOB,
  last_record_at INTEGER,
  last_read_at INTEGER,
  ended_at INTEGER,
  updated_at INTEGER NOT NULL
);

CREATE TABLE source_cursor_occurrence (      -- operational state, committed with the cursor:
  source_id TEXT NOT NULL REFERENCES source(id), -- how many content-keyed lines of each content hash
  content_hash BLOB NOT NULL,                    -- the current file generation holds before the cursor
  seen INTEGER NOT NULL,                         -- (numbers the 'h:<hash>#<occurrence>' record keys)
  PRIMARY KEY (source_id, content_hash)
);

CREATE TABLE observation (                   -- envelope; insert-only
  seq INTEGER PRIMARY KEY AUTOINCREMENT,     -- global order; derivation watermark
  id TEXT UNIQUE NOT NULL,                   -- UUIDv7
  source_id TEXT NOT NULL REFERENCES source(id),
  record_key TEXT NOT NULL,                  -- native record ID, else 'h:' || content_hash || '#' || occurrence
  content_hash BLOB NOT NULL,                -- BLAKE3 of the line bytes without terminator (CRLF = LF)
  channel TEXT NOT NULL,                     -- transcript | os_sensor | git | manual | collector | hook
  locator_path TEXT,
  cursor_offset INTEGER,
  producer TEXT,
  producer_version TEXT,
  adapter_version TEXT NOT NULL,
  manifest_range TEXT,
  native_session TEXT,
  native_parent_session TEXT,
  native_agent TEXT,
  native_turn TEXT,
  native_item TEXT,
  native_request TEXT,
  native_message TEXT,
  native_tool_call TEXT,
  occurred_at INTEGER,                       -- as stated by the source; NULL if none
  observed_at INTEGER NOT NULL,              -- local receipt time
  collector_run_id TEXT NOT NULL,            -- id of the run's coverage_started or import_started record
  receipt_seq INTEGER NOT NULL,              -- monotonic within the collector run
  ingest_trigger TEXT NOT NULL,              -- hint | dir_hint | startup | sweep | overflow | wake | discovery | backfill
  parse_status TEXT NOT NULL,                -- ok | partial | unknown_format | failed
  payload_state TEXT NOT NULL DEFAULT 'never_stored',
  retention_class TEXT NOT NULL,
  UNIQUE (source_id, record_key, content_hash)
);
CREATE INDEX observation_source_hash ON observation (source_id, content_hash);

CREATE TABLE observation_facts (             -- retained fact body; one row per extractor version; insert-only
  observation_id TEXT NOT NULL REFERENCES observation(id),
  extractor_version TEXT NOT NULL,
  kind TEXT NOT NULL,
  subtype TEXT,
  time_quality TEXT NOT NULL,
  facts_json TEXT NOT NULL,                  -- allowlisted non-content facts (§11)
  PRIMARY KEY (observation_id, extractor_version)
);
CREATE INDEX observation_facts_kind ON observation_facts (kind, subtype);

CREATE TABLE observation_tombstone (         -- written before any evidence delete; append-only
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  id TEXT UNIQUE NOT NULL,
  source_id TEXT NOT NULL,
  record_key TEXT NOT NULL,
  kind TEXT,
  native_session TEXT,
  occurred_at INTEGER,
  removed_at INTEGER NOT NULL,
  reason TEXT NOT NULL
);

CREATE TABLE user_action (                   -- every user write; global order for composition
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  id TEXT UNIQUE NOT NULL,
  kind TEXT NOT NULL,
  actor TEXT NOT NULL,
  at INTEGER NOT NULL,
  reason TEXT,
  client TEXT NOT NULL
);

CREATE TABLE correction (                    -- append-only
  id TEXT PRIMARY KEY,
  action_seq INTEGER NOT NULL REFERENCES user_action(seq),
  axis TEXT NOT NULL CHECK (axis IN ('counting','attribution')),
  op TEXT NOT NULL,
  ui_op TEXT,
  subject_kind TEXT NOT NULL,
  range_start INTEGER,
  range_end INTEGER,
  subject_anchor TEXT,
  data TEXT,
  against TEXT
);

CREATE TABLE manual_entry_rev (
  entry_id TEXT NOT NULL,
  rev INTEGER NOT NULL,
  action_seq INTEGER NOT NULL REFERENCES user_action(seq),
  kind TEXT NOT NULL,                        -- timer | retroactive
  start_at INTEGER NOT NULL,
  end_at INTEGER,                            -- NULL while a timer runs
  target TEXT,
  category TEXT,
  category_other TEXT,
  note TEXT,
  deleted INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (entry_id, rev)
);

CREATE TABLE binding_rev (
  binding_id TEXT NOT NULL,
  rev INTEGER NOT NULL,
  action_seq INTEGER NOT NULL REFERENCES user_action(seq),
  subject_kind TEXT NOT NULL,                -- agent_session | worktree | timer
  subject_key TEXT NOT NULL,
  target TEXT NOT NULL,
  valid_from INTEGER,
  valid_to INTEGER,
  ended INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (binding_id, rev)
);

CREATE TABLE setting_rev (                   -- timezone, rule parameters, approved roots
  setting_key TEXT NOT NULL,
  rev INTEGER NOT NULL,
  action_seq INTEGER NOT NULL REFERENCES user_action(seq),
  value TEXT NOT NULL,                       -- JSON
  PRIMARY KEY (setting_key, rev)
);

-- §3.3: immutability is enforced by the database. Every ledger connection
-- sets recursive_triggers = ON so that REPLACE conflict resolution fires the
-- DELETE triggers below.

CREATE TRIGGER observation_immutable BEFORE UPDATE ON observation
  BEGIN SELECT RAISE(ABORT, 'observations are immutable'); END;
CREATE TRIGGER facts_immutable BEFORE UPDATE ON observation_facts
  BEGIN SELECT RAISE(ABORT, 'retained facts are immutable'); END;
CREATE TRIGGER observation_delete_needs_tombstone BEFORE DELETE ON observation
  WHEN NOT EXISTS (SELECT 1 FROM observation_tombstone WHERE id = OLD.id)
  BEGIN SELECT RAISE(ABORT, 'write a tombstone first'); END;
CREATE TRIGGER facts_delete_needs_tombstone BEFORE DELETE ON observation_facts
  WHEN NOT EXISTS (SELECT 1 FROM observation_tombstone WHERE id = OLD.observation_id)
  BEGIN SELECT RAISE(ABORT, 'write a tombstone first'); END;

CREATE TRIGGER tombstone_append_only_u BEFORE UPDATE ON observation_tombstone
  BEGIN SELECT RAISE(ABORT, 'tombstones are append-only'); END;
CREATE TRIGGER tombstone_append_only_d BEFORE DELETE ON observation_tombstone
  BEGIN SELECT RAISE(ABORT, 'tombstones are append-only'); END;
CREATE TRIGGER user_action_append_only_u BEFORE UPDATE ON user_action
  BEGIN SELECT RAISE(ABORT, 'user actions are append-only'); END;
CREATE TRIGGER user_action_append_only_d BEFORE DELETE ON user_action
  BEGIN SELECT RAISE(ABORT, 'user actions are append-only'); END;
CREATE TRIGGER correction_append_only_u BEFORE UPDATE ON correction
  BEGIN SELECT RAISE(ABORT, 'corrections are append-only'); END;
CREATE TRIGGER correction_append_only_d BEFORE DELETE ON correction
  BEGIN SELECT RAISE(ABORT, 'corrections are append-only'); END;
CREATE TRIGGER manual_entry_rev_append_only_u BEFORE UPDATE ON manual_entry_rev
  BEGIN SELECT RAISE(ABORT, 'revisions are append-only'); END;
CREATE TRIGGER manual_entry_rev_append_only_d BEFORE DELETE ON manual_entry_rev
  BEGIN SELECT RAISE(ABORT, 'revisions are append-only'); END;
CREATE TRIGGER binding_rev_append_only_u BEFORE UPDATE ON binding_rev
  BEGIN SELECT RAISE(ABORT, 'revisions are append-only'); END;
CREATE TRIGGER binding_rev_append_only_d BEFORE DELETE ON binding_rev
  BEGIN SELECT RAISE(ABORT, 'revisions are append-only'); END;
CREATE TRIGGER setting_rev_append_only_u BEFORE UPDATE ON setting_rev
  BEGIN SELECT RAISE(ABORT, 'revisions are append-only'); END;
CREATE TRIGGER setting_rev_append_only_d BEFORE DELETE ON setting_rev
  BEGIN SELECT RAISE(ABORT, 'revisions are append-only'); END;
