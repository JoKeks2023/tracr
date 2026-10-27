//! The fleet: the registry of running sessions and the per-session figures.

use std::fs;
use std::path::Path;

use serde_json::json;
use tempfile::TempDir;
use tracr_app::fleet;
use tracr_app::{App, Reader, RootKind, SystemClock};
use tracr_core::time::UtcMillis;
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";

#[test]
fn the_registry_is_read_without_opening_key_files_and_the_alive_check_works() {
    let dir = TempDir::new().unwrap();
    let sessions = dir.path().join("sessions");
    fs::create_dir_all(&sessions).unwrap();
    let me = i64::from(std::process::id());
    fs::write(
        sessions.join(format!("{me}.json")),
        json!({"pid": me, "sessionId": "s-live", "cwd": "/x/harbor", "startedAt": 10, "status": "busy",
               "statusUpdatedAt": 30, "name": "Billing", "version": "2.1.310", "kind": "interactive",
               "messagingSocketPath": "/tmp/x.sock"}).to_string(),
    ).unwrap();
    fs::write(
        sessions.join("999999.json"),
        json!({"pid": 999_999, "sessionId": "s-dead", "status": "idle", "statusUpdatedAt": 20})
            .to_string(),
    )
    .unwrap();
    // A key file must never be read: it would fail the JSON parse and hold a secret.
    fs::write(
        sessions.join(format!("{me}.deadbeef.key")),
        "SECRET-KEY-MATERIAL",
    )
    .unwrap();
    fs::write(sessions.join("broken.json"), "not json").unwrap();

    let entries = fleet::registry(&[dir.path().to_path_buf()]);
    assert_eq!(
        entries.len(),
        2,
        "the key file and the broken file are skipped"
    );
    let live = entries
        .iter()
        .find(|e| e.native_session_id == "s-live")
        .unwrap();
    assert!(live.alive);
    assert_eq!(
        (
            live.status.as_deref(),
            live.name.as_deref(),
            live.cwd.as_deref()
        ),
        (Some("busy"), Some("Billing"), Some("/x/harbor"))
    );
    let dead = entries
        .iter()
        .find(|e| e.native_session_id == "s-dead")
        .unwrap();
    assert!(!dead.alive);
    assert_eq!(
        entries[0].native_session_id, "s-live",
        "newest state change first"
    );
    assert!(fleet::registry(&[dir.path().join("missing")]).is_empty());
}

#[test]
fn stats_give_spend_of_the_last_hour_the_last_request_and_the_cache_life() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    let mut a = ClaudeSession::new(A);
    write_lines(
        &Path::new(&root)
            .join("-Users-dev-project")
            .join(format!("{A}.jsonl")),
        &[
            a.prompt("09:00"),
            a.assistant("09:01", "m1", 100),
            a.assistant("09:20", "m2", 100),
        ],
    );
    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    app.import(&mut |_, _| {}).unwrap();
    app.rebuild().unwrap();
    drop(app);

    let at = |hhmm: &str| {
        UtcMillis::parse_rfc3339(&tracr_testkit::collector::at(hhmm))
            .unwrap()
            .0
    };
    let r = Reader::open(&data).unwrap();
    let f = r.fleet(at("09:22")).unwrap();
    assert!(!f.registry_available, "no Claude home approved");
    assert_eq!(f.stats.len(), 1);
    let s = &f.stats[0];
    assert_eq!(s.native_session_id, A);
    assert_eq!(s.last_request_at, at("09:20"));
    // Input of that request: 10 fresh + 100 written + 1000 read.
    assert_eq!(s.context_tokens, Some(1110));
    assert!((s.cache_hit.unwrap() - 1000.0 / 1110.0).abs() < 1e-9);
    assert_eq!(
        s.cache_ttl_left_ms,
        3 * 60_000,
        "5 minutes from 09:20, seen at 09:22"
    );
    assert!(s.cost_last_hour_nano.is_some());
    // Later the cache has expired and, after an hour, nothing is spent.
    let later = r.fleet(at("10:30")).unwrap();
    assert_eq!(later.stats.len(), 1, "still within the last 24 hours");
    assert!(
        later.stats[0].cache_ttl_left_ms < 0,
        "the cache has expired"
    );
    assert_eq!(
        later.stats[0].cost_last_hour_nano, None,
        "nothing spent in the last hour"
    );
}
