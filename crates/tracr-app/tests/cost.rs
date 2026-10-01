//! Estimated API cost and provider-reported figures (S1.5, architecture §8).
//!
//! Every record is estimated on its own. Unpriced is never zero, a partial
//! estimate says what it leaves out, and the provider's cumulative figure is
//! kept apart and never summed.

use std::path::Path;

use tempfile::TempDir;
use tracr_app::{App, Reader, RootKind, SystemClock};
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const DAY: &str = "2026-09-28";

fn with_usage(line: String, f: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut v: serde_json::Value = serde_json::from_str(&line).unwrap();
    f(&mut v["message"]);
    v.to_string()
}

fn setup(data: &Path, root: &Path) {
    let mut a = ClaudeSession::new(A);
    let lines = vec![
        a.prompt("09:00"),
        // Cache writes with their TTL tier: every category is priced.
        with_usage(a.assistant("09:01", "msg_1", 100), |m| {
            m["usage"]["cache_creation"] = serde_json::json!({
                "ephemeral_5m_input_tokens": 100, "ephemeral_1h_input_tokens": 0
            });
        }),
        a.cost_state("0.40"),
        // Cache writes without a tier: those 100 tokens stay unpriced.
        a.assistant("09:05", "msg_2", 200),
        // A model without a price rule: unpriced, not zero.
        with_usage(a.assistant("09:10", "msg_3", 300), |m| {
            m["model"] = serde_json::json!("claude-opus-5-5-preview");
        }),
        a.cost_state("0.95"),
    ];
    write_lines(
        &root.join("-Users-dev-project").join(format!("{A}.jsonl")),
        &lines,
    );
    let mut app = App::open_writer(data, Box::new(SystemClock)).unwrap();
    app.add_root(RootKind::Provider, root).unwrap();
    app.import(&mut |_, _| {}).unwrap();
    app.rebuild().unwrap();
}

#[test]
fn records_are_estimated_one_by_one_and_unpriced_is_never_zero() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    setup(&data, &root);

    let day = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    let s = &day.sessions[0];
    let line = |model: &str| {
        s.usage
            .iter()
            .find(|u| u.model.as_deref() == Some(model))
            .unwrap_or_else(|| panic!("{model} line"))
    };

    // claude-opus-5-5: $4 input, $5 5m write, $0.20 read, $20 output per MTok.
    // msg_1: 10×4000 + 100×5000 + 1000×200 + 100×20000 = 2,740,000 nano
    // msg_2: 10×4000 + 1000×200 + 200×20000 = 4,240,000 nano (+100 unpriced)
    let opus = &line("claude-opus-5-5").cost;
    assert_eq!(opus.amount_nano, Some(6_980_000));
    assert_eq!(opus.priced_records, 1);
    assert_eq!(opus.partially_priced_records, 1);
    assert_eq!(opus.unpriced_tokens, 100);
    assert!(
        opus.reasons
            .contains(&"cache_write_tier_not_reported".to_owned())
    );
    // Speed and geography are not retained yet: stated, not hidden.
    assert!(
        opus.assumptions
            .contains(&"speed_not_recorded:standard".to_owned())
    );

    let unknown = &line("claude-opus-5-5-preview").cost;
    assert_eq!(unknown.amount_nano, None, "unpriced, not zero");
    assert_eq!(unknown.status(), Some("unpriced"));
    assert_eq!(unknown.unpriced_tokens, 10 + 100 + 1000 + 300);

    // The session and the day add estimates of one kind and one revision.
    assert_eq!(s.cost.amount_nano, Some(6_980_000));
    assert_eq!(s.cost.status(), Some("partially_priced"));
    assert_eq!(s.cost.unpriced_tokens, 100 + 1410);
    assert_eq!(day.cost.identified.amount_nano, Some(6_980_000));
    assert_eq!(day.cost.spanning_records, 0);
    assert!(day.cost.pricing.revision.starts_with("2026-10-01.1+"));
    assert_eq!(day.cost.pricing.label, "API-equivalent estimate, not spend");
    assert!(day.pending.is_empty(), "the cost lane is derived now");

    // The provider's figure: the latest only, never summed with 0.40.
    assert_eq!(s.reported.len(), 1);
    assert_eq!(s.reported[0].value_as_stated, "0.95");
    assert_eq!(s.reported[0].value_nano, Some(950_000_000));
}

#[test]
fn the_ledger_keeps_the_catalog_and_csv_carries_the_estimate() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    setup(&data, &root);

    let reader = Reader::open(&data).unwrap();
    let day = reader.day_agents(DAY).unwrap();
    let ledger = rusqlite::Connection::open(data.join("ledger.sqlite")).unwrap();
    let stored: String = ledger
        .query_row(
            "SELECT content FROM pricing_catalog WHERE revision = ?1",
            [&day.cost.pricing.revision],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, tracr_app::pricing::bundled_text());

    let csv = tracr_app::export::agent_sessions_csv(&reader, DAY, DAY).unwrap();
    let mut lines = csv.lines();
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    let row: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |name: &str| row[header.iter().position(|h| *h == name).unwrap()];
    assert_eq!(col("estimated_api_cost"), "0.00698");
    assert_eq!(col("estimated_api_cost_currency"), "USD");
    assert_eq!(col("estimate_status"), "partially_priced");
    assert_eq!(col("unpriced_tokens"), "1510");
    assert_eq!(col("provider_reported_cost"), "0.95");
}

#[test]
fn an_incremental_update_keeps_the_estimates() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    setup(&data, &root);
    let before = Reader::open(&data).unwrap().day_agents(DAY).unwrap();

    let mut a = ClaudeSession::new(A);
    a.uuid_prefix = "cccccccc".into(); // a new record, not a repeat of the first
    let path = root.join("-Users-dev-project").join(format!("{A}.jsonl"));
    let mut content = std::fs::read_to_string(&path).unwrap();
    content.push_str(&format!("{}\n", a.assistant("09:20", "msg_4", 50)));
    std::fs::write(&path, content).unwrap();
    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    app.import(&mut |_, _| {}).unwrap();
    app.update_derived().unwrap();

    let after = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    // msg_4: 10×4000 + 1000×200 + 50×20000 = 1,240,000 nano
    assert_eq!(
        after.sessions[0].cost.amount_nano,
        before.sessions[0].cost.amount_nano.map(|n| n + 1_240_000)
    );
}

/// INV-7: no API sums different money kinds or successive reported
/// snapshots. The day DTO has no total across kinds, and the reported figure
/// is the latest one, never added to anything.
#[test]
fn inv7_no_total_across_money_kinds() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    setup(&data, &root);
    let day = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    let json = serde_json::to_value(&day).unwrap();

    fn keys(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, x) in m {
                    out.push(k.clone());
                    keys(x, out);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| keys(x, out)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    keys(&json, &mut all);
    for k in &all {
        let k = k.to_lowercase();
        assert!(
            !(k.contains("total")
                && (k.contains("cost") || k.contains("money") || k.contains("spend"))),
            "a key that totals money: {k}"
        );
    }
    // The estimate and the reported figure live in separate places.
    assert!(json["cost"].get("reported").is_none());
    assert_eq!(
        json["sessions"][0]["reported"][0]["value_nano"],
        950_000_000
    );
    assert_eq!(json["sessions"][0]["cost"]["amount_nano"], 6_980_000);
}

/// `tracr day`: the estimate and the provider's figure are separate lines,
/// partial estimates are floors, and the footer names the catalog.
#[test]
fn day_report_keeps_money_kinds_apart() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    setup(&data, &root);
    let reader = Reader::open(&data).unwrap();
    let day = reader.day_agents(DAY).unwrap();
    let text = tracr_app::report::day_text(&day, &reader.ledger_counts(&day).unwrap());
    assert!(text.contains("API-equivalent estimate, not spend · catalog 2026-10-01.1"));
    assert!(text.contains(">= 0.01 USD"), "{text}");
    assert!(text.contains("0.95 USD (totalCostUSD"), "{text}");
    assert!(
        !text.contains("0.40 USD"),
        "only the latest reported figure"
    );
    assert!(text.contains("cost kinds: estimated API cost, provider-reported"));
}
