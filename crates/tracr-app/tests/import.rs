//! Use-case level checks: single-writer lock (AD-03), approved roots in
//! `setting_rev` (PS-02), and idempotent import.

use tempfile::TempDir;
use tracr_app::{App, Reader, RootKind, RootsChange, SystemClock};
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const S: &str = "cccccccc-3333-4333-8333-333333333333";

fn setup() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let data = dir.path().join("data");
    let root = dir.path().join("projects");
    let mut s = ClaudeSession::new(S);
    write_lines(
        &root.join("-Users-dev-p").join(format!("{S}.jsonl")),
        &[
            s.prompt("09:00"),
            s.assistant("09:01", "msg_1", 10),
            s.mode(),
        ],
    );
    (dir, data, root)
}

#[test]
fn only_one_writer_at_a_time() {
    let (_d, data, _) = setup();
    let first = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    let second = App::open_writer(&data, Box::new(SystemClock));
    assert!(
        second
            .err()
            .unwrap()
            .to_string()
            .contains("another tracr process")
    );
    drop(first);
    App::open_writer(&data, Box::new(SystemClock)).unwrap();
}

#[test]
fn import_needs_an_approved_root_and_is_idempotent() {
    let (_d, data, root) = setup();
    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    let err = app.import(&mut |_, _| {}).unwrap_err();
    assert!(err.to_string().contains("no approved provider roots"));

    assert!(matches!(
        app.add_root(RootKind::Provider, &root).unwrap(),
        RootsChange::Added(_)
    ));
    assert!(matches!(
        app.add_root(RootKind::Provider, &root).unwrap(),
        RootsChange::AlreadyApproved(_)
    ));
    assert!(
        app.add_root(RootKind::Repository, &root.join("nope"))
            .is_err()
    );

    let first = app.import(&mut |_, _| {}).unwrap();
    assert_eq!(
        (
            first.new_sources,
            first.inserted,
            first.provider_observations_after
        ),
        (1, 2, 2)
    );
    assert_eq!(first.ignored_by_type.get("mode"), Some(&1));
    let second = app.import(&mut |_, _| {}).unwrap();
    assert_eq!(
        second.provider_observations_after - second.provider_observations_before,
        0
    );
    assert_eq!((second.new_sources, second.inserted), (0, 0));
    drop(app);

    let stats = Reader::open(&data).unwrap().stats().unwrap();
    assert_eq!(stats.approved_provider_roots.len(), 1);
    assert!(
        stats
            .transcript_kinds
            .iter()
            .any(|(k, n)| k == "user_interaction/prompt_submitted" && *n == 1)
    );
    let settings: i64 = rusqlite::Connection::open(data.join("ledger.sqlite"))
        .unwrap()
        .query_row("SELECT COUNT(*) FROM setting_rev", [], |r| r.get(0))
        .unwrap();
    assert_eq!(settings, 1, "the duplicate approval wrote no revision");
}

#[test]
fn a_missing_root_never_ends_its_sources() {
    let (d, data, root) = setup();
    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    assert_eq!(app.import(&mut |_, _| {}).unwrap().inserted, 2);

    // The root disappears (an unmounted volume): the source is missing, not ended.
    let away = d.path().join("projects-away");
    std::fs::rename(&root, &away).unwrap();
    let r = app.import(&mut |_, _| {}).unwrap();
    assert_eq!(r.missing_roots.len(), 1);
    assert_eq!((r.ended, r.missing, r.gaps.len()), (0, 1, 0));

    // Back again: reading continues, nothing was lost or doubled.
    std::fs::rename(&away, &root).unwrap();
    let r = app.import(&mut |_, _| {}).unwrap();
    assert_eq!((r.ended, r.missing, r.inserted, r.gaps.len()), (0, 0, 0, 0));

    // A file deleted under a fully walked root ends its source.
    std::fs::remove_file(root.join("-Users-dev-p").join(format!("{S}.jsonl"))).unwrap();
    let r = app.import(&mut |_, _| {}).unwrap();
    assert_eq!((r.ended, r.missing), (1, 0));
}
