//! Projects and repository mappings (S3.1): a view over the counted minutes.
//! Changing a mapping moves no minute and edits no history.

use std::process::Command;
use std::sync::Arc;

use tempfile::TempDir;
use tracr_app::projects::ProjectOp;
use tracr_app::runtime::Collector;
use tracr_app::{App, Reader, RootKind, SystemClock};
use tracr_core::device::DeviceSample;
use tracr_core::evidence::IngestTrigger;
use tracr_core::time::UtcMillis;
use tracr_platform::ManualClock;
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const DAY: &str = "2026-09-28";

fn at(hhmm: &str) -> UtcMillis {
    UtcMillis::parse_rfc3339(&tracr_testkit::collector::at(hhmm)).unwrap()
}

/// One ten-minute day in the repository `example/repo`, collected with the device active.
fn setup(dir: &TempDir) -> std::path::PathBuf {
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let (data, root, code) = (base.join("data"), base.join("projects"), base.join("code"));
    let repo = code.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "remote",
            "add",
            "origin",
            "https://github.com/example/repo.git",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(&args)
                .current_dir(&repo)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let mut a = ClaudeSession::new(A);
    a.cwd = repo.to_string_lossy().into_owned();
    write_lines(
        &root.join("-code-repo").join(format!("{A}.jsonl")),
        &[
            a.prompt("09:00"),
            a.assistant("09:01", "m1", 1),
            a.prompt("09:10"),
        ],
    );
    let clock = Arc::new(ManualClock::new(at("08:55")));
    let mut app = App::open_writer(&data, Box::new(clock.clone())).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    app.add_root(RootKind::Repository, &code).unwrap();
    let mut c = Collector::begin(app, "test", true).unwrap();
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
    data
}

#[test]
fn counted_time_groups_by_project_and_nothing_moves() {
    let dir = TempDir::new().unwrap();
    let data = setup(&dir);
    let reader = Reader::open(&data).unwrap();
    let before = reader.day_agents(DAY).unwrap();
    assert_eq!(
        before.human.counted_by_target.get("example/repo"),
        Some(&(10 * 60_000))
    );
    assert!(before.counted_by_project.is_empty(), "no project yet");

    let repos = reader.known_repositories().unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(
        (repos[0].key.as_str(), repos[0].label.as_str()),
        ("github.com/example/repo", "example/repo")
    );

    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    let id = app
        .project_op(&ProjectOp::Create {
            name: "Example shop".into(),
        })
        .unwrap();
    assert!(id.starts_with("prj_"));
    assert!(
        app.project_op(&ProjectOp::Create {
            name: "example SHOP".into()
        })
        .is_err()
    );
    // Created but not mapped: the repository stays as it is, shown as "other".
    let day = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    assert_eq!(day.counted_by_project.len(), 1);
    assert_eq!(day.counted_by_project[0].kind, "other");

    app.project_op(&ProjectOp::Map {
        repository: "github.com/example/repo".into(),
        project: Some(id.clone()),
    })
    .unwrap();
    let day = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    let t = &day.counted_by_project;
    assert_eq!(
        (t[0].kind, t[0].label.as_str(), t[0].ms),
        ("project", "Example shop", 10 * 60_000)
    );
    assert_eq!(t[0].parts[0].label, "example/repo");
    // The repository line and every minute are exactly as before.
    assert_eq!(day.human.counted_by_target, before.human.counted_by_target);
    assert_eq!(day.human.counted_ms, before.human.counted_ms);

    // Renaming shows at once; deleting the project drops the mapping, not the time.
    app.project_op(&ProjectOp::Rename {
        id: id.clone(),
        name: "Shop".into(),
    })
    .unwrap();
    assert_eq!(
        Reader::open(&data)
            .unwrap()
            .day_agents(DAY)
            .unwrap()
            .counted_by_project[0]
            .label,
        "Shop"
    );
    app.project_op(&ProjectOp::Delete { id }).unwrap();
    let day = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    assert!(day.counted_by_project.is_empty());
    assert_eq!(day.human.counted_ms, before.human.counted_ms);

    // Every change is a user action in the ledger.
    let ledger = rusqlite::Connection::open(data.join("ledger.sqlite")).unwrap();
    let kinds: Vec<String> = ledger
        .prepare("SELECT kind FROM user_action WHERE kind LIKE 'setting.projects.%' ORDER BY seq")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        kinds,
        [
            "setting.projects.create",
            "setting.projects.map",
            "setting.projects.rename",
            "setting.projects.delete"
        ]
    );
}

#[test]
fn overview_groups_sessions_by_project_then_by_repository() {
    let dir = TempDir::new().unwrap();
    let data = setup(&dir);
    let reader = Reader::open(&data).unwrap();

    // No project yet: the repository stands on its own.
    let o = reader.project_overview(DAY, DAY).unwrap();
    assert_eq!(o.entries.len(), 1);
    let e = &o.entries[0];
    assert_eq!(
        (e.kind, e.label.as_str(), e.sessions),
        ("repository", "example/repo", 1)
    );
    assert_eq!(e.session_ids.len(), 1);
    assert_eq!(e.repositories[0].key, "github.com/example/repo");

    // A mapped project takes the repository; an empty project still appears.
    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    let shop = app
        .project_op(&ProjectOp::Create {
            name: "Shop".into(),
        })
        .unwrap();
    let _idle = app
        .project_op(&ProjectOp::Create {
            name: "Idle".into(),
        })
        .unwrap();
    app.project_op(&ProjectOp::Map {
        repository: "github.com/example/repo".into(),
        project: Some(shop.clone()),
    })
    .unwrap();
    let o = Reader::open(&data)
        .unwrap()
        .project_overview(DAY, DAY)
        .unwrap();
    assert_eq!(o.entries.len(), 2);
    let shop_entry = o.entries.iter().find(|e| e.label == "Shop").unwrap();
    assert_eq!((shop_entry.kind, shop_entry.sessions), ("project", 1));
    assert_eq!(shop_entry.project_id.as_deref(), Some(shop.as_str()));
    assert_eq!(shop_entry.repositories[0].label, "example/repo");
    let idle = o.entries.iter().find(|e| e.label == "Idle").unwrap();
    assert_eq!((idle.sessions, idle.agent_ms), (0, 0));
    // The same numbers as the session list.
    let list = Reader::open(&data).unwrap().sessions(DAY, DAY).unwrap();
    assert_eq!(shop_entry.cost.amount_nano, list.cost.amount_nano);
}
