//! Analytics over a range: cost per day and model, cache health, cold requests, what-if.

use std::path::Path;

use tempfile::TempDir;
use tracr_app::{App, Reader, RootKind, SystemClock};
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const DAY: &str = "2026-09-28";

/// An assistant record whose usage is replaced: input, cache write (5 min tier), cache read, output.
fn usage(line: &str, input: u64, write: u64, read: u64, output: u64) -> String {
    let mut v: serde_json::Value = serde_json::from_str(line).unwrap();
    v["message"]["usage"] = serde_json::json!({
        "input_tokens": input,
        "cache_creation_input_tokens": write,
        "cache_read_input_tokens": read,
        "output_tokens": output,
        "service_tier": "standard",
        "cache_creation": {"ephemeral_5m_input_tokens": write, "ephemeral_1h_input_tokens": 0}
    });
    v.to_string()
}

fn import(dir: &TempDir, lines: Vec<String>) -> std::path::PathBuf {
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    write_lines(
        &Path::new(&root)
            .join("-Users-dev-project")
            .join(format!("{A}.jsonl")),
        &lines,
    );
    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    app.import(&mut |_, _| {}).unwrap();
    app.rebuild().unwrap();
    data
}

#[test]
fn cost_per_day_and_model_and_the_cache_numbers_add_up() {
    let dir = TempDir::new().unwrap();
    let mut a = ClaudeSession::new(A);
    let lines = vec![
        a.prompt("09:00"),
        usage(&a.assistant("09:01", "m1", 100), 10, 100, 1000, 100),
        usage(&a.assistant("09:02", "m2", 100), 10, 100, 1000, 100),
    ];
    let data = import(&dir, lines);
    let an = Reader::open(&data)
        .unwrap()
        .analytics("2026-09-27", "2026-09-29")
        .unwrap();
    assert_eq!(
        an.days.len(),
        3,
        "every day of the range is there, even without usage"
    );
    let d = an.days.iter().find(|d| d.day == DAY).unwrap();
    assert_eq!(d.models.len(), 1);
    assert_eq!(d.models[0].model, "claude-opus-5-5");
    assert_eq!(d.models[0].records, 2);
    assert!(d.models[0].amount_nano.is_some());
    assert!(
        an.days
            .iter()
            .filter(|d| d.day != DAY)
            .all(|d| d.models.is_empty())
    );
    assert_eq!(an.models[0].tokens, 2 * (10 + 100 + 1000 + 100));
    // 2000 read of 2220 input tokens.
    assert_eq!(an.cache.read_tokens, 2000);
    assert_eq!(an.cache.write_5m_tokens, 200);
    let hit = an.cache.hit_ratio.unwrap();
    assert!((hit - 2000.0 / 2220.0).abs() < 1e-9, "{hit}");
    assert!(
        an.cache.saved_nano > 0,
        "reading from the cache is cheaper than reading fresh"
    );
    assert_eq!(an.cache.cold_requests, 0);
    // The what-if reprices Opus on Sonnet 5.5: cheaper per the catalog.
    let w = an.what_if.unwrap();
    assert_eq!(w.to_model, "claude-sonnet-5-5");
    assert!(w.saving_nano > 0 && w.repriced_nano < w.current_nano);
}

#[test]
fn a_request_that_rewrites_the_context_after_the_cache_expired_is_cold() {
    let dir = TempDir::new().unwrap();
    let mut a = ClaudeSession::new(A);
    let lines = vec![
        a.prompt("09:00"),
        usage(&a.assistant("09:01", "m1", 100), 10, 100, 9000, 100),
        a.prompt("09:20"),
        // 19 minutes later: the whole context is written again, nothing is read.
        usage(&a.assistant("09:21", "m2", 100), 10, 9000, 0, 100),
    ];
    let data = import(&dir, lines);
    let an = Reader::open(&data).unwrap().analytics(DAY, DAY).unwrap();
    assert_eq!(an.cache.cold_requests, 1);
    assert_eq!(an.cache.cold_tokens, 9000);
    assert!(an.cache.cold_extra_nano > 0);
}

#[test]
fn a_reversed_or_malformed_range_is_refused() {
    let dir = TempDir::new().unwrap();
    let mut a = ClaudeSession::new(A);
    let data = import(&dir, vec![a.prompt("09:00")]);
    let r = Reader::open(&data).unwrap();
    assert!(r.analytics(DAY, "2026-09-01").is_err());
    assert!(r.analytics("nope", DAY).is_err());
}
