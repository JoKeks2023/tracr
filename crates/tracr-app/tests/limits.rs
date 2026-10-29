//! Provider-reported limit figures (T-0010): Codex rollouts and the Claude
//! statusLine tap. Only the percent, window, reset and plan are ever kept.

use std::fs;

use serde_json::json;
use tempfile::TempDir;
use tracr_app::limits;

fn codex_line(ts: &str, primary: f64, secondary: f64) -> String {
    json!({
        "timestamp": ts,
        "type": "event_msg",
        "payload": {
            "type": "token_count",
            "info": null,
            "rate_limits": {
                "limit_id": "codex",
                "primary": {"used_percent": primary, "window_minutes": 300, "resets_at": 4_102_444_800i64},
                "secondary": {"used_percent": secondary, "window_minutes": 10080, "resets_at": 4_102_444_800i64},
                "plan_type": "plus",
                "credits": {"has_credits": false, "unlimited": false, "balance": "0"}
            }
        }
    })
    .to_string()
}

#[test]
fn codex_figures_come_from_the_last_record_of_the_newest_rollout() {
    let dir = TempDir::new().unwrap();
    let day = dir.path().join("2026/10/27");
    fs::create_dir_all(&day).unwrap();
    fs::write(
        day.join("rollout-2026-10-27T10-00-00-aaaa.jsonl"),
        format!(
            "{}\n{{\"type\":\"other\"}}\n{}\n",
            codex_line("2026-10-27T10:00:00.000Z", 10.0, 20.0),
            codex_line("2026-10-27T10:05:00.000Z", 40.0, 26.0)
        ),
    )
    .unwrap();
    let w = limits::codex_windows(&[dir.path().to_path_buf()]);
    assert_eq!(w.len(), 2);
    let five = w.iter().find(|w| w.window == "five_hour").unwrap();
    let week = w.iter().find(|w| w.window == "weekly").unwrap();
    assert_eq!(five.used_percent, 40.0);
    assert_eq!(week.used_percent, 26.0);
    assert_eq!(five.plan.as_deref(), Some("plus"));
    assert!(!five.expired, "the reset is far in the future");
}

#[test]
fn a_figure_whose_reset_has_passed_is_marked_expired() {
    let dir = TempDir::new().unwrap();
    let line = json!({
        "timestamp": "2020-01-01T00:00:00.000Z",
        "payload": {"rate_limits": {"primary": {"used_percent": 100.0, "window_minutes": 300, "resets_at": 1_577_851_200i64}}}
    });
    fs::write(dir.path().join("rollout-x.jsonl"), line.to_string() + "\n").unwrap();
    let w = limits::codex_windows(&[dir.path().to_path_buf()]);
    assert_eq!(w.len(), 1);
    assert!(w[0].expired);
}

#[test]
fn the_tap_keeps_only_the_numbers_and_only_when_they_change() {
    let dir = TempDir::new().unwrap();
    let payload = |five: f64| {
        json!({
            "session_id": "secret-session",
            "cwd": "/Users/someone/secret-project",
            "rate_limits": {
                "five_hour": {"used_percentage": five, "resets_at": 4_102_444_800i64},
                "seven_day": {"used_percentage": 12.5, "resets_at": 4_102_444_800i64}
            }
        })
        .to_string()
    };
    limits::tap(dir.path(), &payload(30.0), 1_000).unwrap();
    limits::tap(dir.path(), &payload(30.0), 2_000).unwrap(); // unchanged: no new line
    limits::tap(dir.path(), &payload(35.0), 3_000).unwrap();
    limits::tap(dir.path(), r#"{"cwd":"/x"}"#, 4_000).unwrap(); // no limits: nothing
    let spool = fs::read_to_string(dir.path().join("limits/claude_statusline.jsonl")).unwrap();
    assert_eq!(spool.lines().count(), 2);
    assert!(
        !spool.contains("secret"),
        "nothing but the numbers is stored"
    );
    let w = limits::claude_windows(dir.path());
    assert_eq!(
        w.iter()
            .find(|w| w.window == "five_hour")
            .unwrap()
            .used_percent,
        35.0
    );
    assert_eq!(
        w.iter()
            .find(|w| w.window == "weekly")
            .unwrap()
            .used_percent,
        12.5
    );
    assert!(limits::tap(dir.path(), "not json", 5_000).is_err());
}

#[test]
fn install_chains_the_existing_status_line_keeps_other_settings_and_uninstall_restores() {
    let dir = TempDir::new().unwrap();
    let data = dir.path().join("data");
    let settings = dir.path().join("settings.json");
    let original = r#"{
  "model": "x",
  "statusLine": {"type": "command", "command": "node /opt/clawd.js", "padding": 2},
  "permissions": {"allow": ["Bash(ls)"]}
}"#;
    fs::write(&settings, original).unwrap();
    let bin = std::path::Path::new("/Applications/tracr.app/Contents/MacOS/tracr");

    assert!(!limits::tap_state(&data, &settings).installed);
    limits::install_tap(&data, &settings, bin, 42).unwrap();
    let state = limits::tap_state(&data, &settings);
    assert!(state.installed && state.chained);
    assert_eq!(
        limits::chained_command(&data).as_deref(),
        Some("node /opt/clawd.js")
    );
    let after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(after["model"], "x");
    assert_eq!(after["permissions"]["allow"][0], "Bash(ls)");
    assert_eq!(
        after["statusLine"]["padding"], 2,
        "other statusLine keys stay"
    );
    assert!(
        after["statusLine"]["command"]
            .as_str()
            .unwrap()
            .contains("statusline-tap")
    );
    let keys: Vec<&str> = after
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        ["model", "statusLine", "permissions"],
        "key order is kept"
    );
    assert!(
        data.join("backups/claude-settings.42.json").exists(),
        "a copy was made first"
    );

    limits::install_tap(&data, &settings, bin, 43).unwrap(); // idempotent
    assert_eq!(fs::read_dir(data.join("backups")).unwrap().count(), 1);

    limits::uninstall_tap(&data, &settings).unwrap();
    let restored: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    let want: serde_json::Value = serde_json::from_str(original).unwrap();
    assert_eq!(restored, want);
    assert!(!limits::tap_state(&data, &settings).installed);
}

#[test]
fn install_without_a_settings_file_or_status_line_and_uninstall_removes_it() {
    let dir = TempDir::new().unwrap();
    let data = dir.path().join("data");
    let settings = dir.path().join(".claude/settings.json");
    limits::install_tap(&data, &settings, std::path::Path::new("/bin/tracr"), 1).unwrap();
    assert!(limits::tap_state(&data, &settings).installed);
    assert!(!limits::tap_state(&data, &settings).chained);
    limits::uninstall_tap(&data, &settings).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert!(v.get("statusLine").is_none());
}

#[test]
fn codex_credits_are_read_with_the_limits() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("rollout-c.jsonl"),
        codex_line("2026-10-27T10:05:00.000Z", 40.0, 26.0) + "\n",
    )
    .unwrap();
    let l = limits::read(
        &dir.path().join("data"),
        &[dir.path().to_path_buf()],
        &dir.path().join("none.json"),
    );
    assert_eq!(l.credits.len(), 1);
    assert!(!l.credits[0].has_credits && !l.credits[0].unlimited);
    assert_eq!(l.credits[0].balance.as_deref(), Some("0"));
}
