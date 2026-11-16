//! Walkthrough H (docs/walkthroughs.md): a reviewed day whose derivation
//! changes later, with the user's corrections in place.
//!
//! Must hold: the reviewed revision r1 never changes; the user's records
//! (k1–k5) apply unchanged to their stated ranges after re-derivation, so
//! the resolved day equals r1. H2: evidence that contradicts a user
//! assertion changes nothing on that axis. k6 (a rejected issue suggestion)
//! needs issue suggestions (TRACR-40) and is not covered here.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tempfile::TempDir;
use tracr_app::correct::CorrectionRequest;
use tracr_app::day::DayAgents;
use tracr_app::runtime::Collector;
use tracr_app::{App, Reader, RootKind};
use tracr_core::device::DeviceSample;
use tracr_core::evidence::IngestTrigger;
use tracr_core::time::UtcMillis;
use tracr_platform::ManualClock;
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const B: &str = "bbbbbbbb-2222-4222-8222-222222222222";
const DAY: &str = "2026-09-28";
const MIN: i64 = 60_000;
const TA: &str = "other:Project A";
const TB: &str = "other:Project B";

fn at(hhmm: &str) -> UtcMillis {
    UtcMillis::parse_rfc3339(&tracr_testkit::collector::at(hhmm)).unwrap()
}

fn transcript(root: &Path, s: &str) -> PathBuf {
    root.join("-Users-dev-project").join(format!("{s}.jsonl"))
}

fn req(op: &str, start: i64, end: i64, target: Option<&str>) -> CorrectionRequest {
    CorrectionRequest {
        op: op.into(),
        start: Some(start),
        end: Some(end),
        target: target.map(str::to_owned),
        revokes: None,
        ui_op: Some("walkthrough_h".into()),
        reason: None,
    }
}

struct Day {
    data: PathBuf,
    root: PathBuf,
    collector: Collector,
    a: ClaudeSession,
    b: ClaudeSession,
    a_lines: Vec<String>,
    b_lines: Vec<String>,
}

impl Day {
    fn view(&self) -> DayAgents {
        Reader::open(&self.data).unwrap().day_agents(DAY).unwrap()
    }

    /// Lines that live collection missed, read later by a sweep (a backfill).
    fn backfill(&mut self, session: char, line: impl FnOnce(&mut ClaudeSession) -> String) {
        let (s, lines, id) = match session {
            'a' => (&mut self.a, &mut self.a_lines, A),
            _ => (&mut self.b, &mut self.b_lines, B),
        };
        lines.push(line(s));
        lines.sort_by_key(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["timestamp"]
                .as_str()
                .unwrap()
                .to_owned()
        });
        write_lines(&transcript(&self.root, id), lines);
        self.collector.reconcile_all(IngestTrigger::Sweep).unwrap();
        self.collector.rebuild().unwrap();
    }
}

/// A 09:00 and 09:10, B 09:15, A 09:35: session switches leave 09:10–09:35
/// undecided.
fn setup(dir: &TempDir) -> Day {
    let data = dir.path().join("data");
    let root = dir.path().join("projects");
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(&root).unwrap();
    let clock = Arc::new(ManualClock::new(at("08:55")));
    let mut app = App::open_writer(&data, Box::new(clock.clone())).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    let mut collector = Collector::begin(app, "test", true).unwrap();
    let (mut a, mut b) = (ClaudeSession::new(A), ClaudeSession::new(B));
    let a_lines = vec![
        a.prompt("09:00"),
        a.assistant("09:01", "m1", 1),
        a.prompt("09:10"),
        a.prompt("09:35"),
    ];
    let b_lines = vec![b.prompt("09:15")];
    write_lines(&transcript(&root, A), &a_lines);
    write_lines(&transcript(&root, B), &b_lines);
    let mut t = at("08:55");
    while t <= at("09:40") {
        clock.set(t);
        collector
            .sample(&DeviceSample {
                at: t,
                idle_ms: Some(1_000),
                locked: Some(false),
                last_sleep_wake: None,
            })
            .unwrap();
        if (t.0 / 1000) % 60 == 0 {
            collector.heartbeat().unwrap();
        }
        t = UtcMillis(t.0 + 15_000);
    }
    collector.reconcile_all(IngestTrigger::Startup).unwrap();
    collector.rebuild().unwrap();
    Day {
        data,
        root,
        collector,
        a,
        b,
        a_lines,
        b_lines,
    }
}

/// k1–k5 as in the walkthrough, then review r1.
fn corrected_and_reviewed(d: &mut Day) -> (i64, i64) {
    let v = d.view();
    let unknown: Vec<_> = v
        .human
        .atoms
        .iter()
        .filter(|x| x.status == "unknown")
        .map(|x| (x.start, x.end))
        .collect();
    assert_eq!(
        unknown,
        vec![(at("09:10").0, at("09:35").0)],
        "one window tracr cannot decide"
    );
    // The user splits it with explicit ranges: A until 09:15, then B.
    let (w1, w2) = (
        (at("09:10").0, at("09:15").0),
        (at("09:15").0, at("09:35").0),
    );
    let c = &mut d.collector;
    c.correct(&req("confirm_work", w1.0, w1.1, None)).unwrap(); // k1
    c.correct(&req("set_target", w1.0, w1.1, Some(TA))).unwrap(); // k2
    c.correct(&req("confirm_work", w2.0, w2.1, None)).unwrap(); // k3
    c.correct(&req("set_target", w2.0, w2.1, Some(TB))).unwrap(); // k4
    // k5: the user stopped working on A at 09:08.
    c.correct(&req("mark_non_work", at("09:08").0, at("09:10").0, None))
        .unwrap();
    // The rest of A's inferred work goes to A.
    c.correct(&req("set_target", at("09:00").0, at("09:08").0, Some(TA)))
        .unwrap();
    c.review(DAY, None).unwrap();
    w2
}

/// What the derivation underneath says at `hhmm`, before the user's records.
fn candidate(v: &DayAgents, hhmm: &str) -> String {
    let t = at(hhmm).0;
    v.human
        .atoms
        .iter()
        .find(|x| x.start <= t && t < x.end)
        .and_then(|x| x.candidate_status.clone())
        .unwrap_or_default()
}

fn totals(v: &DayAgents) -> (i64, i64, i64, Option<i64>, Option<i64>) {
    let h = &v.human;
    (
        h.counted_ms / MIN,
        h.unknown_ms / MIN,
        h.atoms
            .iter()
            .filter(|x| x.status == "non_work")
            .map(|x| x.end - x.start)
            .sum::<i64>()
            / MIN,
        h.counted_by_target.get("Project A").map(|ms| ms / MIN),
        h.counted_by_target.get("Project B").map(|ms| ms / MIN),
    )
}

#[test]
fn h_changed_derivation_keeps_the_review_and_the_corrections() {
    let dir = TempDir::new().unwrap();
    let mut d = setup(&dir);
    corrected_and_reviewed(&mut d);

    let v = d.view();
    let r1 = v.review.clone().expect("r1");
    assert_eq!(r1.rev, 1);
    // A: 09:00–09:08 inferred + 09:10–09:15 asserted = 13; B: 09:15–09:35 = 20;
    // 09:08–09:10 trimmed by the user (k5).
    assert_eq!(totals(&v), (33, 0, 2, Some(13), Some(20)));
    assert_eq!(
        (r1.snapshot.counted_ms / MIN, r1.snapshot.unknown_ms / MIN),
        (33, 0)
    );
    assert!(v.discrepancy.is_none());
    assert_eq!(candidate(&v, "09:20"), "unknown");

    // The change: a backfill reveals a B engagement at 09:25 that live
    // collection missed, inside the range the user confirmed as B (k3, k4).
    d.backfill('b', |b| b.prompt("09:25"));

    let v = d.view();
    // The derivation underneath did change: 09:15–09:25 is now inferred B.
    assert_eq!(candidate(&v, "09:20"), "work");
    let after = v.review.clone().expect("r1 stays");
    assert_eq!(after.revision_id, r1.revision_id, "no new revision");
    assert_eq!(
        serde_json::to_string(&after.snapshot).unwrap(),
        serde_json::to_string(&r1.snapshot).unwrap(),
        "r1 is stored by value and never changes"
    );
    // k1–k5 apply unchanged to their stated ranges: the resolved day equals r1.
    assert_eq!(totals(&v), (33, 0, 2, Some(13), Some(20)));
    // The new engagement lies under the user's assertion: the time stays
    // user-asserted, not re-inferred.
    let x = v
        .human
        .atoms
        .iter()
        .find(|x| x.start <= at("09:25").0 && at("09:25").0 < x.end)
        .unwrap();
    assert_eq!(
        (x.status.as_str(), x.duration_basis.as_str()),
        ("work", "user_asserted")
    );
}

#[test]
fn h2_contradicting_evidence_does_not_override_the_user() {
    let dir = TempDir::new().unwrap();
    let mut d = setup(&dir);
    let w2 = corrected_and_reviewed(&mut d);
    let r1 = d.view().review.expect("r1");

    // The backfill instead reveals an A engagement at 09:25, inside the
    // range the user confirmed as work for B.
    d.backfill('a', |a| a.prompt("09:25"));

    let v = d.view();
    // Underneath, 09:25–09:35 is now inferred work, engaged by A.
    assert_eq!(candidate(&v, "09:30"), "work");
    assert_eq!(v.review.as_ref().unwrap().revision_id, r1.revision_id);
    // The assertion wins on its axis: counted, and still B.
    assert_eq!(totals(&v), (33, 0, 2, Some(13), Some(20)));
    let in_w2: Vec<_> = v
        .human
        .atoms
        .iter()
        .filter(|x| x.start >= w2.0 && x.end <= w2.1)
        .collect();
    assert!(!in_w2.is_empty());
    for x in in_w2 {
        assert_eq!(x.status, "work");
        let labels = &x.attribution.as_ref().expect("attributed").labels;
        assert_eq!(labels, &["Project B"], "{x:?}");
    }
}
