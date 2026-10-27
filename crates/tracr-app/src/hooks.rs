//! Claude Code session hooks (T-0015, after ClaudeBar's session hooks): who
//! needs you, who finished, who started.
//!
//! An opt-in `tracr hook` command is registered for a few Claude Code events.
//! It keeps only the event name, the session id, the kind of notification
//! where Claude states one, and the time. The text of a notification, the
//! prompt and every other field of the payload are dropped, never written.
//! The installed command never changes Claude's behaviour: no output, exit 0.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The events tracr listens to.
pub const EVENTS: [&str; 5] = [
    "Notification",
    "Stop",
    "UserPromptSubmit",
    "SessionStart",
    "SessionEnd",
];
const MARK: &str = " hook";
const SPOOL_MAX_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookEvent {
    pub at: i64,
    pub session: String,
    pub event: String,
    /// `notification_type` where Claude states one (e.g. `permission_prompt`), a short identifier.
    pub kind: Option<String>,
}

fn spool_path(data_dir: &Path) -> PathBuf {
    data_dir.join("hooks").join("claude_events.jsonl")
}

/// A short identifier-like string, else nothing: free text never passes.
fn ident(v: Option<&Value>) -> Option<String> {
    let s = v?.as_str()?;
    (!s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'))
    .then(|| s.to_owned())
}

/// Stores the event of one hook payload. Everything but the four fields is dropped.
pub fn record(data_dir: &Path, stdin_json: &str, now: i64) -> Result<()> {
    let v: Value = serde_json::from_str(stdin_json).context("hook input is not JSON")?;
    let (Some(session), Some(event)) =
        (ident(v.get("session_id")), ident(v.get("hook_event_name")))
    else {
        return Ok(());
    };
    if !EVENTS.contains(&event.as_str()) {
        return Ok(());
    }
    let line = HookEvent {
        at: now,
        session,
        event,
        kind: ident(v.get("notification_type")),
    };
    let path = spool_path(data_dir);
    fs::create_dir_all(path.parent().context("spool dir")?)?;
    if fs::metadata(&path).is_ok_and(|m| m.len() > SPOOL_MAX_BYTES) {
        let keep: Vec<String> = fs::read_to_string(&path)?
            .lines()
            .rev()
            .take(1000)
            .map(str::to_owned)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        fs::write(&path, keep.join("\n") + "\n")?;
    }
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(f, "{}", serde_json::to_string(&line)?)?;
    Ok(())
}

/// Every stored event, oldest first.
pub fn events(data_dir: &Path) -> Vec<HookEvent> {
    fs::read_to_string(spool_path(data_dir))
        .map(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveState {
    /// Claude is working on a prompt.
    Working,
    /// Claude asks for you (a permission, a plan, an idle prompt).
    Waiting,
    /// Claude finished its turn: your move.
    YourTurn,
    /// The session just started.
    Started,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveSession {
    /// Claude's own session id (matches `native_session_id` of a derived session).
    pub session: String,
    pub state: LiveState,
    /// When it entered this state (epoch ms).
    pub since: i64,
    pub kind: Option<String>,
}

/// The state of each session whose last event is newer than `max_age_ms`; ended sessions are left out.
pub fn live(data_dir: &Path, now: i64, max_age_ms: i64) -> Vec<LiveSession> {
    let mut last: BTreeMap<String, HookEvent> = BTreeMap::new();
    for e in events(data_dir) {
        last.insert(e.session.clone(), e);
    }
    let mut out: Vec<LiveSession> = last
        .into_values()
        .filter(|e| now - e.at <= max_age_ms)
        .filter_map(|e| {
            let state = match e.event.as_str() {
                "UserPromptSubmit" => LiveState::Working,
                "Notification" => LiveState::Waiting,
                "Stop" => LiveState::YourTurn,
                "SessionStart" => LiveState::Started,
                _ => return None, // SessionEnd
            };
            Some(LiveSession {
                session: e.session,
                state,
                since: e.at,
                kind: e.kind,
            })
        })
        .collect();
    out.sort_by_key(|s| s.since);
    out
}

// ---- opt-in install into Claude Code's settings ---------------------------------

fn read_settings(path: &Path) -> Result<Value> {
    match fs::read_to_string(path) {
        Ok(t) if t.trim().is_empty() => Ok(json!({})),
        Ok(t) => serde_json::from_str(&t)
            .with_context(|| format!("{} is not valid JSON", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.into()),
    }
}

fn write_atomically(path: &Path, v: &Value) -> Result<()> {
    fs::create_dir_all(path.parent().context("settings dir")?)?;
    let tmp = path.with_extension("json.tracr-tmp");
    fs::write(&tmp, serde_json::to_string_pretty(v)? + "\n")?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn is_ours(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hs| {
            hs.iter().any(|h| {
                h.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|c| c.ends_with(MARK) && c.contains("--data-dir"))
            })
        })
}

/// Whether tracr's hook is registered for every event.
pub fn installed(settings: &Path) -> bool {
    let Ok(v) = read_settings(settings) else {
        return false;
    };
    EVENTS.iter().all(|e| {
        v.pointer(&format!("/hooks/{e}"))
            .and_then(Value::as_array)
            .is_some_and(|a| a.iter().any(is_ours))
    })
}

/// Registers `tracr hook` for the events, beside whatever hooks are there. A copy of the settings is made first.
pub fn install(data_dir: &Path, settings: &Path, tracr_bin: &Path, now: i64) -> Result<()> {
    let mut v = read_settings(settings)?;
    let root = v
        .as_object_mut()
        .ok_or_else(|| anyhow!("settings.json is not an object"))?;
    if settings.exists() {
        let dir = data_dir.join("backups");
        fs::create_dir_all(&dir)?;
        fs::copy(
            settings,
            dir.join(format!("claude-settings.hooks.{now}.json")),
        )?;
    }
    let command = format!(
        "{} --data-dir {}{MARK}",
        shell_quote(&tracr_bin.display().to_string()),
        shell_quote(&data_dir.display().to_string())
    );
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .ok_or_else(|| anyhow!("settings hooks is not an object"))?;
    for e in EVENTS {
        let list = hooks.entry(e).or_insert_with(|| json!([]));
        let list = list
            .as_array_mut()
            .ok_or_else(|| anyhow!("settings hooks.{e} is not a list"))?;
        list.retain(|x| !is_ours(x));
        list.push(json!({ "hooks": [{ "type": "command", "command": command }] }));
    }
    write_atomically(settings, &v)
}

/// Removes tracr's entries and leaves every other hook as it was.
pub fn uninstall(settings: &Path) -> Result<()> {
    let mut v = read_settings(settings)?;
    let Some(hooks) = v.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    for e in EVENTS {
        if let Some(list) = hooks.get_mut(e).and_then(Value::as_array_mut) {
            list.retain(|x| !is_ours(x));
        }
        if hooks
            .get(e)
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            hooks.remove(e);
        }
    }
    if hooks.is_empty() {
        v.as_object_mut().map(|o| o.remove("hooks"));
    }
    write_atomically(settings, &v)
}
