//! Walkthrough F and the reconcile guarantees of architecture §4:
//! F main (lost cursor write + restart), F2 (atomic replacement),
//! F3 (rotation + crash after batch 1), F4 (partial line), restart and
//! replay idempotence, ended sources, the stat sweep, parallel streams.

use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::Connection;
use tempfile::TempDir;
use tracr_core::evidence::IngestTrigger;
use tracr_core::ids::MachineId;
use tracr_core::time::UtcMillis;
use tracr_ingest::source::RunContext;
use tracr_ingest::{
    GapInfo, IngestEnv, IngestError, ReadMode, ReconcileOptions, ReconcileReport, Registry,
    discover, reconcile, sweep,
};
use tracr_platform::{FileIdentity, ManualClock, file_identity};
use tracr_provider_claude::ClaudeAdapter;
use tracr_store::connection::open_ledger;
use tracr_store::ledger::{self, SourceRow};
use tracr_store::migrate::migrate_ledger;
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::collector::at;
use tracr_testkit::{append_lines, append_raw, atomic_replace, rotate, write_lines};

const S_A: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const S_B: &str = "bbbbbbbb-2222-4222-8222-222222222222";

struct Harness {
    dir: TempDir,
    conn: Connection,
    clock: ManualClock,
    fid: Box<dyn FileIdentity>,
    registry: Registry,
    machine: MachineId,
    run: RunContext,
}

fn t(hhmm: &str) -> UtcMillis {
    UtcMillis::parse_rfc3339(&at(hhmm)).unwrap()
}

impl Harness {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let mut conn = open_ledger(&dir.path().join("ledger.sqlite")).unwrap();
        migrate_ledger(&mut conn, &dir.path().join("backups"), "t").unwrap();
        let clock = ManualClock::new(t("14:00"));
        let fid = file_identity();
        let machine = ledger::ensure_machine(&conn, "machine-1", None, clock_now(&clock)).unwrap();
        let registry = Registry::new(vec![Arc::new(ClaudeAdapter::new())]);
        let run = {
            let env = IngestEnv {
                clock: &clock,
                file_identity: &*fid,
            };
            RunContext::begin_import(&mut conn, &env, &machine, "test").unwrap()
        };
        Self {
            dir,
            conn,
            clock,
            fid,
            registry,
            machine,
            run,
        }
    }

    fn root(&self) -> PathBuf {
        self.dir.path().join("claude-projects")
    }

    fn session_file(&self, session: &str) -> PathBuf {
        self.root()
            .join("-Users-dev-project")
            .join(format!("{session}.jsonl"))
    }

    /// A new collector run, as after a process restart.
    fn restart(&mut self) {
        drop(std::mem::replace(
            &mut self.conn,
            Connection::open_in_memory().unwrap(),
        ));
        self.conn = open_ledger(&self.dir.path().join("ledger.sqlite")).unwrap();
        let env = IngestEnv {
            clock: &self.clock,
            file_identity: &*self.fid,
        };
        self.run = RunContext::begin_import(&mut self.conn, &env, &self.machine, "test").unwrap();
    }

    fn discover(&mut self) {
        let env = IngestEnv {
            clock: &self.clock,
            file_identity: &*self.fid,
        };
        let root = self.root();
        discover(
            &mut self.conn,
            &env,
            &self.registry,
            &mut self.run,
            &self.machine,
            &[root],
            IngestTrigger::Backfill,
        )
        .unwrap();
    }

    fn source(&self, session: &str) -> SourceRow {
        ledger::sources(&self.conn, Some("claude_transcript"))
            .unwrap()
            .into_iter()
            .find(|s| s.stream_key.ends_with(session))
            .expect("source discovered")
    }

    fn try_reconcile(
        &mut self,
        session: &str,
        trigger: IngestTrigger,
        opts: &ReconcileOptions,
    ) -> Result<ReconcileReport, IngestError> {
        let source = self.source(session);
        let env = IngestEnv {
            clock: &self.clock,
            file_identity: &*self.fid,
        };
        reconcile(
            &mut self.conn,
            &env,
            &self.registry,
            &mut self.run,
            &source,
            trigger,
            opts,
        )
    }

    fn reconcile(&mut self, session: &str, trigger: IngestTrigger) -> ReconcileReport {
        self.try_reconcile(session, trigger, &ReconcileOptions::default())
            .unwrap()
    }

    fn provider_observations(&self) -> i64 {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM observation WHERE channel = 'transcript'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn collector_count(&self, kind: &str) -> i64 {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM observation_facts WHERE kind = ?1",
                [kind],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn gaps(&self) -> Vec<GapInfo> {
        ledger::coverage_gaps(&self.conn)
            .unwrap()
            .into_iter()
            .map(|g| GapInfo {
                start: g.start,
                end: g.end,
                reason: g.reason,
            })
            .collect()
    }

    /// `ingest_trigger` of the transcript observation that occurred at `hhmm`.
    fn trigger_at(&self, hhmm: &str) -> String {
        self.conn
            .query_row(
                "SELECT ingest_trigger FROM observation WHERE channel = 'transcript' AND occurred_at = ?1",
                [t(hhmm).0],
                |r| r.get(0),
            )
            .unwrap()
    }
}

fn clock_now(c: &ManualClock) -> UtcMillis {
    use tracr_platform::Clock;
    c.now()
}

// ---------------------------------------------------------------------------
// Walkthrough F
// ---------------------------------------------------------------------------

#[test]
fn f_main_lost_cursor_write_then_restart_recovers_4_and_skips_1() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    write_lines(&file, &[a.prompt("14:05")]);
    h.discover();
    h.reconcile(S_A, IngestTrigger::Hint);
    let cursor_after_1405 = ledger::cursor(&h.conn, &h.source(S_A).id).unwrap().unwrap();

    // Live processing reaches 14:06 ...
    append_lines(&file, &[a.prompt("14:06")]);
    h.reconcile(S_A, IngestTrigger::Hint);
    // ... but the cursor was last persisted after 14:05 (simulated lost cursor write).
    ledger::put_cursor(&h.conn, &cursor_after_1405).unwrap();

    // 14:07–14:20: records are written, no notifications are processed.
    append_lines(
        &file,
        &[
            a.assistant("14:08", "msg_1", 100),
            a.assistant("14:12", "msg_2", 200),
            a.prompt("14:14"),
            a.assistant("14:18", "msg_3", 300),
        ],
    );
    h.clock.set(t("14:20"));
    h.restart();
    let r = h.reconcile(S_A, IngestTrigger::Startup);

    assert_eq!(r.mode, ReadMode::Tail);
    assert_eq!(r.continuity, "holds");
    assert_eq!(
        (r.inserted, r.duplicates),
        (4, 1),
        "4 ingested, the 14:06 prompt skipped by key"
    );
    assert_eq!(h.provider_observations(), 6);
    assert!(h.gaps().is_empty(), "nothing was lost");
    for hhmm in ["14:08", "14:12", "14:14", "14:18"] {
        assert_eq!(h.trigger_at(hhmm), "startup", "non-live trigger: {hhmm}");
    }
    assert_eq!(
        h.trigger_at("14:06"),
        "hint",
        "the live-ingested record keeps its trigger"
    );
}

#[test]
fn f2_atomic_replacement_is_not_deletion_and_doubles_nothing() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    let lines = vec![
        a.prompt("14:05"),
        a.assistant("14:08", "msg_1", 100),
        a.prompt("14:14"),
    ];
    write_lines(&file, &lines);
    h.discover();
    assert_eq!(h.reconcile(S_A, IngestTrigger::Backfill).inserted, 3);

    // The provider rewrites the transcript and renames it over the original.
    let mut content: String = lines.iter().map(|l| format!("{l}\n")).collect();
    content.push_str(&format!("{}\n", a.assistant("14:30", "msg_2", 50)));
    atomic_replace(&file, content.as_bytes());
    let r = h.reconcile(S_A, IngestTrigger::Startup);
    assert_eq!(r.mode, ReadMode::FullReread);
    assert_eq!(r.continuity, "identity_changed");
    assert_eq!(
        r.gap, None,
        "history preserved: the last record is still there"
    );
    assert_eq!((r.inserted, r.duplicates), (1, 3));
    assert_eq!(h.provider_observations(), 4);

    // The same records rewritten with CRLF line endings hash the same (§3.2).
    atomic_replace(&file, content.replace('\n', "\r\n").as_bytes());
    let r = h.reconcile(S_A, IngestTrigger::Startup);
    assert_eq!((r.inserted, r.duplicates, r.gap.clone()), (0, 4, None));
    assert_eq!(h.provider_observations(), 4);
    assert!(h.gaps().is_empty());
}

#[test]
fn f3_rotation_before_read_is_a_7_minute_gap_that_survives_a_crash() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    write_lines(&file, &[a.prompt("14:05")]);
    h.discover();
    h.reconcile(S_A, IngestTrigger::Hint);
    // Records after 14:05 are written but never read ...
    append_lines(
        &file,
        &[
            a.prompt("14:06"),
            a.assistant("14:08", "msg_1", 10),
            a.assistant("14:11", "msg_2", 20),
        ],
    );
    // ... and rotation removes them before reconciliation reads them.
    rotate(
        &file,
        &[
            a.assistant("14:12", "msg_3", 30),
            a.prompt("14:14"),
            a.assistant("14:18", "msg_4", 40),
        ],
    );

    h.clock.set(t("14:20"));
    let crash = ReconcileOptions {
        batch_size: 1,
        fail_after_batches: Some(1),
        ..ReconcileOptions::default()
    };
    let err = h
        .try_reconcile(S_A, IngestTrigger::Startup, &crash)
        .expect_err("simulated crash");
    assert!(matches!(err, IngestError::SimulatedCrash(1)));
    let expected_gap = GapInfo {
        start: t("14:05"),
        end: t("14:12"),
        reason: "rotated_before_read".into(),
    };
    assert_eq!(
        h.gaps(),
        vec![expected_gap.clone()],
        "the gap committed with batch 1"
    );
    assert_eq!(
        h.provider_observations(),
        2,
        "14:05 and the first batch (14:12)"
    );

    h.restart();
    let r = h.reconcile(S_A, IngestTrigger::Startup);
    assert_eq!(
        r.mode,
        ReadMode::Tail,
        "continuity holds against the cursor committed with the gap"
    );
    assert_eq!((r.inserted, r.gap.clone()), (2, None));
    let gaps = h.gaps();
    assert_eq!(gaps, vec![expected_gap], "exactly one gap after restart");
    assert_eq!(gaps[0].end.0 - gaps[0].start.0, 7 * 60 * 1000, "7 minutes");
    assert_eq!(h.provider_observations(), 4);
    for lost in ["14:06", "14:08", "14:11"] {
        let n: i64 = h
            .conn
            .query_row(
                "SELECT COUNT(*) FROM observation WHERE occurred_at = ?1",
                [t(lost).0],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "{lost} was never read and is not invented");
    }
}

#[test]
fn f4_partial_trailing_line_is_ingested_once_when_complete() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    let complete = a.prompt("14:05");
    let pending = a.assistant("14:20", "msg_1", 42);
    let (head, tail) = pending.split_at(pending.len() / 2);
    write_lines(&file, std::slice::from_ref(&complete));
    append_raw(&file, head.as_bytes());
    h.discover();

    let r = h.reconcile(S_A, IngestTrigger::Hint);
    assert_eq!((r.inserted, r.incomplete_trailing), (1, true));
    let cursor = ledger::cursor(&h.conn, &h.source(S_A).id).unwrap().unwrap();
    assert_eq!(
        cursor.byte_offset,
        complete.len() as u64 + 1,
        "cursor stays before the partial line"
    );
    // Re-reading while still partial records the diagnostic once, and never quarantines it.
    let r = h.reconcile(S_A, IngestTrigger::Sweep);
    assert_eq!((r.inserted, r.quarantined), (0, 0));
    assert_eq!(h.collector_count("incomplete_trailing"), 1);

    append_raw(&file, format!("{tail}\n").as_bytes());
    let r = h.reconcile(S_A, IngestTrigger::Hint);
    assert_eq!((r.inserted, r.incomplete_trailing), (1, false));
    assert_eq!(h.reconcile(S_A, IngestTrigger::Sweep).inserted, 0);
    assert_eq!(h.provider_observations(), 2);
    let status: String = h
        .conn
        .query_row(
            "SELECT parse_status FROM observation WHERE occurred_at = ?1",
            [t("14:20").0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "ok");
}

// ---------------------------------------------------------------------------
// Restart, replay and identity
// ---------------------------------------------------------------------------

fn realistic_session(a: &mut ClaudeSession) -> Vec<String> {
    vec![
        a.mode(),
        a.prompt("09:00"),
        a.assistant("09:01", "msg_1", 120), // streaming re-emissions of one message
        a.assistant("09:01", "msg_1", 480),
        a.assistant("09:02", "msg_1", 910),
        a.tool_result("09:02", "toolu_msg_1"),
        a.task_notification("09:03"),
        a.cost_state("0.40"),
        a.prompt("09:10"),
        a.assistant("09:11", "msg_2", 50),
        a.cost_state("0.95"),
        a.mode(),
    ]
}

#[test]
fn restart_and_replay_change_nothing() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    write_lines(&h.session_file(S_A), &realistic_session(&mut a));
    h.discover();
    let first = h.reconcile(S_A, IngestTrigger::Backfill);
    assert_eq!((first.inserted, first.ignored_total()), (10, 2));
    assert_eq!(h.provider_observations(), 10);

    // Second import after a process restart: nothing new.
    h.restart();
    h.discover();
    let second = h.reconcile(S_A, IngestTrigger::Backfill);
    assert_eq!(
        (second.mode, second.inserted, second.duplicates),
        (ReadMode::Tail, 0, 0)
    );

    // Replay from offset 0 (cursor lost entirely): everything is a duplicate.
    h.conn.execute("DELETE FROM source_cursor", []).unwrap();
    let replay = h.reconcile(S_A, IngestTrigger::Backfill);
    assert_eq!(
        (replay.mode, replay.inserted, replay.duplicates),
        (ReadMode::First, 0, 10)
    );
    assert_eq!(h.provider_observations(), 10);

    // Streamed re-emissions stay separate records with one usage identity:
    // election is the normalizer's job, not ingestion's (§8.1).
    let usage = ledger::usage_record_stats(&h.conn).unwrap();
    assert_eq!(
        (usage.usage_bearing_observations, usage.distinct_identities),
        (4, 2)
    );
}

#[test]
fn records_without_native_identity_get_occurrence_keys() {
    let mut h = Harness::new();
    let a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    // Two identical cost-state lines: distinct records, same content.
    write_lines(&file, &[a.cost_state("1.5"), a.cost_state("1.5")]);
    h.discover();
    assert_eq!(h.reconcile(S_A, IngestTrigger::Backfill).inserted, 2);
    append_lines(&file, &[a.cost_state("1.5")]);
    assert_eq!(
        h.reconcile(S_A, IngestTrigger::Hint).inserted,
        1,
        "tail occurrence continues from the ledger"
    );
    h.conn.execute("DELETE FROM source_cursor", []).unwrap();
    assert_eq!(
        h.reconcile(S_A, IngestTrigger::Backfill).inserted,
        0,
        "replay matches by occurrence"
    );
    let keys: Vec<String> = {
        let mut stmt = h
            .conn
            .prepare("SELECT record_key FROM observation WHERE channel = 'transcript' ORDER BY seq")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(keys.len(), 3);
    assert!(keys.iter().all(|k| k.starts_with("h:")));
    assert!(keys[0].ends_with("#0") && keys[1].ends_with("#1") && keys[2].ends_with("#2"));
    // Unknown time stays unknown: cost-state states no time.
    let null_times: i64 = h
        .conn
        .query_row(
            "SELECT COUNT(*) FROM observation WHERE channel = 'transcript' AND occurred_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(null_times, 3);
}

#[test]
fn same_key_with_new_content_is_kept_and_flagged() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    let p1 = a.prompt("09:00");
    let asst = a.assistant("09:01", "msg_1", 100);
    let p2 = a.prompt("09:05");
    write_lines(&file, &[p1.clone(), asst.clone(), p2.clone()]);
    h.discover();
    h.reconcile(S_A, IngestTrigger::Backfill);
    // The producer rewrites the assistant record under the same uuid with other usage.
    let rewritten = asst.replace("\"output_tokens\":100", "\"output_tokens\":999");
    atomic_replace(&file, format!("{p1}\n{rewritten}\n{p2}\n").as_bytes());
    let r = h.reconcile(S_A, IngestTrigger::Startup);
    assert_eq!((r.inserted, r.key_conflicts, r.gap.clone()), (1, 1, None));
    assert_eq!(
        h.provider_observations(),
        4,
        "both versions kept as evidence"
    );
    assert_eq!(h.collector_count("key_content_conflict"), 1);
}

#[test]
fn deleted_file_ends_the_source_with_a_gap_only_if_unread_bytes_existed() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let mut b = ClaudeSession::new(S_B);
    let (fa, fb) = (h.session_file(S_A), h.session_file(S_B));
    write_lines(&fa, &[a.prompt("09:00")]);
    append_raw(&fa, b"{\"type\":\"user\",\"partial"); // unread bytes at deletion time
    write_lines(&fb, &[b.prompt("09:01")]);
    h.discover();
    h.reconcile(S_A, IngestTrigger::Backfill);
    h.reconcile(S_B, IngestTrigger::Backfill);
    std::fs::remove_file(&fa).unwrap();
    std::fs::remove_file(&fb).unwrap();

    // Without discovery's confirmation a missing file ends nothing (§4.3).
    let missing = h.reconcile(S_A, IngestTrigger::Sweep);
    assert_eq!(
        (missing.mode, missing.ended, missing.gap),
        (ReadMode::Missing, false, None)
    );
    assert_eq!(h.collector_count("source_ended"), 0);
    assert!(h.gaps().is_empty());

    h.discover();
    let confirmed = ReconcileOptions {
        discovery_confirmed: true,
        ..ReconcileOptions::default()
    };
    let ra = h
        .try_reconcile(S_A, IngestTrigger::Sweep, &confirmed)
        .unwrap();
    assert_eq!((ra.mode, ra.ended), (ReadMode::Gone, true));
    let gap = ra.gap.expect("unread bytes were lost");
    assert_eq!(
        (gap.start, gap.reason.as_str()),
        (t("09:00"), "deleted_before_read")
    );
    let rb = h
        .try_reconcile(S_B, IngestTrigger::Sweep, &confirmed)
        .unwrap();
    assert_eq!(
        (rb.mode, rb.gap),
        (ReadMode::Gone, None),
        "fully read before deletion: nothing lost"
    );
    assert_eq!(h.collector_count("source_ended"), 2);
    assert_eq!(
        h.try_reconcile(S_A, IngestTrigger::Sweep, &confirmed)
            .unwrap()
            .mode,
        ReadMode::AlreadyEnded
    );
    assert_eq!(
        h.provider_observations(),
        2,
        "evidence of ended sources stays"
    );
}

#[test]
fn moved_file_is_missing_for_the_sweep_and_continues_at_its_new_path() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    write_lines(&file, &[a.prompt("09:00")]);
    let pending = a.assistant("09:05", "msg_1", 7);
    let (head, tail) = pending.split_at(pending.len() / 2);
    append_raw(&file, head.as_bytes());
    h.discover();
    h.reconcile(S_A, IngestTrigger::Backfill);

    // The project directory is renamed: same file, new path.
    let moved_dir = h.root().join("-Users-dev-renamed");
    std::fs::rename(file.parent().unwrap(), &moved_dir).unwrap();
    let env = IngestEnv {
        clock: &h.clock,
        file_identity: &*h.fid,
    };
    let swept = sweep(
        &mut h.conn,
        &env,
        &h.registry,
        &mut h.run,
        &ReconcileOptions::default(),
    )
    .unwrap();
    assert_eq!(swept.changed, vec![h.source(S_A).id], "not verified");
    assert_eq!(swept.reconciles[0].mode, ReadMode::Missing);
    assert_eq!(h.collector_count("source_ended"), 0);
    assert!(h.gaps().is_empty(), "a move loses nothing");

    // Discovery finds the new locator; the cursor continues on the same file.
    let moved = moved_dir.join(format!("{S_A}.jsonl"));
    append_raw(&moved, format!("{tail}\n").as_bytes());
    h.discover();
    let r = h
        .try_reconcile(
            S_A,
            IngestTrigger::Backfill,
            &ReconcileOptions {
                discovery_confirmed: true,
                ..ReconcileOptions::default()
            },
        )
        .unwrap();
    assert_eq!((r.mode, r.inserted, r.gap), (ReadMode::Tail, 1, None));
    let cursor = ledger::cursor(&h.conn, &h.source(S_A).id).unwrap().unwrap();
    assert_eq!(cursor.path.as_deref(), Some(moved.to_str().unwrap()));
    assert_eq!(h.provider_observations(), 2);
}

// ---------------------------------------------------------------------------
// Sweep, parallel streams, discovery scope
// ---------------------------------------------------------------------------

#[test]
fn sweep_reconciles_only_changed_sources_and_keeps_parallel_streams_apart() {
    let mut h = Harness::new();
    let (mut a, mut b) = (ClaudeSession::new(S_A), ClaudeSession::new(S_B));
    // Two sessions running at the same time, interleaved prompts.
    write_lines(
        &h.session_file(S_A),
        &[
            a.prompt("09:00"),
            a.assistant("09:01", "msg_a1", 10),
            a.prompt("09:02"),
        ],
    );
    write_lines(
        &h.session_file(S_B),
        &[
            b.prompt("09:00"),
            b.assistant("09:01", "msg_b1", 20),
            b.prompt("09:03"),
        ],
    );
    h.discover();
    let env = IngestEnv {
        clock: &h.clock,
        file_identity: &*h.fid,
    };
    let first = sweep(
        &mut h.conn,
        &env,
        &h.registry,
        &mut h.run,
        &ReconcileOptions::default(),
    )
    .unwrap();
    assert_eq!((first.checked, first.changed.len()), (2, 2));
    let quiet = sweep(
        &mut h.conn,
        &env,
        &h.registry,
        &mut h.run,
        &ReconcileOptions::default(),
    )
    .unwrap();
    assert_eq!((quiet.checked, quiet.changed.len()), (2, 0));
    append_lines(&h.session_file(S_A), &[a.prompt("09:04")]);
    let third = sweep(
        &mut h.conn,
        &env,
        &h.registry,
        &mut h.run,
        &ReconcileOptions::default(),
    )
    .unwrap();
    assert_eq!(third.changed, vec![h.source(S_A).id]);
    assert_eq!(third.reconciles[0].inserted, 1);
    assert_eq!(h.collector_count("sweep_completed"), 3);

    // Each stream keeps its own records; nothing merges them.
    let per_session: Vec<(String, i64)> = {
        let mut stmt = h
            .conn
            .prepare("SELECT native_session, COUNT(*) FROM observation WHERE channel = 'transcript' GROUP BY 1 ORDER BY 1")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(per_session, vec![(S_A.to_owned(), 4), (S_B.to_owned(), 3)]);
    let prompts_at_0900: i64 = h
        .conn
        .query_row(
            "SELECT COUNT(*) FROM observation o JOIN observation_facts f ON f.observation_id = o.id
             WHERE f.kind = 'user_interaction' AND o.occurred_at = ?1",
            [t("09:00").0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        prompts_at_0900, 2,
        "simultaneous prompts in two sessions are two records"
    );
}

#[test]
fn discovery_claims_workflow_agents_but_never_journals() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let project = h.root().join("-Users-dev-project");
    write_lines(&h.session_file(S_A), &[a.prompt("09:00")]);
    let mut sub = ClaudeSession::new(S_A);
    write_lines(
        &project.join(S_A).join("subagents/agent-a1.jsonl"),
        &[sub.prompt("09:01")],
    );
    let mut wf = ClaudeSession::new(S_A);
    write_lines(
        &project
            .join(S_A)
            .join("subagents/workflows/wf_x/agent-b2.jsonl"),
        &[wf.prompt("09:02")],
    );
    write_lines(
        &project
            .join(S_A)
            .join("subagents/workflows/wf_x/journal.jsonl"),
        &[r#"{"type":"result","value":"SECRET AGENT OUTPUT"}"#.to_owned()],
    );
    let env = IngestEnv {
        clock: &h.clock,
        file_identity: &*h.fid,
    };
    let root = h.root();
    let report = discover(
        &mut h.conn,
        &env,
        &h.registry,
        &mut h.run,
        &h.machine,
        &[root, h.dir.path().join("missing")],
        IngestTrigger::Backfill,
    )
    .unwrap();
    assert_eq!(
        (
            report.files_seen,
            report.files_claimed,
            report.files_unclaimed,
            report.new_sources
        ),
        (4, 3, 1, 3)
    );
    assert_eq!(report.missing_roots.len(), 1);
    let keys: Vec<String> = ledger::sources(&h.conn, Some("claude_transcript"))
        .unwrap()
        .into_iter()
        .map(|s| s.stream_key)
        .collect();
    assert!(keys.iter().any(|k| k.ends_with(":agent-a1")));
    assert!(keys.iter().any(|k| k.ends_with(":agent-b2")));

    for s in ledger::sources(&h.conn, Some("claude_transcript")).unwrap() {
        let env = IngestEnv {
            clock: &h.clock,
            file_identity: &*h.fid,
        };
        reconcile(
            &mut h.conn,
            &env,
            &h.registry,
            &mut h.run,
            &s,
            IngestTrigger::Backfill,
            &ReconcileOptions::default(),
        )
        .unwrap();
    }
    // Only the session's own prompt is a human prompt; subagent prompts are agent input.
    let kinds: Vec<(String, i64)> = {
        let mut stmt = h
            .conn
            .prepare("SELECT kind || '/' || subtype, COUNT(*) FROM observation_facts WHERE kind IN ('user_interaction','agent_input') GROUP BY 1 ORDER BY 1")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        kinds,
        vec![
            ("agent_input/subagent_stream".to_owned(), 2),
            ("user_interaction/prompt_submitted".to_owned(), 1)
        ]
    );
    let leaked: i64 = h
        .conn
        .query_row(
            "SELECT COUNT(*) FROM observation_facts WHERE facts_json LIKE '%SECRET%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(leaked, 0);
    let journal_located: i64 = h
        .conn
        .query_row(
            "SELECT COUNT(*) FROM source_locator WHERE path LIKE '%journal.jsonl'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(journal_located, 0);
}

#[test]
fn every_observation_of_a_run_carries_its_run_id() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    write_lines(&h.session_file(S_A), &[a.prompt("09:00")]);
    h.discover();
    h.reconcile(S_A, IngestTrigger::Backfill);
    let foreign: i64 = h
        .conn
        .query_row(
            "SELECT COUNT(*) FROM observation WHERE collector_run_id <> ?1",
            [&h.run.run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(foreign, 0);
    let started: String = h
        .conn
        .query_row("SELECT o.id FROM observation o JOIN observation_facts f ON f.observation_id = o.id WHERE f.kind = 'import_started'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(started, h.run.run_id);
}

#[test]
fn rewritten_last_record_is_no_proof_of_history() {
    // §4.3: history counts as preserved only if the last ingested record is
    // present with its key AND content hash. Here the producer rewrote it and
    // dropped the unread records after it: evidence may be missing.
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    let first = a.prompt("14:05");
    let last = a.assistant("14:10", "msg_1", 100);
    write_lines(&file, &[first.clone(), last.clone()]);
    h.discover();
    h.reconcile(S_A, IngestTrigger::Hint);
    append_lines(
        &file,
        &[a.prompt("14:12"), a.assistant("14:14", "msg_2", 5)],
    ); // never read
    let rewritten = last.replace("\"output_tokens\":100", "\"output_tokens\":101");
    let later = a.prompt("14:20");
    atomic_replace(&file, format!("{first}\n{rewritten}\n{later}\n").as_bytes());

    let r = h.reconcile(S_A, IngestTrigger::Startup);
    let gap = r
        .gap
        .expect("a rewritten last record does not prove history");
    assert_eq!(
        (gap.start, gap.end, gap.reason.as_str()),
        (t("14:10"), t("14:20"), "rotated_before_read")
    );
    assert_eq!(
        r.key_conflicts, 1,
        "the rewritten record is kept and flagged"
    );
    assert_eq!(h.gaps().len(), 1);
}

#[test]
fn without_an_ingested_record_a_rewrite_never_proves_history() {
    // §4.3: history is preserved only if the new content contains the last
    // ingested record. Lines of ignored types are no anchor, even when the
    // new file starts with the same bytes.
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    let mode = a.mode();
    write_lines(&file, std::slice::from_ref(&mode));
    h.discover();
    h.clock.set(t("14:05"));
    let r = h.reconcile(S_A, IngestTrigger::Hint);
    assert_eq!((r.inserted, r.ignored_total()), (0, 1));
    append_lines(&file, &[a.prompt("14:06")]); // never read
    rotate(&file, &[mode.clone(), a.prompt("14:12")]);
    h.clock.set(t("14:20"));

    let r = h.reconcile(S_A, IngestTrigger::Startup);
    assert_eq!(r.mode, ReadMode::FullReread);
    let gap = r.gap.expect("no record anchor: history is not proven");
    assert_eq!(
        (gap.start, gap.end, gap.reason.as_str()),
        (t("14:05"), t("14:12"), "rotated_before_read"),
        "from the last read to the earliest new record"
    );
    assert_eq!(r.inserted, 1);
}

#[test]
fn crash_during_a_full_reread_neither_doubles_nor_invents_a_gap() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    let lines = vec![a.cost_state("1.5"), a.cost_state("1.5"), a.prompt("09:00")];
    write_lines(&file, &lines);
    h.discover();
    assert_eq!(h.reconcile(S_A, IngestTrigger::Backfill).inserted, 3);

    // Rewritten with identical content plus one more identical cost line.
    let mut content: String = lines.iter().map(|l| format!("{l}\n")).collect();
    content.push_str(&format!("{}\n", a.cost_state("1.5")));
    atomic_replace(&file, content.as_bytes());
    let crash = ReconcileOptions {
        batch_size: 1,
        fail_after_batches: Some(1),
        ..ReconcileOptions::default()
    };
    let err = h
        .try_reconcile(S_A, IngestTrigger::Startup, &crash)
        .expect_err("simulated crash");
    assert!(matches!(err, IngestError::SimulatedCrash(1)));

    h.restart();
    let r = h.reconcile(S_A, IngestTrigger::Startup);
    assert_eq!(r.mode, ReadMode::Tail, "continues the interrupted re-read");
    assert_eq!(
        (r.inserted, r.duplicates),
        (1, 2),
        "occurrence numbering resumes: #1 is a duplicate, #2 is new"
    );
    assert_eq!(h.provider_observations(), 4);
    assert!(h.gaps().is_empty());
}

#[test]
fn sweep_finishes_an_unfinished_read_of_an_unchanged_file() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    write_lines(
        &h.session_file(S_A),
        &[a.prompt("09:00"), a.prompt("09:01"), a.prompt("09:02")],
    );
    h.discover();
    let crash = ReconcileOptions {
        batch_size: 1,
        fail_after_batches: Some(1),
        ..ReconcileOptions::default()
    };
    assert!(
        h.try_reconcile(S_A, IngestTrigger::Backfill, &crash)
            .is_err()
    );
    assert_eq!(h.provider_observations(), 1);

    // Identity, size and mtime are unchanged, but bytes remain unread.
    let env = IngestEnv {
        clock: &h.clock,
        file_identity: &*h.fid,
    };
    let opts = ReconcileOptions::default();
    let swept = sweep(&mut h.conn, &env, &h.registry, &mut h.run, &opts).unwrap();
    assert_eq!(swept.changed.len(), 1);
    assert_eq!(swept.reconciles[0].inserted, 2);
    let quiet = sweep(&mut h.conn, &env, &h.registry, &mut h.run, &opts).unwrap();
    assert!(quiet.changed.is_empty());
    assert_eq!(h.provider_observations(), 3);
}

#[cfg(unix)]
#[test]
fn an_unreadable_directory_does_not_abort_discovery() {
    use std::os::unix::fs::PermissionsExt;
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let mut b = ClaudeSession::new(S_B);
    write_lines(&h.session_file(S_A), &[a.prompt("09:00")]);
    let locked = h.root().join("-Users-dev-locked");
    write_lines(&locked.join(format!("{S_B}.jsonl")), &[b.prompt("09:00")]);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&locked).is_ok() {
        // Running with privileges that ignore permissions: nothing to test.
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let env = IngestEnv {
        clock: &h.clock,
        file_identity: &*h.fid,
    };
    let root = h.root();
    let report = discover(
        &mut h.conn,
        &env,
        &h.registry,
        &mut h.run,
        &h.machine,
        &[root],
        IngestTrigger::Backfill,
    );
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    let report = report.unwrap();
    assert_eq!((report.files_claimed, report.unreadable_dirs), (1, 1));
    assert_eq!(h.source(S_A).stream_key, format!("claude:machine-1:{S_A}"));
}

#[test]
fn in_place_rewrites_are_caught_by_the_head_fingerprint_and_the_size() {
    let mut h = Harness::new();
    let mut a = ClaudeSession::new(S_A);
    let file = h.session_file(S_A);
    let p1 = a.prompt("09:00");
    let p2 = a.prompt("09:01");
    write_lines(&file, &[p1.clone(), p2.clone()]);
    h.discover();
    h.reconcile(S_A, IngestTrigger::Backfill);
    let identity = ledger::cursor(&h.conn, &h.source(S_A).id)
        .unwrap()
        .unwrap()
        .file_identity;

    // Same file, same length, same last line: only the head fingerprint sees
    // that the first record was rewritten.
    let p1_edited = p1.replace("prompt text at 09:00", "prompt text at 08:59");
    assert_eq!(p1_edited.len(), p1.len());
    std::fs::write(&file, format!("{p1_edited}\n{p2}\n")).unwrap();
    let r = h.reconcile(S_A, IngestTrigger::Startup);
    assert_eq!(
        (r.mode, r.continuity.as_str()),
        (ReadMode::FullReread, "head_changed")
    );
    let cursor = ledger::cursor(&h.conn, &h.source(S_A).id).unwrap().unwrap();
    assert_eq!(cursor.file_identity, identity, "rewritten in place");
    assert_eq!((r.inserted, r.key_conflicts, r.gap), (1, 1, None));

    // Truncated below the cursor with the last record gone: evidence may be missing.
    std::fs::write(&file, format!("{p1_edited}\n")).unwrap();
    let r = h.reconcile(S_A, IngestTrigger::Startup);
    assert_eq!(r.continuity, "truncated");
    let gap = r.gap.expect("the last record is gone");
    assert_eq!((gap.start, gap.reason.as_str()), (t("09:01"), "truncated"));
}
