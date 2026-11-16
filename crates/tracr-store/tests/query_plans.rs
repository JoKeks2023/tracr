//! The queries the in-place derived update runs on every edit must not scan
//! the observation table (GH #32: 10 s to 2 min per review on a 300 MB
//! ledger). The SQL mirrors `ledger.rs`; keep both in step.

use rusqlite::Connection;
use tempfile::TempDir;
use tracr_store::connection::open_ledger;
use tracr_store::migrate::migrate_ledger;

fn migrated() -> (TempDir, Connection) {
    let dir = TempDir::new().unwrap();
    let mut conn = open_ledger(&dir.path().join("ledger.sqlite")).unwrap();
    migrate_ledger(&mut conn, &dir.path().join("backups"), "t").unwrap();
    (dir, conn)
}

fn plan(conn: &Connection, sql: &str) -> Vec<String> {
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
    stmt.query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// A plain table scan reads every row; a scan of a (partial) index does not.
fn assert_no_table_scan(conn: &Connection, name: &str, sql: &str) {
    let lines = plan(conn, sql);
    for l in &lines {
        let scans_table = l.starts_with("SCAN ") && !l.contains("INDEX");
        assert!(!scans_table, "{name} scans a table: {lines:?}");
    }
}

#[test]
fn derived_update_queries_use_indexes() {
    let (_dir, conn) = migrated();
    let cases = [
        (
            "sources_sharing_records",
            "SELECT DISTINCT o2.source_id FROM observation o1
             JOIN observation o2 ON o2.record_key = o1.record_key AND o2.source_id <> o1.source_id
             WHERE o1.source_id IN ('a', 'b') AND o1.record_key NOT LIKE 'h:%'
               AND o1.channel = 'transcript' AND o2.channel = 'transcript'",
        ),
        (
            "parent_links",
            "SELECT DISTINCT source_id, native_parent_session FROM observation
             WHERE channel = 'transcript' AND native_parent_session IS NOT NULL",
        ),
        (
            "degraded_extents",
            "SELECT o.source_id, o.seq, o.occurred_at FROM observation o
             WHERE o.channel = 'transcript' AND o.parse_status <> 'ok'",
        ),
        (
            "degraded_extents timed records",
            "SELECT seq, occurred_at FROM observation
             WHERE source_id = 'a' AND occurred_at IS NOT NULL ORDER BY seq",
        ),
        (
            "recovered_record_times",
            "SELECT occurred_at FROM observation WHERE channel = 'transcript' AND occurred_at >= 0
               AND ingest_trigger NOT IN ('hint', 'dir_hint')",
        ),
    ];
    for (name, sql) in cases {
        assert_no_table_scan(&conn, name, sql);
    }
}
