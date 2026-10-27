//! The session list over a range of days (T-0015): the same sessions and
//! numbers the day view shows, over several days at once.

use std::path::Path;

use tempfile::TempDir;
use tracr_app::{App, Reader, RootKind, SystemClock};
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const B: &str = "bbbbbbbb-2222-4222-8222-222222222222";
const DAY: &str = "2026-09-28";

fn import(data: &Path, root: &Path) {
    let mut app = App::open_writer(data, Box::new(SystemClock)).unwrap();
    app.add_root(RootKind::Provider, root).unwrap();
    app.import(&mut |_, _| {}).unwrap();
    app.rebuild().unwrap();
}

fn transcript(root: &Path, session: &str) -> std::path::PathBuf {
    root.join("-Users-dev-project")
        .join(format!("{session}.jsonl"))
}

#[test]
fn range_lists_each_session_once_and_matches_the_day_view() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    let mut a = ClaudeSession::new(A);
    write_lines(
        &transcript(&root, A),
        &[a.prompt("09:00"), a.assistant("09:01", "msg_1", 100)],
    );
    let mut b = ClaudeSession::new(B);
    write_lines(
        &transcript(&root, B),
        &[b.prompt("10:00"), b.assistant("10:01", "msg_2", 200)],
    );
    import(&data, &root);

    let reader = Reader::open(&data).unwrap();
    let day = reader.day_agents(DAY).unwrap();
    let list = reader.sessions(DAY, DAY).unwrap();
    assert_eq!(list.sessions.len(), 2);
    assert_eq!(list.sessions.len(), day.sessions.len());
    assert_eq!(list.cost.amount_nano, day.cost.identified.amount_nano);
    assert_eq!(
        list.sessions[0].native_session_id, B,
        "newest activity first"
    );
}

#[test]
fn range_outside_the_data_is_empty_and_a_reversed_range_is_refused() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    let mut a = ClaudeSession::new(A);
    write_lines(
        &transcript(&root, A),
        &[a.prompt("09:00"), a.assistant("09:01", "msg_1", 100)],
    );
    import(&data, &root);

    let reader = Reader::open(&data).unwrap();
    assert!(
        reader
            .sessions("2026-01-01", "2026-01-31")
            .unwrap()
            .sessions
            .is_empty()
    );
    assert!(reader.sessions(DAY, "2026-09-01").is_err());
    assert!(reader.sessions("nope", DAY).is_err());
}
