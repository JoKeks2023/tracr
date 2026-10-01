-- tracr ledger schema v2 (architecture §2.3, §7.6): reviewed day revisions
-- and dismissed discrepancies. Additive; both tables are append-only.

CREATE TABLE worklog_revision (              -- a reviewed day, stored by value
  id TEXT PRIMARY KEY,                       -- UUIDv7
  day TEXT NOT NULL,                         -- reporting day YYYY-MM-DD
  rev INTEGER NOT NULL,                      -- 1, 2, … per day
  action_seq INTEGER NOT NULL REFERENCES user_action(seq),
  timezone TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('reviewed', 'finalized')),
  reason TEXT,                               -- why a later revision was made (accept, re-review)
  snapshot TEXT NOT NULL,                    -- JSON: intervals by value, totals, rule refs, derived as_of
  fingerprint TEXT NOT NULL,                 -- of the resolved day that was reviewed
  UNIQUE (day, rev)
);

CREATE TABLE discrepancy_dismissal (         -- "keep my revision" for one specific difference
  id TEXT PRIMARY KEY,
  day TEXT NOT NULL,
  revision_id TEXT NOT NULL REFERENCES worklog_revision(id),
  fingerprint TEXT NOT NULL,                 -- the resolved day the user dismissed
  action_seq INTEGER NOT NULL REFERENCES user_action(seq)
);

CREATE TRIGGER worklog_revision_append_only_u BEFORE UPDATE ON worklog_revision
  BEGIN SELECT RAISE(ABORT, 'revisions are append-only'); END;
CREATE TRIGGER worklog_revision_append_only_d BEFORE DELETE ON worklog_revision
  BEGIN SELECT RAISE(ABORT, 'revisions are append-only'); END;
CREATE TRIGGER discrepancy_dismissal_append_only_u BEFORE UPDATE ON discrepancy_dismissal
  BEGIN SELECT RAISE(ABORT, 'dismissals are append-only'); END;
CREATE TRIGGER discrepancy_dismissal_append_only_d BEFORE DELETE ON discrepancy_dismissal
  BEGIN SELECT RAISE(ABORT, 'dismissals are append-only'); END;
