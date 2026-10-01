//! §3.3: immutability is enforced by the database. UPDATE, DELETE,
//! INSERT OR REPLACE and REPLACE INTO must abort on evidence and on every
//! append-only user-record table.

use rusqlite::{Connection, params};
use tempfile::TempDir;
use tracr_core::evidence::{
    Channel, IngestTrigger, NativeIds, ParseStatus, RetentionClass, TimeQuality,
};
use tracr_core::ids::{ContentHash, SourceId};
use tracr_core::time::UtcMillis;
use tracr_store::connection::{ledger_pragmas, open_ledger};
use tracr_store::ledger::{self, NewObservation, NewSource, NewUserAction};
use tracr_store::migrate::{self, Migration, migrate_ledger, user_version};

struct Fixture {
    _dir: TempDir,
    path: std::path::PathBuf,
    conn: Connection,
    obs_id: String,
}

fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ledger.sqlite");
    let mut conn = open_ledger(&path).unwrap();
    migrate_ledger(&mut conn, &dir.path().join("backups"), "t").unwrap();

    let machine = ledger::ensure_machine(&conn, "m1", Some("test"), UtcMillis(1)).unwrap();
    let (source, _) = ledger::ensure_source(
        &conn,
        &NewSource {
            id: "s1",
            kind: "test_transcript",
            machine_id: &machine,
            stream_key: "test:m1:session",
            adapter: "test",
            adapter_version: "0",
            created_at: UtcMillis(1),
            stream_meta: None,
        },
    )
    .unwrap();
    let hash = ContentHash::of(b"{}");
    let native = NativeIds::default();
    ledger::insert_observation(
        &conn,
        &NewObservation {
            id: "o1",
            source_id: &source.id,
            record_key: "r1",
            content_hash: &hash,
            channel: Channel::Transcript,
            locator_path: None,
            cursor_offset: Some(0),
            producer: Some("test"),
            producer_version: None,
            adapter_version: "0",
            manifest_range: None,
            native: &native,
            occurred_at: None,
            observed_at: UtcMillis(2),
            collector_run_id: "run",
            receipt_seq: 1,
            ingest_trigger: IngestTrigger::Backfill,
            parse_status: ParseStatus::Ok,
            retention_class: RetentionClass::EvidenceMetadata,
            extractor_version: "x/1",
            kind: "agent_message",
            subtype: None,
            time_quality: TimeQuality::Receipt,
            facts_json: "{}",
        },
    )
    .unwrap();
    let seq = ledger::append_user_action(
        &conn,
        &NewUserAction {
            id: "a1",
            kind: "setting",
            actor: "user",
            at: UtcMillis(3),
            reason: None,
            client: "test",
        },
    )
    .unwrap();
    ledger::append_setting(
        &conn,
        "approved_provider_roots",
        &serde_json::json!(["/x"]),
        seq,
    )
    .unwrap();
    conn.execute(
        "INSERT INTO correction (id, action_seq, axis, op, subject_kind, range_start, range_end)
         VALUES ('c1', ?1, 'counting', 'confirm_work', 'human_range', 10, 20)",
        [seq],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO manual_entry_rev (entry_id, rev, action_seq, kind, start_at, end_at)
         VALUES ('e1', 1, ?1, 'retroactive', 10, 20)",
        [seq],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO binding_rev (binding_id, rev, action_seq, subject_kind, subject_key, target)
         VALUES ('b1', 1, ?1, 'agent_session', 'k', 't')",
        [seq],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO observation_tombstone (id, source_id, record_key, removed_at, reason)
         VALUES ('tomb-other', 's1', 'r-other', 5, 'test')",
        [],
    )
    .unwrap();
    Fixture {
        _dir: dir,
        path,
        conn,
        obs_id: "o1".into(),
    }
}

fn assert_aborts(conn: &Connection, sql: &str) {
    let err = conn
        .execute(sql, [])
        .expect_err(&format!("expected abort: {sql}"));
    let text = err.to_string();
    assert!(
        text.contains("immutable") || text.contains("append-only") || text.contains("tombstone"),
        "unexpected error for `{sql}`: {text}"
    );
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn connection_factory_sets_required_pragmas() {
    let f = fixture();
    let p = ledger_pragmas(&f.conn).unwrap();
    assert_eq!(p.journal_mode, "wal");
    assert_eq!(p.synchronous, 2, "synchronous=FULL");
    assert!(p.foreign_keys);
    assert!(p.recursive_triggers);
}

#[test]
fn observations_and_facts_are_immutable() {
    let f = fixture();
    let c = &f.conn;
    assert_aborts(c, "UPDATE observation SET record_key = 'changed'");
    assert_aborts(c, "UPDATE observation SET payload_state = 'present'");
    assert_aborts(c, "DELETE FROM observation");
    assert_aborts(c, "UPDATE observation_facts SET facts_json = '{\"x\":1}'");
    assert_aborts(c, "DELETE FROM observation_facts");
    // REPLACE on the natural key and on the id both need a delete, which must abort.
    assert_aborts(
        c,
        "INSERT OR REPLACE INTO observation (id, source_id, record_key, content_hash, channel, adapter_version,
             observed_at, collector_run_id, receipt_seq, ingest_trigger, parse_status, retention_class)
         SELECT 'o2', source_id, record_key, content_hash, channel, adapter_version, observed_at, collector_run_id,
             receipt_seq, ingest_trigger, parse_status, retention_class FROM observation WHERE id = 'o1'",
    );
    assert_aborts(
        c,
        "REPLACE INTO observation (id, source_id, record_key, content_hash, channel, adapter_version,
             observed_at, collector_run_id, receipt_seq, ingest_trigger, parse_status, retention_class)
         VALUES ('o1', 's1', 'other', x'00', 'transcript', '0', 9, 'run', 9, 'backfill', 'ok', 'evidence_metadata')",
    );
    assert_aborts(
        c,
        "REPLACE INTO observation_facts (observation_id, extractor_version, kind, time_quality, facts_json)
         VALUES ('o1', 'x/1', 'changed', 'receipt', '{}')",
    );
    assert_eq!(count(c, "observation"), 1);
    let facts: String = c
        .query_row(
            "SELECT facts_json FROM observation_facts WHERE observation_id = 'o1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(facts, "{}");
}

#[test]
fn user_records_are_append_only() {
    let f = fixture();
    let c = &f.conn;
    for (table, update, replace) in [
        (
            "user_action",
            "UPDATE user_action SET actor = 'x'",
            "REPLACE INTO user_action (seq, id, kind, actor, at, client) VALUES (1, 'a1', 'k', 'x', 1, 'c')",
        ),
        (
            "correction",
            "UPDATE correction SET op = 'mark_non_work'",
            "REPLACE INTO correction (id, action_seq, axis, op, subject_kind) VALUES ('c1', 1, 'counting', 'mark_non_work', 'human_range')",
        ),
        (
            "setting_rev",
            "UPDATE setting_rev SET value = '[]'",
            "REPLACE INTO setting_rev (setting_key, rev, action_seq, value) VALUES ('approved_provider_roots', 1, 1, '[]')",
        ),
        (
            "manual_entry_rev",
            "UPDATE manual_entry_rev SET end_at = 99",
            "REPLACE INTO manual_entry_rev (entry_id, rev, action_seq, kind, start_at) VALUES ('e1', 1, 1, 'timer', 0)",
        ),
        (
            "binding_rev",
            "UPDATE binding_rev SET target = 'other'",
            "REPLACE INTO binding_rev (binding_id, rev, action_seq, subject_kind, subject_key, target) VALUES ('b1', 1, 1, 'worktree', 'k', 'x')",
        ),
        (
            "observation_tombstone",
            "UPDATE observation_tombstone SET reason = 'x'",
            "REPLACE INTO observation_tombstone (seq, id, source_id, record_key, removed_at, reason) VALUES (1, 'tomb-other', 's1', 'r', 1, 'x')",
        ),
    ] {
        let before = count(c, table);
        assert_aborts(c, update);
        assert_aborts(c, &format!("DELETE FROM {table}"));
        assert_aborts(c, replace);
        assert_aborts(
            c,
            &replace.replacen("REPLACE INTO", "INSERT OR REPLACE INTO", 1),
        );
        assert_eq!(count(c, table), before, "{table} row count changed");
    }
    let op: String = c
        .query_row("SELECT op FROM correction WHERE id = 'c1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(op, "confirm_work");
    let roots: String = c
        .query_row(
            "SELECT value FROM setting_rev WHERE setting_key = 'approved_provider_roots'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(roots, "[\"/x\"]");
}

#[test]
fn without_recursive_triggers_replace_would_rewrite_a_correction() {
    // Negative control: proves the pragma is what makes REPLACE abort.
    let f = fixture();
    drop(f.conn);
    let raw = Connection::open(&f.path).unwrap();
    raw.execute_batch("PRAGMA recursive_triggers = OFF; PRAGMA foreign_keys = OFF;")
        .unwrap();
    raw.execute(
        "REPLACE INTO correction (id, action_seq, axis, op, subject_kind) VALUES ('c1', 1, 'counting', 'mark_non_work', 'human_range')",
        [],
    )
    .expect("without recursive_triggers the REPLACE slips through");
    let op: String = raw
        .query_row("SELECT op FROM correction WHERE id = 'c1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(op, "mark_non_work");
}

#[test]
fn evidence_can_only_be_deleted_after_a_tombstone() {
    let f = fixture();
    let c = &f.conn;
    assert_aborts(
        c,
        "DELETE FROM observation_facts WHERE observation_id = 'o1'",
    );
    c.execute(
        "INSERT INTO observation_tombstone (id, source_id, record_key, removed_at, reason)
         VALUES (?1, 's1', 'r1', 10, 'user_directed_deletion')",
        params![f.obs_id],
    )
    .unwrap();
    c.execute(
        "DELETE FROM observation_facts WHERE observation_id = 'o1'",
        [],
    )
    .unwrap();
    c.execute("DELETE FROM observation WHERE id = 'o1'", [])
        .unwrap();
    assert_eq!(count(c, "observation"), 0);
    assert_eq!(count(c, "observation_tombstone"), 2);
}

#[test]
fn duplicate_insert_is_ignored_not_replaced() {
    let f = fixture();
    let source = SourceId::new("s1");
    let hash = ContentHash::of(b"{}");
    let native = NativeIds::default();
    let outcome = ledger::insert_observation(
        &f.conn,
        &NewObservation {
            id: "o-dup",
            source_id: &source,
            record_key: "r1",
            content_hash: &hash,
            channel: Channel::Transcript,
            locator_path: None,
            cursor_offset: Some(0),
            producer: None,
            producer_version: None,
            adapter_version: "0",
            manifest_range: None,
            native: &native,
            occurred_at: None,
            observed_at: UtcMillis(99),
            collector_run_id: "run2",
            receipt_seq: 1,
            ingest_trigger: IngestTrigger::Backfill,
            parse_status: ParseStatus::Ok,
            retention_class: RetentionClass::EvidenceMetadata,
            extractor_version: "x/1",
            kind: "agent_message",
            subtype: None,
            time_quality: TimeQuality::Receipt,
            facts_json: "{}",
        },
    )
    .unwrap();
    assert_eq!(outcome, ledger::InsertOutcome::Duplicate);
    let observed: i64 = f
        .conn
        .query_row(
            "SELECT observed_at FROM observation WHERE record_key = 'r1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(observed, 2, "the original envelope is untouched");
}

#[test]
fn migrations_are_forward_only_with_backup_before_migrating() {
    let f = fixture();
    let mut conn = f.conn;
    let latest = migrate::LEDGER_MIGRATIONS.last().unwrap().version;
    assert_eq!(user_version(&conn).unwrap(), latest);
    let dir = f.path.parent().unwrap().join("backups");
    let again = migrate_ledger(&mut conn, &dir, "t2").unwrap();
    assert_eq!(
        (again.from_version, again.to_version, again.backup),
        (latest, latest, None)
    );

    // A synthetic next migration on top of the real ones.
    let mut next: Vec<Migration> = migrate::LEDGER_MIGRATIONS.to_vec();
    next.push(Migration {
        version: latest + 1,
        name: "test_additive",
        sql: "CREATE TABLE test_additive (x INTEGER);",
    });
    let report = migrate::migrate(&mut conn, &next, &dir, "t3").unwrap();
    assert_eq!(
        (report.from_version, report.to_version),
        (latest, latest + 1)
    );
    let backup = report
        .backup
        .expect("backup before migrating an existing ledger");
    assert!(backup.exists());
    let old = Connection::open(&backup).unwrap();
    assert_eq!(user_version(&old).unwrap(), latest);
    assert_eq!(count(&old, "observation"), 1);

    let err = migrate_ledger(&mut conn, &dir, "t4").expect_err("ledger newer than build");
    assert!(err.to_string().contains("newer"));
}

#[test]
fn reviewed_revisions_and_dismissals_are_append_only() {
    let f = fixture();
    let c = &f.conn;
    c.execute(
        "INSERT INTO user_action (id, kind, actor, at, client) VALUES ('ua-r', 'review', 'user', 1, 't')",
        [],
    )
    .unwrap();
    let seq: i64 = c
        .query_row("SELECT seq FROM user_action WHERE id = 'ua-r'", [], |r| {
            r.get(0)
        })
        .unwrap();
    c.execute(
        "INSERT INTO worklog_revision (id, day, rev, action_seq, timezone, state, snapshot, fingerprint)
         VALUES ('w1', '2026-09-28', 1, ?1, 'UTC', 'reviewed', '{}', 'f1')",
        [seq],
    )
    .unwrap();
    c.execute(
        "INSERT INTO discrepancy_dismissal (id, day, revision_id, fingerprint, action_seq)
         VALUES ('d1', '2026-09-28', 'w1', 'f2', ?1)",
        [seq],
    )
    .unwrap();
    for sql in [
        "UPDATE worklog_revision SET snapshot = '{\"changed\":1}'",
        "DELETE FROM worklog_revision",
        "REPLACE INTO worklog_revision (id, day, rev, action_seq, timezone, state, snapshot, fingerprint)
         VALUES ('w1', '2026-09-28', 1, 1, 'UTC', 'reviewed', '{}', 'x')",
        "UPDATE discrepancy_dismissal SET fingerprint = 'x'",
        "DELETE FROM discrepancy_dismissal",
    ] {
        assert_aborts(c, sql);
    }
}
