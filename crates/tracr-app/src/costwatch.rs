//! Cost alerts (T-0015 B1, after Claudoscope's cost alerts): a single
//! session over a cap, spend inside a rolling window (which also catches a
//! runaway), the day, the month. An alert fires when a figure crosses its
//! threshold and again at each doubling (X, 2X, 4X ...), not on a timer.
//! Every figure is an API-equivalent estimate from the price catalog.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::day::{AUTOMATED_MODELS, ReportingZone};
use crate::pricing::bundled;

/// Thresholds in whole currency units (the catalog's currency); `0` switches one off.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CostAlertPrefs {
    pub enabled: bool,
    pub session: u32,
    /// Spend inside the last `window_minutes` minutes.
    pub window: u32,
    pub window_minutes: u32,
    pub day: u32,
    pub month: u32,
}

impl Default for CostAlertPrefs {
    fn default() -> Self {
        Self {
            enabled: false,
            session: 25,
            window: 15,
            window_minutes: 30,
            day: 0,
            month: 0,
        }
    }
}

fn path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("cost_alerts.json")
}

pub fn load(data_dir: &Path) -> CostAlertPrefs {
    fs::read_to_string(path(data_dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(data_dir: &Path, prefs: &CostAlertPrefs) -> Result<()> {
    fs::create_dir_all(data_dir)?;
    fs::write(path(data_dir), serde_json::to_string_pretty(prefs)? + "\n")?;
    Ok(())
}

/// What has been spent right now, in the catalog's currency units.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Measures {
    /// Spend per session over the last 24 hours (session id, folder name, amount).
    pub sessions: Vec<(String, Option<String>, f64)>,
    pub window: f64,
    pub day: f64,
    pub month: f64,
}

fn sum_since(
    conn: &Connection,
    since: i64,
    now: i64,
    revision: &str,
    show_automated: bool,
) -> Result<f64> {
    let automated: String = AUTOMATED_MODELS
        .iter()
        .map(|m| format!("'{m}'"))
        .collect::<Vec<_>>()
        .join(",");
    let filter = if show_automated {
        String::new()
    } else {
        format!(" AND COALESCE(u.model, '') NOT IN ({automated})")
    };
    let nano: Option<i64> = conn.query_row(
        &format!(
            "SELECT SUM(c.amount_nano) FROM usage_record u
             JOIN cost_estimate c ON c.usage_id = u.id AND c.pricing_revision = ?3
             WHERE u.canonical = 1 AND u.identified = 1 AND u.extent_start >= ?1 AND u.extent_start <= ?2{filter}"
        ),
        rusqlite::params![since, now, revision],
        |r| r.get(0),
    )?;
    Ok(nano.unwrap_or(0) as f64 / 1e9)
}

/// The figures the alerts are judged on, as of `now`.
pub fn measure(
    conn: &Connection,
    zone: &ReportingZone,
    now: i64,
    prefs: &CostAlertPrefs,
    show_automated: bool,
) -> Result<Measures> {
    let revision = &bundled().revision;
    let day = zone.day_of(now);
    let day_start = day
        .map(|d| zone.bounds(d))
        .transpose()?
        .map_or(now, |b| b.0);
    let month_start = day
        .map(|d| d.first_of_month())
        .map(|d| zone.bounds(d))
        .transpose()?
        .map_or(now, |b| b.0);
    let day_nano = sum_since(conn, day_start, now, revision, show_automated)?;
    let month = sum_since(conn, month_start, now, revision, show_automated)?;
    let window = sum_since(
        conn,
        now - i64::from(prefs.window_minutes.max(1)) * 60_000,
        now,
        revision,
        show_automated,
    )?;
    let automated: String = AUTOMATED_MODELS
        .iter()
        .map(|m| format!("'{m}'"))
        .collect::<Vec<_>>()
        .join(",");
    let filter = if show_automated {
        String::new()
    } else {
        format!(" AND COALESCE(u.model, '') NOT IN ({automated})")
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT u.session_id, s.label, SUM(c.amount_nano)
         FROM usage_record u
         JOIN cost_estimate c ON c.usage_id = u.id AND c.pricing_revision = ?2
         LEFT JOIN agent_session s ON s.id = u.session_id
         WHERE u.canonical = 1 AND u.identified = 1 AND u.session_id IS NOT NULL AND u.extent_start >= ?1{filter}
         GROUP BY u.session_id"
    ))?;
    let sessions = stmt
        .query_map(rusqlite::params![now - 24 * 3_600_000, revision], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<i64>>(2)?.unwrap_or(0) as f64 / 1e9,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Measures {
        sessions,
        window,
        day: day_nano,
        month,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Session { label: Option<String> },
    Window { minutes: u32 },
    Day,
    Month,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CostAlert {
    pub kind: Kind,
    pub amount: f64,
    /// The threshold that was crossed (the base, or a doubling of it).
    pub crossed: f64,
}

/// The step a value has reached for a threshold: 0 below it, 1 at it, 2 at twice it, 3 at four times...
fn step(value: f64, threshold: f64) -> u32 {
    if threshold <= 0.0 || value < threshold {
        0
    } else {
        1 + (value / threshold).log2().floor() as u32
    }
}

/// Remembers the step reached per figure so an alert fires only when a higher one is reached.
#[derive(Debug, Default)]
pub struct CostWatcher {
    seen: HashMap<String, u32>,
    period: String,
}

impl CostWatcher {
    /// `period` changes with the day and the month; a new period starts again from nothing.
    pub fn changes(&mut self, m: &Measures, prefs: &CostAlertPrefs, day: &str) -> Vec<CostAlert> {
        let mut out = Vec::new();
        if self.period != day {
            self.seen
                .retain(|k, _| k.starts_with("session:") || k.starts_with("window:"));
            self.seen
                .retain(|k, _| !k.starts_with("day:") && !k.starts_with("month:"));
            self.period = day.to_owned();
        }
        if !prefs.enabled {
            return out;
        }
        let first_look = self.seen.is_empty();
        let mut check =
            |key: String, value: f64, threshold: u32, kind: Kind, out: &mut Vec<CostAlert>| {
                let t = f64::from(threshold);
                let now = step(value, t);
                let before = self.seen.insert(key, now).unwrap_or(0);
                if now > before && !first_look {
                    out.push(CostAlert {
                        kind,
                        amount: value,
                        crossed: t * f64::from(1u32 << (now - 1)),
                    });
                }
            };
        if prefs.session > 0 {
            for (id, label, v) in &m.sessions {
                check(
                    format!("session:{id}"),
                    *v,
                    prefs.session,
                    Kind::Session {
                        label: label.clone(),
                    },
                    &mut out,
                );
            }
        }
        if prefs.window > 0 {
            check(
                format!("window:{}", prefs.window_minutes),
                m.window,
                prefs.window,
                Kind::Window {
                    minutes: prefs.window_minutes,
                },
                &mut out,
            );
        }
        if prefs.day > 0 {
            check(format!("day:{day}"), m.day, prefs.day, Kind::Day, &mut out);
        }
        if prefs.month > 0 {
            check(
                format!("month:{}", &day[..day.len().min(7)]),
                m.month,
                prefs.month,
                Kind::Month,
                &mut out,
            );
        }
        out
    }
}
