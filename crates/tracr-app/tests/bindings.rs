//! Bindings and issue suggestions (S3.2, T-0023): a binding says what a
//! session or repository is for. It changes attribution only, never the
//! counted time (TW-09); corrections still win (attribution §5); bindings
//! that disagree are conflicting; a rejected suggestion stays rejected.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use tempfile::TempDir;
use tracr_app::bindings::{BindingOp, Subject};
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
const DAY: &str = "2026-09-28";
const MIN: i64 = 60_000;

fn at(hhmm: &str) -> UtcMillis {
    UtcMillis::parse_rfc3339(&tracr_testkit::collector::at(hhmm)).unwrap()
}

fn repo(base: &Path, name: &str) -> String {
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "remote",
            "add",
            "origin",
            &format!("https://github.com/example/{name}.git"),
        ],
    ] {
        assert!(
            Command::new("git")
                .args(&args)
                .current_dir(&dir)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    dir.to_string_lossy().into_owned()
}

struct Setup {
    _dir: TempDir,
    data: PathBuf,
    c: Collector,
    clock: Arc<ManualClock>,
}

impl Setup {
    fn view(&self) -> DayAgents {
        Reader::open(&self.data).unwrap().day_agents(DAY).unwrap()
    }

    fn op(&mut self, op: BindingOp) -> anyhow::Result<String> {
        self.c.binding_op(&op)
    }
}

/// Session A in repository `example/shop` on branch `tracr-12-checkout`:
/// prompts 09:00, 09:05, 09:10, so 09:00–09:10 is inferred work.
fn setup() -> Setup {
    let dir = TempDir::new().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let data = base.join("data");
    let root = base.join("projects");
    let code = base.join("code");
    std::fs::create_dir_all(&root).unwrap();
    let clock = Arc::new(ManualClock::new(at("08:55")));
    let mut app = App::open_writer(&data, Box::new(clock.clone())).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    std::fs::create_dir_all(&code).unwrap();
    app.add_root(RootKind::Repository, &code).unwrap();
    let mut c = Collector::begin(app, "test", true).unwrap();
    let mut a = ClaudeSession::new(A);
    a.cwd = repo(&code, "shop");
    a.branch = "tracr-12-checkout".into();
    write_lines(
        &root.join("-code-shop").join(format!("{A}.jsonl")),
        &[a.prompt("09:00"), a.prompt("09:05"), a.prompt("09:10")],
    );
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
    c.probe_git().unwrap();
    c.rebuild().unwrap();
    Setup {
        _dir: dir,
        data,
        c,
        clock,
    }
}

fn work(v: &DayAgents) -> Vec<(i64, i64, String, String, Vec<String>)> {
    v.human
        .atoms
        .iter()
        .filter(|x| x.status == "work")
        .map(|x| {
            let a = x.attribution.as_ref().unwrap();
            (
                (x.start - at("09:00").0) / MIN,
                (x.end - at("09:00").0) / MIN,
                a.level.clone(),
                a.state.clone(),
                a.labels.clone(),
            )
        })
        .collect()
}

fn bind(subject: Subject, target: &str, from: &str) -> BindingOp {
    BindingOp::Bind {
        subject,
        target: target.into(),
        valid_from: Some(at(from).0),
    }
}

fn session() -> Subject {
    Subject::Session(A.into())
}

fn repository(v: &DayAgents) -> Subject {
    Subject::Repository(v.sessions[0].repository.clone().expect("repository"))
}

#[test]
fn a_branch_key_is_suggested_and_counts_only_once_accepted() {
    let mut s = setup();
    let v = s.view();
    assert_eq!(v.human.counted_ms, 10 * MIN);
    // Structural attribution: the repository.
    assert_eq!(
        work(&v),
        vec![(
            0,
            10,
            "repository".into(),
            "confirmed".into(),
            vec!["example/shop".into()]
        )]
    );
    let sess = &v.sessions[0];
    assert_eq!(sess.suggestions.len(), 1);
    assert_eq!(sess.suggestions[0].target, "other:TRACR-12");
    assert_eq!(sess.suggestions[0].evidence, "branch:tracr-12-checkout");
    assert!(sess.bound_to.is_empty());
    assert!(
        !v.human.counted_by_target.contains_key("TRACR-12"),
        "a suggestion does not count"
    );

    // Accepting binds the session from its first evidence.
    s.clock.set(at("09:30"));
    s.op(bind(session(), "other:TRACR-12", "09:00")).unwrap();
    let v = s.view();
    assert_eq!(
        v.human.counted_ms,
        10 * MIN,
        "a binding never changes counted time"
    );
    assert_eq!(
        work(&v),
        vec![(
            0,
            10,
            "binding".into(),
            "confirmed".into(),
            vec!["TRACR-12".into()]
        )]
    );
    assert_eq!(v.human.counted_by_target.get("TRACR-12"), Some(&(10 * MIN)));
    assert_eq!(v.sessions[0].bound_to, ["other:TRACR-12"]);
    assert!(
        v.sessions[0].suggestions.is_empty(),
        "bound, so no longer suggested"
    );
}

#[test]
fn a_rejected_suggestion_is_not_suggested_again() {
    let mut s = setup();
    s.clock.set(at("09:30"));
    s.op(BindingOp::Reject {
        subject: session(),
        target: "other:TRACR-12".into(),
    })
    .unwrap();
    assert!(s.view().sessions[0].suggestions.is_empty());
    // A full rebuild (reindexing) does not bring it back (AT-05).
    s.c.rebuild().unwrap();
    assert!(s.view().sessions[0].suggestions.is_empty());
    // The repository attribution is untouched.
    assert_eq!(work(&s.view())[0].2, "repository");
}

#[test]
fn a_correction_wins_over_a_binding_and_an_end_stops_it() {
    let mut s = setup();
    s.clock.set(at("09:30"));
    let id = s.op(bind(session(), "other:TRACR-12", "09:00")).unwrap();
    // The user says 09:00–09:05 was a meeting: the correction wins there.
    s.c.correct(&CorrectionRequest {
        op: "set_target".into(),
        start: Some(at("09:00").0),
        end: Some(at("09:05").0),
        target: Some("category:meeting".into()),
        revokes: None,
        ui_op: None,
        reason: None,
    })
    .unwrap();
    let v = s.view();
    assert_eq!(
        work(&v),
        vec![
            (
                0,
                5,
                "explicit".into(),
                "confirmed".into(),
                vec!["Meeting".into()]
            ),
            (
                5,
                10,
                "binding".into(),
                "confirmed".into(),
                vec!["TRACR-12".into()]
            ),
        ]
    );
    // Ending the binding at 09:05 hands the rest back to the repository.
    s.op(BindingOp::End {
        id,
        at: Some(at("09:05").0),
    })
    .unwrap();
    let v = s.view();
    assert_eq!(work(&v)[1].2, "repository");
    assert_eq!(v.human.counted_ms, 10 * MIN);
}

#[test]
fn a_session_and_a_repository_bound_to_different_targets_conflict() {
    let mut s = setup();
    s.clock.set(at("09:30"));
    let v = s.view();
    s.op(bind(session(), "other:TRACR-12", "09:00")).unwrap();
    s.op(bind(repository(&v), "category:review", "08:00"))
        .unwrap();
    let v = s.view();
    let w = work(&v);
    assert_eq!(
        (w[0].2.as_str(), w[0].3.as_str()),
        ("binding", "conflicting")
    );
    // Conflicting time counts, but for no target (attribution §3).
    assert_eq!(v.human.counted_ms, 10 * MIN);
    assert!(!v.human.counted_by_target.contains_key("TRACR-12"));
    assert!(!v.human.counted_by_target.contains_key("Review"));
    assert_eq!(
        v.human.counted_by_target.get("conflicting"),
        Some(&(10 * MIN))
    );
}

#[test]
fn a_binding_that_starts_later_leaves_earlier_time_alone() {
    let mut s = setup();
    s.clock.set(at("09:30"));
    s.op(bind(session(), "other:TRACR-12", "09:05")).unwrap();
    let w = work(&s.view());
    assert_eq!(w.len(), 2, "{w:?}");
    assert_eq!(w[0].2, "repository");
    assert_eq!((w[1].0, w[1].2.as_str()), (5, "binding"));
    // A binding in the future is refused.
    assert!(s.op(bind(session(), "other:TRACR-13", "10:00")).is_err());
}
