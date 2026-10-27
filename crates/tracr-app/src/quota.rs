//! How a quota window is judged and shown outside the window: levels, pace,
//! the menu bar text and alerts when a window gets worse (T-0015, after the
//! ideas of ClaudeBar: levels by what is left, optional pace awareness, a
//! countdown beside the percentage).
//!
//! Every figure is the provider's own statement (see `limits`); nothing here
//! estimates a limit size.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::limits::LimitWindow;

/// Preferences for showing quota. Stored beside the data, read by the window and the menu bar alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuotaPrefs {
    /// Show what is left instead of what is used.
    pub remaining: bool,
    /// Judge a window by its pace, not by the level alone.
    pub pace_aware: bool,
    /// Show the tightest window in the menu bar.
    pub menu_bar: bool,
    /// Notify when a window gets worse.
    pub notify: bool,
    /// Show each provider's tightest window as a ring instead of one bar per window.
    pub rings: bool,
    /// Notify when Claude Code needs you (a permission, a plan, an idle prompt).
    pub notify_waiting: bool,
    /// Notify when Claude Code finishes and it is your turn.
    pub notify_finished: bool,
}

impl Default for QuotaPrefs {
    fn default() -> Self {
        Self {
            remaining: false,
            pace_aware: true,
            menu_bar: true,
            notify: true,
            rings: false,
            notify_waiting: true,
            notify_finished: true,
        }
    }
}

fn prefs_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("quota_prefs.json")
}

pub fn load_prefs(data_dir: &Path) -> QuotaPrefs {
    fs::read_to_string(prefs_path(data_dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_prefs(data_dir: &Path, prefs: &QuotaPrefs) -> Result<()> {
    fs::create_dir_all(data_dir)?;
    fs::write(
        prefs_path(data_dir),
        serde_json::to_string_pretty(prefs)? + "\n",
    )?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Ok,
    Warn,
    Crit,
    Depleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Pace {
    Hot,
    OnTrack,
    Room,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Forecast {
    pub pace: Pace,
    /// Share of the window that had passed when the figure was stated.
    pub elapsed: f64,
    /// When the window would be used up at the pace so far, if before its reset (epoch ms).
    pub runs_out_at: Option<i64>,
}

/// Under this share of the window gone, the pace is noise.
const MIN_ELAPSED: f64 = 0.03;
/// Percentage points ahead of the time gone that count as running hot.
const HOT_POINTS: f64 = 5.0;

/// Pace as of the moment the provider stated the figure. `None` without a window to measure, with nothing used, or after the reset.
pub fn forecast(w: &LimitWindow) -> Option<Forecast> {
    let (resets_at, minutes) = (w.resets_at?, w.window_minutes?);
    if w.expired || minutes <= 0 {
        return None;
    }
    let length = (minutes * 60_000) as f64;
    let start = resets_at as f64 - length;
    let elapsed = (w.observed_at as f64 - start) / length;
    if !(MIN_ELAPSED..=1.0).contains(&elapsed) {
        return None;
    }
    let used = w.used_percent.clamp(0.0, 100.0);
    if used >= 100.0 {
        return None;
    }
    let ahead = used - elapsed * 100.0;
    let pace = if ahead > HOT_POINTS {
        Pace::Hot
    } else if ahead < -HOT_POINTS {
        Pace::Room
    } else {
        Pace::OnTrack
    };
    let runs_out_at = (pace == Pace::Hot && used > 0.0)
        .then(|| w.observed_at as f64 + ((100.0 - used) / used) * (w.observed_at as f64 - start))
        .filter(|out| *out < resets_at as f64)
        .map(|out| out as i64);
    Some(Forecast {
        pace,
        elapsed,
        runs_out_at,
    })
}

/// Healthy from 50 % left, warning from 20 %, critical below, depleted at none;
/// with pace awareness a window that lasts is healthy and one running hot is a warning (down to critical).
pub fn level(w: &LimitWindow, pace_aware: bool) -> Level {
    if w.expired {
        return Level::Ok;
    }
    let left = 100.0 - w.used_percent;
    if left <= 0.0 {
        return Level::Depleted;
    }
    if left < 20.0 {
        return Level::Crit;
    }
    if pace_aware && let Some(f) = forecast(w) {
        return if f.pace == Pace::Hot {
            Level::Warn
        } else {
            Level::Ok
        };
    }
    if left < 50.0 { Level::Warn } else { Level::Ok }
}

fn span(ms: i64) -> String {
    let m = (ms.max(0) + 30_000) / 60_000;
    if m < 60 {
        format!("{m}m")
    } else if m < 48 * 60 {
        format!("{}h{:02}m", m / 60, m % 60)
    } else {
        format!("{}d", (m + 12 * 60) / (24 * 60))
    }
}

/// The window the menu bar shows: the one with the least left that has not reset since.
pub fn tightest(windows: &[LimitWindow]) -> Option<&LimitWindow> {
    windows
        .iter()
        .filter(|w| !w.expired)
        .max_by(|a, b| a.used_percent.total_cmp(&b.used_percent))
}

/// "62% · 1h38m" (or "38% left"): the tightest window and when it resets.
pub fn menu_bar_text(windows: &[LimitWindow], prefs: &QuotaPrefs, now: i64) -> Option<String> {
    let w = tightest(windows)?;
    let shown = if prefs.remaining {
        100.0 - w.used_percent
    } else {
        w.used_percent
    };
    let mut text = format!("{:.0}%", shown.clamp(0.0, 100.0));
    if let Some(r) = w.resets_at {
        text.push_str(&format!(" · {}", span(r - now)));
    }
    if level(w, prefs.pace_aware) >= Level::Crit {
        text.push('!');
    }
    Some(text)
}

/// A window that got worse since the last look.
#[derive(Debug, Clone, PartialEq)]
pub struct Alert {
    pub provider: String,
    pub window: String,
    pub level: Level,
    pub used_percent: f64,
    pub resets_at: Option<i64>,
}

/// Remembers each window's level so only a change for the worse is reported. The first look reports nothing.
#[derive(Debug, Default)]
pub struct Watcher {
    seen: HashMap<String, Level>,
}

impl Watcher {
    pub fn changes(&mut self, windows: &[LimitWindow], pace_aware: bool) -> Vec<Alert> {
        let mut out = Vec::new();
        let first_look = self.seen.is_empty();
        for w in windows.iter().filter(|w| !w.expired) {
            // A new reset time is a new window: it starts from healthy.
            let key = format!("{}/{}/{}", w.provider, w.window, w.resets_at.unwrap_or(0));
            let now = level(w, pace_aware);
            let before = self.seen.insert(key, now);
            let worse = match before {
                Some(b) => now > b,
                None => !first_look && now > Level::Ok,
            };
            if worse {
                out.push(Alert {
                    provider: w.provider.clone(),
                    window: w.window.clone(),
                    level: now,
                    used_percent: w.used_percent,
                    resets_at: w.resets_at,
                });
            }
        }
        out
    }
}
