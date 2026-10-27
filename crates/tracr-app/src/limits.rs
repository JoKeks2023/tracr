//! Usage limits as the providers report them (T-0010, T-0015).
//!
//! A snapshot is "x % of a window used at time t, resets at r", exactly as the
//! provider states it. It is a separate kind from money (canon: money kinds
//! stay apart) and is never attributed to a session or project: parallel
//! sessions share one limit, so only the snapshot is honest.
//!
//! Sources:
//! - Codex writes `rate_limits` into its rollout records. The newest ones are
//!   read on demand; only the percent, window length, reset time and plan are
//!   taken, nothing else of the record.
//! - Claude Code hands `rate_limits` to its `statusLine` command. tracr's
//!   opt-in tap (`tracr statusline-tap`) stores the same few numbers in a
//!   spool file in the data directory.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracr_core::time::UtcMillis;

/// One provider-reported window.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LimitWindow {
    /// `codex` or `claude_code`.
    pub provider: String,
    /// `five_hour`, `weekly` or `minutes:<n>` for a window of another length.
    pub window: String,
    pub window_minutes: Option<i64>,
    pub used_percent: f64,
    /// When the window resets (epoch ms), as the provider states it.
    pub resets_at: Option<i64>,
    /// When the provider stated this figure (epoch ms).
    pub observed_at: i64,
    pub plan: Option<String>,
    /// The reset time has passed since the figure was stated: the figure is history, not the current state.
    pub expired: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Limits {
    pub windows: Vec<LimitWindow>,
    pub claude_tap: TapState,
}

/// Whether the Claude Code statusLine tap is set up.
#[derive(Debug, Clone, Serialize)]
pub struct TapState {
    pub installed: bool,
    /// The statusLine command that was there before, which the tap passes the data on to.
    pub chained: bool,
    pub settings_path: String,
}

fn window_key(minutes: Option<i64>) -> String {
    match minutes {
        Some(300) => "five_hour".to_owned(),
        Some(10_080) => "weekly".to_owned(),
        Some(m) => format!("minutes:{m}"),
        None => "unknown".to_owned(),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

// ---- Codex: newest rollout records ---------------------------------------------

const TAIL_BYTES: u64 = 4 * 1024 * 1024;

fn is_rollout(p: &Path) -> bool {
    let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    n.starts_with("rollout-") && (n.ends_with(".jsonl") || n.ends_with(".jsonl.zst"))
}

fn collect_rollouts(dir: &Path, depth: u8, out: &mut Vec<(SystemTime, PathBuf)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() && depth < 6 {
            collect_rollouts(&p, depth + 1, out);
        } else if meta.is_file()
            && is_rollout(&p)
            && let Ok(m) = meta.modified()
        {
            out.push((m, p));
        }
    }
}

/// The last `rate_limits` of one rollout file, as windows.
fn rollout_windows(path: &Path) -> Option<Vec<LimitWindow>> {
    let file = fs::File::open(path).ok()?;
    let mut file = if path.extension().is_some_and(|e| e == "zst") {
        tracr_ingest::reader::decompress(file).ok()?
    } else {
        file
    };
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES)))
        .ok()?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail).ok()?;
    let text = String::from_utf8_lossy(&tail);
    let now = now_ms();
    text.lines()
        .rev()
        .filter(|l| l.contains("\"rate_limits\""))
        .find_map(|l| codex_line_windows(l, now))
}

fn codex_line_windows(line: &str, now: i64) -> Option<Vec<LimitWindow>> {
    let v: Value = serde_json::from_str(line).ok()?;
    let observed_at = UtcMillis::parse_rfc3339(v.get("timestamp")?.as_str()?)?.0;
    let rl = v.get("payload")?.get("rate_limits")?;
    let plan = rl
        .get("plan_type")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut out = Vec::new();
    for key in ["primary", "secondary"] {
        let Some(w) = rl.get(key).filter(|w| w.is_object()) else {
            continue;
        };
        let Some(used) = w.get("used_percent").and_then(Value::as_f64) else {
            continue;
        };
        let minutes = w.get("window_minutes").and_then(Value::as_i64);
        let resets_at = w.get("resets_at").and_then(Value::as_i64).map(|s| s * 1000);
        out.push(LimitWindow {
            provider: "codex".to_owned(),
            window: window_key(minutes),
            window_minutes: minutes,
            used_percent: used,
            expired: resets_at.is_some_and(|r| r < now),
            resets_at,
            observed_at,
            plan: plan.clone(),
        });
    }
    (!out.is_empty()).then_some(out)
}

/// The newest Codex figures under the approved provider roots.
pub fn codex_windows(roots: &[PathBuf]) -> Vec<LimitWindow> {
    let mut files = Vec::new();
    for r in roots {
        collect_rollouts(r, 0, &mut files);
    }
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    let mut best: Vec<LimitWindow> = Vec::new();
    // The newest file that carries limits wins; look at a few in case the newest has none yet.
    for (_, p) in files.iter().take(6) {
        if let Some(w) = rollout_windows(p) {
            if best
                .first()
                .is_none_or(|b| w[0].observed_at > b.observed_at)
            {
                best = w;
            }
            break;
        }
    }
    best
}

// ---- Claude Code: statusLine tap -------------------------------------------------

fn spool_path(data_dir: &Path) -> PathBuf {
    data_dir.join("limits").join("claude_statusline.jsonl")
}

fn prev_path(data_dir: &Path) -> PathBuf {
    data_dir.join("limits").join("claude_statusline_prev.json")
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct SpoolLine {
    at: i64,
    five_hour: Option<SpoolWindow>,
    seven_day: Option<SpoolWindow>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
struct SpoolWindow {
    used_percentage: f64,
    resets_at: Option<i64>,
}

fn spool_window(v: &Value, key: &str) -> Option<SpoolWindow> {
    let w = v.get("rate_limits")?.get(key)?;
    Some(SpoolWindow {
        used_percentage: w.get("used_percentage")?.as_f64()?,
        resets_at: w.get("resets_at").and_then(Value::as_i64),
    })
}

const SPOOL_MAX_BYTES: u64 = 512 * 1024;

/// Keeps only the limit numbers of a statusLine payload. Nothing else of the
/// payload is stored. A line is written only when a figure changed.
pub fn tap(data_dir: &Path, stdin_json: &str, now: i64) -> Result<()> {
    let v: Value = serde_json::from_str(stdin_json).context("statusLine input is not JSON")?;
    let line = SpoolLine {
        at: now,
        five_hour: spool_window(&v, "five_hour"),
        seven_day: spool_window(&v, "seven_day"),
    };
    if line.five_hour.is_none() && line.seven_day.is_none() {
        return Ok(());
    }
    let path = spool_path(data_dir);
    fs::create_dir_all(path.parent().context("spool dir")?)?;
    if let Some(last) = last_spool_line(&path)
        && last.five_hour == line.five_hour
        && last.seven_day == line.seven_day
    {
        return Ok(());
    }
    if fs::metadata(&path).is_ok_and(|m| m.len() > SPOOL_MAX_BYTES) {
        let keep: Vec<String> = fs::read_to_string(&path)?
            .lines()
            .rev()
            .take(500)
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

fn last_spool_line(path: &Path) -> Option<SpoolLine> {
    let text = fs::read_to_string(path).ok()?;
    text.lines()
        .rev()
        .find_map(|l| serde_json::from_str(l).ok())
}

pub fn claude_windows(data_dir: &Path) -> Vec<LimitWindow> {
    let Some(last) = last_spool_line(&spool_path(data_dir)) else {
        return Vec::new();
    };
    let now = now_ms();
    [(300, last.five_hour), (10_080, last.seven_day)]
        .into_iter()
        .filter_map(|(minutes, w)| {
            let w = w?;
            let resets_at = w.resets_at.map(|s| s * 1000);
            Some(LimitWindow {
                provider: "claude_code".to_owned(),
                window: window_key(Some(minutes)),
                window_minutes: Some(minutes),
                used_percent: w.used_percentage,
                expired: resets_at.is_some_and(|r| r < now),
                resets_at,
                observed_at: last.at,
                plan: None,
            })
        })
        .collect()
}

/// The command the tap passes the payload on to (what the statusLine was before), if any.
pub fn chained_command(data_dir: &Path) -> Option<String> {
    let v: Value = serde_json::from_str(&fs::read_to_string(prev_path(data_dir)).ok()?).ok()?;
    v.get("command")?.as_str().map(str::to_owned)
}

// ---- Claude Code settings: opt-in install ----------------------------------------

/// `~/.claude/settings.json` of the user.
pub fn default_claude_settings() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude").join("settings.json"))
}

const MARK: &str = "statusline-tap";

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

pub fn tap_state(data_dir: &Path, settings: &Path) -> TapState {
    let installed = read_settings(settings)
        .ok()
        .and_then(|v| {
            v.pointer("/statusLine/command")
                .and_then(Value::as_str)
                .map(|c| c.contains(MARK))
        })
        .unwrap_or(false);
    TapState {
        installed,
        chained: installed && chained_command(data_dir).is_some(),
        settings_path: settings.display().to_string(),
    }
}

/// Points Claude Code's `statusLine` at the tap. The previous command is kept
/// and run by the tap, so what the user sees does not change. A copy of the
/// settings file is made first.
pub fn install_tap(data_dir: &Path, settings: &Path, tracr_bin: &Path, now: i64) -> Result<()> {
    let mut v = read_settings(settings)?;
    let obj = v
        .as_object_mut()
        .ok_or_else(|| anyhow!("settings.json is not an object"))?;
    let previous = obj
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    if previous.as_deref().is_some_and(|c| c.contains(MARK)) {
        return Ok(()); // already installed
    }
    if settings.exists() {
        let dir = data_dir.join("backups");
        fs::create_dir_all(&dir)?;
        fs::copy(settings, dir.join(format!("claude-settings.{now}.json")))?;
    }
    fs::create_dir_all(data_dir.join("limits"))?;
    fs::write(
        prev_path(data_dir),
        serde_json::to_string(&json!({ "command": previous, "original": obj.get("statusLine") }))?,
    )?;
    let command = format!(
        "{} --data-dir {} {MARK}",
        shell_quote(&tracr_bin.display().to_string()),
        shell_quote(&data_dir.display().to_string())
    );
    let mut status = obj
        .get("statusLine")
        .filter(|s| s.is_object())
        .cloned()
        .unwrap_or_else(|| json!({ "type": "command" }));
    status["type"] = json!("command");
    status["command"] = json!(command);
    obj.insert("statusLine".to_owned(), status);
    write_atomically(settings, &v)
}

/// Restores the statusLine that was there before (or removes it if there was none).
pub fn uninstall_tap(data_dir: &Path, settings: &Path) -> Result<()> {
    let mut v = read_settings(settings)?;
    let installed = v
        .pointer("/statusLine/command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(MARK));
    if !installed {
        return Ok(());
    }
    let original = fs::read_to_string(prev_path(data_dir))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|p| p.get("original").cloned())
        .filter(|o| !o.is_null());
    let obj = v
        .as_object_mut()
        .ok_or_else(|| anyhow!("settings.json is not an object"))?;
    match original {
        Some(o) => {
            obj.insert("statusLine".to_owned(), o);
        }
        None => {
            obj.remove("statusLine");
        }
    }
    write_atomically(settings, &v)?;
    let _ = fs::remove_file(prev_path(data_dir));
    Ok(())
}

/// Every window the providers have reported: Codex from its rollouts, Claude from the tap.
pub fn read(data_dir: &Path, provider_roots: &[PathBuf], settings: &Path) -> Limits {
    let mut windows = codex_windows(provider_roots);
    windows.extend(claude_windows(data_dir));
    Limits {
        windows,
        claude_tap: tap_state(data_dir, settings),
    }
}
