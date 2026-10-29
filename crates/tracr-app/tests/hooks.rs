//! Claude Code hooks: what is kept (four fields), what each event means, and the opt-in install.

use std::fs;

use serde_json::json;
use tempfile::TempDir;
use tracr_app::hooks::{self, LiveState};

fn payload(event: &str, session: &str) -> String {
    json!({
        "hook_event_name": event,
        "session_id": session,
        "cwd": "/Users/someone/secret-project",
        "transcript_path": "/Users/someone/.claude/projects/x/y.jsonl",
        "prompt": "secret prompt text",
        "message": "Claude needs your permission to use Bash",
        "notification_type": if event == "Notification" { "permission_prompt" } else { "" }
    })
    .to_string()
}

#[test]
fn only_event_session_kind_and_time_are_kept() {
    let dir = TempDir::new().unwrap();
    hooks::record(dir.path(), &payload("Notification", "s1"), 1_000).unwrap();
    hooks::record(dir.path(), &payload("Stop", "s1"), 2_000).unwrap();
    hooks::record(
        dir.path(),
        &json!({"hook_event_name": "PreToolUse", "session_id": "s1"}).to_string(),
        3_000,
    )
    .unwrap();
    hooks::record(
        dir.path(),
        &json!({"hook_event_name": "Stop", "session_id": "bad id with spaces"}).to_string(),
        3_000,
    )
    .unwrap();
    let raw = fs::read_to_string(dir.path().join("hooks/claude_events.jsonl")).unwrap();
    assert_eq!(
        raw.lines().count(),
        2,
        "an unknown event and a free-text id are not stored"
    );
    for secret in ["secret", "Users", "permission to use", "transcript"] {
        assert!(!raw.contains(secret), "{secret} must not be stored");
    }
    let e = hooks::events(dir.path());
    assert_eq!(
        (e[0].event.as_str(), e[0].kind.as_deref()),
        ("Notification", Some("permission_prompt"))
    );
    assert!(hooks::record(dir.path(), "not json", 4_000).is_err());
}

#[test]
fn the_last_event_decides_the_state_and_ended_sessions_are_gone() {
    let dir = TempDir::new().unwrap();
    for (ev, s, at) in [
        ("SessionStart", "a", 1),
        ("UserPromptSubmit", "a", 2),
        ("Notification", "a", 3),
        ("UserPromptSubmit", "b", 4),
        ("Stop", "b", 5),
        ("SessionStart", "c", 6),
        ("SessionEnd", "c", 7),
        ("UserPromptSubmit", "d", 8),
    ] {
        hooks::record(dir.path(), &payload(ev, s), at).unwrap();
    }
    let live = hooks::live(dir.path(), 10, 1_000);
    let state = |s: &str| live.iter().find(|l| l.session == s).map(|l| l.state);
    assert_eq!(state("a"), Some(LiveState::Waiting));
    assert_eq!(state("b"), Some(LiveState::YourTurn));
    assert_eq!(state("c"), None, "ended");
    assert_eq!(state("d"), Some(LiveState::Working));
    assert!(
        hooks::live(dir.path(), 10_000, 100).is_empty(),
        "too old to be live"
    );
}

#[test]
fn install_adds_beside_existing_hooks_and_uninstall_leaves_theirs() {
    let dir = TempDir::new().unwrap();
    let data = dir.path().join("data");
    let settings = dir.path().join("settings.json");
    let original = json!({
        "model": "x",
        "hooks": {
            "Stop": [{"hooks": [{"type": "command", "command": "node /opt/clawd-hook.js Stop"}]}],
            "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "echo vet"}]}]
        }
    });
    fs::write(&settings, serde_json::to_string_pretty(&original).unwrap()).unwrap();
    let bin = std::path::Path::new("/Applications/tracr.app/Contents/MacOS/tracr");
    assert!(!hooks::installed(&settings));
    hooks::install(&data, &settings, bin, 7).unwrap();
    assert!(hooks::installed(&settings));
    assert!(data.join("backups/claude-settings.hooks.7.json").exists());
    let v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(
        v["hooks"]["Stop"].as_array().unwrap().len(),
        2,
        "theirs stays, ours is added"
    );
    assert_eq!(v["hooks"]["PreToolUse"], original["hooks"]["PreToolUse"]);
    assert_eq!(v["hooks"]["Notification"].as_array().unwrap().len(), 1);

    hooks::install(&data, &settings, bin, 8).unwrap(); // idempotent
    let v2: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(v2["hooks"]["Stop"].as_array().unwrap().len(), 2);

    hooks::uninstall(&settings).unwrap();
    let back: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(back, original, "exactly as it was");
    assert!(!hooks::installed(&settings));
}

#[test]
fn response_times_are_the_waits_between_stop_and_the_next_prompt() {
    let dir = TempDir::new().unwrap();
    for (ev, s, at) in [
        ("Stop", "a", 1_000),
        ("UserPromptSubmit", "a", 61_000), // 60 s
        ("Stop", "a", 70_000),
        ("UserPromptSubmit", "a", 250_000), // 180 s
        ("Stop", "b", 80_000),              // never answered
        ("UserPromptSubmit", "c", 90_000),  // a prompt without a stop before it
    ] {
        hooks::record(dir.path(), &payload(ev, s), at).unwrap();
    }
    let r = hooks::response_times(dir.path(), 0);
    assert_eq!((r.count, r.mean_ms, r.max_ms), (2, 120_000, 180_000));
    assert_eq!(
        hooks::response_times(dir.path(), 100_000).count,
        0,
        "events before `since` do not count"
    );
}
