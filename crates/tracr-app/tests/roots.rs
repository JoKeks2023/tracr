//! Approving and withdrawing roots (PS-02): both are user actions in the
//! ledger, and withdrawing never deletes what was already imported.

use tempfile::TempDir;
use tracr_app::{App, Reader, RootKind, RootsChange, SystemClock};
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";

#[test]
fn roots_can_be_approved_and_withdrawn_and_history_stays() {
    let dir = TempDir::new().unwrap();
    let (data, one, two) = (
        dir.path().join("data"),
        dir.path().join("one"),
        dir.path().join("two"),
    );
    std::fs::create_dir_all(&one).unwrap();
    std::fs::create_dir_all(&two).unwrap();
    let mut a = ClaudeSession::new(A);
    write_lines(
        &one.join("-Users-dev-project").join(format!("{A}.jsonl")),
        &[a.prompt("09:00"), a.assistant("09:01", "msg_1", 100)],
    );

    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    assert!(matches!(
        app.add_root(RootKind::Provider, &one).unwrap(),
        RootsChange::Added(r) if r.len() == 1
    ));
    assert!(matches!(
        app.add_root(RootKind::Provider, &two).unwrap(),
        RootsChange::Added(r) if r.len() == 2
    ));
    app.import(&mut |_, _| {}).unwrap();
    let before = Reader::open(&data).unwrap().stats().unwrap();
    assert_eq!(before.approved_provider_roots.len(), 2);

    // Withdraw one: it is no longer approved, the other stays.
    let RootsChange::Removed(left) = app.remove_root(RootKind::Provider, &one).unwrap() else {
        panic!("expected Removed");
    };
    assert_eq!(left.len(), 1);
    assert!(left[0].ends_with("two"), "{left:?}");
    assert!(matches!(
        app.remove_root(RootKind::Provider, &one).unwrap(),
        RootsChange::NotApproved(_)
    ));
    let reader = Reader::open(&data).unwrap();
    assert_eq!(reader.provider_roots().unwrap(), left);
    assert!(reader.repository_roots().unwrap().is_empty());

    // What was imported stays in the append-only ledger.
    let after = reader.stats().unwrap();
    assert_eq!(
        after.observations_by_channel,
        before.observations_by_channel
    );

    // A root whose folder is gone can still be withdrawn.
    // (the UI passes the stored, resolved path back, not the one that was typed)
    app.add_root(RootKind::Repository, &two).unwrap();
    let stored = Reader::open(&data).unwrap().repository_roots().unwrap()[0].clone();
    std::fs::remove_dir_all(&two).unwrap();
    assert!(matches!(
        app.remove_root(RootKind::Repository, std::path::Path::new(&stored)).unwrap(),
        RootsChange::Removed(r) if r.is_empty()
    ));
}

/// AD-14: the reporting zone is a user choice, validated, and fixed once a day
/// has been reviewed.
#[test]
fn the_reporting_zone_can_be_chosen_until_a_day_is_reviewed() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    let mut a = ClaudeSession::new(A);
    write_lines(
        &root.join("-Users-dev-project").join(format!("{A}.jsonl")),
        &[a.prompt("09:00"), a.assistant("09:01", "msg_1", 100)],
    );
    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    app.import(&mut |_, _| {}).unwrap();
    app.rebuild().unwrap();

    assert!(
        app.set_timezone(Some("Mars/Olympus")).is_err(),
        "unknown zone"
    );
    app.set_timezone(Some("Asia/Tokyo")).unwrap();
    app.rebuild().unwrap();
    let reader = Reader::open(&data).unwrap();
    let zone = reader.reporting_zone().unwrap();
    assert!(zone.from_setting);
    assert_eq!(zone.name, "Asia/Tokyo");
    assert_eq!(reader.days().unwrap().timezone, "Asia/Tokyo");

    // Back to the system zone.
    app.set_timezone(None).unwrap();
    assert!(
        !Reader::open(&data)
            .unwrap()
            .reporting_zone()
            .unwrap()
            .from_setting
    );

    // One reviewed day fixes the zone.
    app.rebuild().unwrap();
    let day = Reader::open(&data).unwrap().days().unwrap().days[0].clone();
    app.review_day(&day, None).unwrap();
    assert_eq!(
        Reader::open(&data).unwrap().reviewed_day_count().unwrap(),
        1
    );
    let refused = app
        .set_timezone(Some("Asia/Tokyo"))
        .unwrap_err()
        .to_string();
    assert!(refused.contains("1 reviewed day"), "{refused}");
    // The unchanged zone is not a change.
    app.set_timezone(None).unwrap();
}
