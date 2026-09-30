//! Collector runtime (architecture §4.5, §4.7, §4.8), driven job by job.

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tracr_app::runtime::Collector;
use tracr_app::{App, RootKind, SystemClock};
use tracr_core::device::DeviceSample;
use tracr_core::evidence::IngestTrigger;
use tracr_core::time::UtcMillis;
use tracr_store::connection::open_ledger_read_only;
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::{append_lines, write_lines};

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const B: &str = "bbbbbbbb-2222-4222-8222-222222222222";

fn transcript(root: &Path, s: &str) -> PathBuf {
    root.join("-Users-dev-project").join(format!("{s}.jsonl"))
}

fn setup() -> (TempDir, PathBuf, PathBuf, ClaudeSession) {
    let dir = TempDir::new().unwrap();
    let data = dir.path().join("data");
    let root = dir.path().join("projects");
    let mut a = ClaudeSession::new(A);
    write_lines(
        &transcript(&root, A),
        &[a.prompt("09:00"), a.assistant("09:01", "m1", 1)],
    );
    let root = {
        std::fs::create_dir_all(&root).unwrap();
        std::fs::canonicalize(&root).unwrap()
    };
    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    drop(app);
    (dir, data, root, a)
}

fn collector(data: &Path) -> Collector {
    let app = App::open_writer(data, Box::new(SystemClock)).unwrap();
    Collector::begin(app, "test", true).unwrap()
}

/// (kind, ingest_trigger) → count, over the whole ledger.
fn counts(data: &Path) -> Vec<(String, String, i64)> {
    let conn = open_ledger_read_only(&data.join("ledger.sqlite")).unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT f.kind, o.ingest_trigger, COUNT(*) FROM observation o
             JOIN observation_facts f ON f.observation_id = o.id GROUP BY 1, 2 ORDER BY 1, 2",
        )
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn count(c: &[(String, String, i64)], kind: &str, trigger: &str) -> i64 {
    c.iter()
        .find(|(k, t, _)| k == kind && t == trigger)
        .map(|x| x.2)
        .unwrap_or(0)
}

fn sample(at: i64, idle_s: i64) -> DeviceSample {
    DeviceSample {
        at: UtcMillis(at),
        idle_ms: Some(idle_s * 1000),
        locked: Some(false),
        last_sleep_wake: None,
    }
}

#[test]
fn a_run_records_coverage_lifecycle_and_device_transitions() {
    let (_d, data, _root, _a) = setup();
    let mut c = collector(&data);
    assert_eq!(c.reconcile_all(IngestTrigger::Startup).unwrap(), 2);
    let t = 1_790_000_000_000;
    assert!(!c.sample(&sample(t, 2)).unwrap());
    c.sample(&sample(t + 15_000, 10)).unwrap(); // below the collection threshold
    c.sample(&sample(t + 75_000, 70)).unwrap(); // idle since the last input
    c.heartbeat().unwrap();
    c.end("test done").unwrap();

    let got = counts(&data);
    assert_eq!(count(&got, "coverage_started", "startup"), 1);
    assert_eq!(count(&got, "collector_heartbeat", "startup"), 1);
    assert_eq!(count(&got, "coverage_ended", "startup"), 1);
    assert_eq!(
        count(&got, "user_interaction", "startup"),
        1,
        "startup reconcile is not live"
    );
    assert_eq!(count(&got, "device_active", "sample"), 1);
    assert_eq!(count(&got, "device_idle", "sample"), 1);
}

#[test]
fn hints_read_known_files_live_and_discover_new_files() {
    let (_d, data, root, mut a) = setup();
    let mut c = collector(&data);
    c.reconcile_all(IngestTrigger::Startup).unwrap();

    append_lines(&transcript(&root, A), &[a.prompt("09:10")]);
    c.hints(&[transcript(&root, A)]).unwrap();

    let mut b = ClaudeSession::new(B);
    write_lines(&transcript(&root, B), &[b.prompt("09:20")]);
    c.hints(&[transcript(&root, B)]).unwrap();
    c.end("test done").unwrap();

    let got = counts(&data);
    assert_eq!(
        count(&got, "user_interaction", "hint"),
        1,
        "appended prompt read live"
    );
    assert_eq!(
        count(&got, "user_interaction", "dir_hint"),
        1,
        "new session found by a directory hint"
    );
}

#[test]
fn only_one_runtime_can_collect() {
    let (_d, data, _root, _a) = setup();
    let c = collector(&data);
    assert!(App::open_writer(&data, Box::new(SystemClock)).is_err());
    c.end("test done").unwrap();
}

/// End to end: a collecting run with an active device, two prompts of one
/// session 10 minutes apart, then a prompt in another session 5 minutes later.
/// The same-session gap is counted (inferred); the cross-session gap is unknown.
#[test]
fn a_collected_day_counts_bridged_minutes_and_leaves_cross_session_gaps_unknown() {
    use std::sync::Arc;
    use tracr_platform::ManualClock;

    let dir = TempDir::new().unwrap();
    let data = dir.path().join("data");
    let root = dir.path().join("projects");
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(&root).unwrap();
    let at = |hhmm: &str| UtcMillis::parse_rfc3339(&tracr_testkit::collector::at(hhmm)).unwrap();
    let clock = Arc::new(ManualClock::new(at("08:55")));

    let mut app = App::open_writer(&data, Box::new(clock.clone())).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    let mut c = Collector::begin(app, "test", true).unwrap();

    let mut a = ClaudeSession::new(A);
    let mut b = ClaudeSession::new(B);
    write_lines(
        &transcript(&root, A),
        &[
            a.prompt("09:00"),
            a.assistant("09:01", "m1", 1),
            a.prompt("09:10"),
            a.assistant("09:11", "m2", 1),
        ],
    );
    write_lines(&transcript(&root, B), &[b.prompt("09:15")]);

    // Collect 08:55–09:20: a sample every 15 s (always active), a heartbeat every minute.
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
    c.end("test done").unwrap();

    let day = tracr_app::Reader::open(&data)
        .unwrap()
        .day_agents("2026-09-28")
        .unwrap();
    let h = &day.human;
    assert_eq!(
        h.counted_ms,
        10 * 60_000,
        "09:00–09:10, same session, device active"
    );
    assert_eq!(h.unknown_ms, 5 * 60_000, "09:10–09:15 crosses sessions");
    assert_eq!(
        h.unknown_by_reason.get("unresolved_window"),
        Some(&(5 * 60_000))
    );
    assert_eq!(
        h.evidence_span.map(|(s, e)| e - s),
        Some(15 * 60_000),
        "an upper bound only"
    );
    assert!(day.device.iter().any(|d| d.state == "active"));
    assert!(
        day.coverage["os_sensor"]
            .iter()
            .any(|c| c.state == "collecting")
    );
    assert!(day.pending.iter().all(|p| p.lane == "cost"));
}

/// Collected day in a real git repository: the counted interval is
/// attributed to the repository (normalized remote, no credentials), only
/// once its root is approved.
#[test]
fn counted_time_is_attributed_to_the_repository_only_under_an_approved_root() {
    use std::process::Command;
    use std::sync::Arc;
    use tracr_platform::ManualClock;

    for approved in [true, false] {
        let dir = TempDir::new().unwrap();
        let data = dir.path().join("data");
        let root = dir.path().join("projects");
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(&root).unwrap();
        let repo = std::fs::canonicalize(dir.path()).unwrap().join("code/repo");
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&repo)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
        };
        git(&["init", "-q"]);
        git(&[
            "remote",
            "add",
            "origin",
            "https://user:ghp_secret@github.com/Example/Repo.git",
        ]);

        let at =
            |hhmm: &str| UtcMillis::parse_rfc3339(&tracr_testkit::collector::at(hhmm)).unwrap();
        let clock = Arc::new(ManualClock::new(at("08:55")));
        let mut app = App::open_writer(&data, Box::new(clock.clone())).unwrap();
        app.add_root(RootKind::Provider, &root).unwrap();
        if approved {
            app.add_root(RootKind::Repository, repo.parent().unwrap())
                .unwrap();
        }
        let mut c = Collector::begin(app, "test", true).unwrap();
        let mut a = ClaudeSession::new(A);
        a.cwd = repo.to_string_lossy().into_owned();
        write_lines(
            &transcript(&root, A),
            &[
                a.prompt("09:00"),
                a.assistant("09:01", "m1", 1),
                a.prompt("09:10"),
            ],
        );
        let mut t = at("08:55");
        while t <= at("09:15") {
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
        c.end("test done").unwrap();

        let day = tracr_app::Reader::open(&data)
            .unwrap()
            .day_agents("2026-09-28")
            .unwrap();
        assert_eq!(day.human.counted_ms, 10 * 60_000);
        let work = day.human.atoms.iter().find(|x| x.status == "work").unwrap();
        let attribution = work.attribution.as_ref().unwrap();
        if approved {
            assert_eq!(
                (attribution.level.as_str(), attribution.state.as_str()),
                ("repository", "confirmed")
            );
            assert_eq!(
                attribution.targets,
                vec!["github.com/example/repo".to_owned()]
            );
            assert_eq!(
                day.human.counted_by_target.get("example/repo"),
                Some(&(10 * 60_000))
            );
            assert_eq!(
                day.sessions[0].repository_label.as_deref(),
                Some("example/repo")
            );
        } else {
            assert_eq!(
                attribution.state, "unattributed",
                "git never ran outside approved roots"
            );
            assert_eq!(day.sessions[0].repository, None);
        }
        let ledger = std::fs::read(data.join("ledger.sqlite")).unwrap();
        assert!(
            !ledger.windows(10).any(|w| w == b"ghp_secret"),
            "credentials in a remote URL never reach the ledger"
        );
    }
}
