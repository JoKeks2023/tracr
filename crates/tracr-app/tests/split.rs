//! Einteilen: splitting a window into parts and putting time on a project, as one step.
//! The total never changes by splitting; a bad part changes nothing.

use std::path::Path;
use std::sync::Arc;

use tempfile::TempDir;
use tracr_app::correct::CorrectionRequest;
use tracr_app::projects::ProjectOp;
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

fn at(hhmm: &str) -> UtcMillis {
    UtcMillis::parse_rfc3339(&tracr_testkit::collector::at(hhmm)).unwrap()
}

/// 10 counted minutes and one 5-minute unknown window (as in the corrections test), plus a project.
fn setup(dir: &TempDir) -> (std::path::PathBuf, Collector, String) {
    let data = dir.path().join("data");
    let root = dir.path().join("projects");
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(&root).unwrap();
    let clock = Arc::new(ManualClock::new(at("08:55")));
    let mut app = App::open_writer(&data, Box::new(clock.clone())).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    let project = app
        .project_op(&ProjectOp::Create {
            name: "Shop".into(),
        })
        .unwrap();
    let mut c = Collector::begin(app, "test", true).unwrap();
    let (mut a, mut b) = (ClaudeSession::new(A), ClaudeSession::new(B));
    let transcript = |s: &str| -> std::path::PathBuf {
        Path::new(&root)
            .join("-Users-dev-project")
            .join(format!("{s}.jsonl"))
    };
    write_lines(
        &transcript(A),
        &[
            a.prompt("09:00"),
            a.assistant("09:01", "m1", 1),
            a.prompt("09:10"),
        ],
    );
    write_lines(&transcript(B), &[b.prompt("09:15")]);
    let mut t = at("08:55");
    while t <= at("09:20") {
        clock.set(t);
        c.sample(&DeviceSample {
            at: t,
            idle_ms: Some(1_000),
            locked: Some(false),
            last_sleep_wake: None,
        })
        .unwrap();
        if (t.0 / 1000) % 60 == 0 {
            c.heartbeat().unwrap();
        }
        t = UtcMillis(t.0 + 15_000);
    }
    c.reconcile_all(IngestTrigger::Startup).unwrap();
    c.rebuild().unwrap();
    (data, c, project)
}

fn req(op: &str, start: i64, end: i64, target: Option<&str>) -> CorrectionRequest {
    CorrectionRequest {
        op: op.into(),
        start: Some(start),
        end: Some(end),
        target: target.map(str::to_owned),
        revokes: None,
        ui_op: Some("split".into()),
        reason: None,
    }
}

#[test]
fn a_window_splits_into_parts_with_different_destinations_in_one_step() {
    let dir = TempDir::new().unwrap();
    let (data, mut c, project) = setup(&dir);
    let day = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    let w = day
        .human
        .atoms
        .iter()
        .find(|a| a.status == "unknown")
        .unwrap()
        .clone();
    assert_eq!((w.end - w.start) / 60_000, 5);
    let mid = w.start + 2 * 60_000;
    let key = format!("project:{project}");

    c.correct_batch(&[
        req("confirm_work", w.start, mid, None),
        req("set_target", w.start, mid, Some(&key)),
        req("mark_non_work", mid, w.end, None),
    ])
    .unwrap();

    let after = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    // 10 counted + the first 2 minutes; the last 3 are a break, none stay unclear.
    assert_eq!(
        (after.human.counted_ms, after.human.unknown_ms),
        (12 * 60_000, 0)
    );
    assert_eq!(after.human.counted_by_target.get(&key), Some(&(2 * 60_000)));
    let p = after
        .counted_by_project
        .iter()
        .find(|p| p.project_id.as_deref() == Some(project.as_str()))
        .unwrap();
    assert_eq!(
        (p.kind, p.label.as_str(), p.ms),
        ("project", "Shop", 2 * 60_000)
    );
    // The atom reports the project's name, not its id.
    let part = after
        .human
        .atoms
        .iter()
        .find(|a| a.start == w.start)
        .unwrap();
    assert_eq!(part.attribution.as_ref().unwrap().labels, ["Shop"]);
}

#[test]
fn a_bad_part_records_nothing_and_overlapping_parts_are_refused() {
    let dir = TempDir::new().unwrap();
    let (data, mut c, _) = setup(&dir);
    let before = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    let w = before
        .human
        .atoms
        .iter()
        .find(|a| a.status == "unknown")
        .unwrap()
        .clone();
    let mid = w.start + 2 * 60_000;

    // The second request names no valid target: the first must not be recorded either.
    let bad = c.correct_batch(&[
        req("confirm_work", w.start, mid, None),
        req("set_target", w.start, mid, Some("project:not a valid id")),
    ]);
    assert!(bad.is_err());
    // Two counting decisions on overlapping ranges are layers, not parts.
    let overlap = c.correct_batch(&[
        req("confirm_work", w.start, mid + 60_000, None),
        req("mark_non_work", mid, w.end, None),
    ]);
    assert!(overlap.unwrap_err().to_string().contains("overlap"));
    // A part under a minute is a slip.
    let tiny = c.correct_batch(&[
        req("confirm_work", w.start, w.start + 30_000, None),
        req("mark_non_work", w.start + 30_000, w.end, None),
    ]);
    assert!(tiny.unwrap_err().to_string().contains("minute"));

    let after = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    assert_eq!(
        (after.human.counted_ms, after.human.unknown_ms),
        (before.human.counted_ms, before.human.unknown_ms),
        "nothing was recorded"
    );
    assert!(after.corrections.is_empty());
}

#[test]
fn counted_time_can_be_moved_to_a_project_and_a_deleted_project_keeps_its_minutes() {
    let dir = TempDir::new().unwrap();
    let (data, mut c, project) = setup(&dir);
    let day = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    let work = day
        .human
        .atoms
        .iter()
        .find(|a| a.status == "work")
        .unwrap()
        .clone();
    let key = format!("project:{project}");
    c.correct(&req("set_target", work.start, work.end, Some(&key)))
        .unwrap();
    let after = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    assert_eq!(
        after.human.counted_ms, day.human.counted_ms,
        "moving does not change the total"
    );
    assert_eq!(
        after.human.counted_by_target.get(&key),
        Some(&(work.end - work.start))
    );

    // The targets offered include the project.
    let targets = Reader::open(&data).unwrap().targets().unwrap();
    assert!(
        targets
            .iter()
            .any(|t| t.group == "project" && t.key == key && t.label == "Shop")
    );

    // Deleting the project never deletes minutes: they stay under one line.
    drop(c); // the collector holds the writer lock
    let mut app = App::open_writer(&data, Box::new(tracr_app::SystemClock)).unwrap();
    app.project_op(&ProjectOp::Delete { id: project }).unwrap();
    let gone = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    assert_eq!(gone.human.counted_ms, day.human.counted_ms);
    assert!(
        gone.counted_by_project
            .iter()
            .any(|p| p.label == "Deleted project" && p.ms == work.end - work.start)
    );
}
