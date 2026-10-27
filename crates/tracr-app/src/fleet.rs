//! The fleet: which Claude Code sessions run right now and what they cost
//! (T-0015 B2, after Claudoscope's Fleet). Two sources:
//! - Claude Code's own registry, `~/.claude/sessions/<pid>.json`, read from the
//!   folder the user approved. Only the process id, session id, working
//!   folder, times, state, version and Claude's own session name are taken;
//!   the `.key` files beside them are never opened.
//! - the derived store: spend of the last hour, the size of the last request
//!   and how long its prompt cache lives.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

use crate::day::AUTOMATED_MODELS;
use crate::pricing::bundled;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RegistryEntry {
    /// Claude's own session id (`native_session_id` of a derived session).
    pub native_session_id: String,
    pub pid: Option<i64>,
    /// The process is still running.
    pub alive: bool,
    /// `busy`, `idle` or `waiting`, as Claude states it.
    pub status: Option<String>,
    pub status_since: Option<i64>,
    pub started_at: Option<i64>,
    /// The name Claude gives the session (its own topic name or a rename).
    pub name: Option<String>,
    pub cwd: Option<String>,
    pub version: Option<String>,
    /// `interactive` or another kind (a background job).
    pub kind: Option<String>,
}

fn int(v: &Value, k: &str) -> Option<i64> {
    v.get(k).and_then(Value::as_i64)
}

fn text(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_owned)
}

/// Which of these process ids are alive. One `ps` per id: a single invalid id makes `ps` refuse the whole list.
fn alive_pids(pids: &[i64]) -> Vec<i64> {
    pids.iter()
        .copied()
        .filter(|pid| {
            Command::new("ps")
                .args(["-p", &pid.to_string(), "-o", "pid="])
                .output()
                .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == pid.to_string())
        })
        .collect()
}

/// The registry entries under the approved Claude Code home folders, newest first.
pub fn registry(config_roots: &[PathBuf]) -> Vec<RegistryEntry> {
    let mut out = Vec::new();
    for root in config_roots {
        let Ok(rd) = fs::read_dir(root.join("sessions")) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            // Only the plain registry files; the `.key` files beside them are never opened.
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&p) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            let Some(id) = text(&v, "sessionId") else {
                continue;
            };
            out.push(RegistryEntry {
                native_session_id: id,
                pid: int(&v, "pid"),
                alive: false,
                status: text(&v, "status"),
                status_since: int(&v, "statusUpdatedAt"),
                started_at: int(&v, "startedAt"),
                name: text(&v, "name"),
                cwd: text(&v, "cwd"),
                version: text(&v, "version"),
                kind: text(&v, "kind"),
            });
        }
    }
    let alive = alive_pids(&out.iter().filter_map(|e| e.pid).collect::<Vec<_>>());
    for e in &mut out {
        e.alive = e.pid.is_some_and(|p| alive.contains(&p));
    }
    out.sort_by_key(|e| std::cmp::Reverse(e.status_since.or(e.started_at)));
    out
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SessionStat {
    pub native_session_id: String,
    pub session_id: String,
    /// Estimated spend of the last 60 minutes, subagents included (nano-units; `None` when nothing was priced).
    pub cost_last_hour_nano: Option<i64>,
    pub last_request_at: i64,
    /// Input of the last main-chain request in tokens (fresh + cache read + cache write).
    pub context_tokens: Option<i64>,
    /// Share of that request's input read from the cache (0..1).
    pub cache_hit: Option<f64>,
    /// Milliseconds until the cache entry of that request expires (negative: it has expired).
    pub cache_ttl_left_ms: i64,
}

const TTL_5M: i64 = 5 * 60_000;
const TTL_1H: i64 = 60 * 60_000;

/// Per session with usage in the last 24 hours: spend of the last hour, the last request, its cache life.
pub fn stats(conn: &Connection, now: i64, show_automated: bool) -> Result<Vec<SessionStat>> {
    let revision = &bundled().revision;
    let automated = AUTOMATED_MODELS
        .iter()
        .map(|m| format!("'{m}'"))
        .collect::<Vec<_>>()
        .join(",");
    let filter = if show_automated {
        String::new()
    } else {
        format!(" AND COALESCE(u.model, '') NOT IN ({automated})")
    };
    // Spend of the last hour per session, all actors.
    let mut spend: BTreeMap<String, Option<i64>> = BTreeMap::new();
    {
        let mut stmt = conn.prepare(&format!(
            "SELECT u.session_id, SUM(c.amount_nano) FROM usage_record u
             LEFT JOIN cost_estimate c ON c.usage_id = u.id AND c.pricing_revision = ?3
             WHERE u.canonical = 1 AND u.identified = 1 AND u.session_id IS NOT NULL
               AND u.extent_start >= ?1 AND u.extent_start <= ?2{filter}
             GROUP BY u.session_id"
        ))?;
        for row in stmt.query_map(rusqlite::params![now - TTL_1H, now, revision], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
        })? {
            let (id, n) = row?;
            spend.insert(id, n);
        }
    }
    // The last main-chain request per session in the last day.
    let mut stmt = conn.prepare(&format!(
        "SELECT session_id, native_session_id, extent_start, input_fresh, input_total, cache_read, cache_creation, cache_creation_tiers FROM (
            SELECT u.session_id, s.native_session_id, u.extent_start, u.input_fresh, u.input_total, u.cache_read, u.cache_creation, u.cache_creation_tiers,
                   ROW_NUMBER() OVER (PARTITION BY u.session_id ORDER BY u.extent_start DESC) AS rn
            FROM usage_record u
            JOIN agent_session s ON s.id = u.session_id
            LEFT JOIN agent_actor a ON a.id = u.actor_id
            WHERE u.canonical = 1 AND u.identified = 1 AND u.extent_start IS NOT NULL AND u.extent_start >= ?1
              AND u.extent_start <= ?2 AND COALESCE(a.actor_kind, 'main') != 'known'{filter}
         ) WHERE rn = 1"
    ))?;
    let rows = stmt.query_map(rusqlite::params![now - 24 * 3_600_000, now], |r| {
        let tiers: Option<String> = r.get(7)?;
        let one_hour = tiers
            .and_then(|t| serde_json::from_str::<BTreeMap<String, u64>>(&t).ok())
            .is_some_and(|m| m.get("ephemeral_1h_input_tokens").copied().unwrap_or(0) > 0);
        let (fresh, total, read, creation): (Option<i64>, Option<i64>, Option<i64>, Option<i64>) =
            (r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?);
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            fresh,
            total,
            read,
            creation,
            one_hour,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (session_id, native, at, fresh, total, read, creation, one_hour) = row?;
        let all = total.or_else(|| Some(fresh? + read? + creation?));
        let ttl = if one_hour { TTL_1H } else { TTL_5M };
        out.push(SessionStat {
            cost_last_hour_nano: spend.get(&session_id).copied().flatten(),
            native_session_id: native,
            session_id,
            last_request_at: at,
            context_tokens: all,
            cache_hit: all
                .filter(|a| *a > 0)
                .zip(read)
                .map(|(a, r)| r as f64 / a as f64),
            cache_ttl_left_ms: at + ttl - now,
        });
    }
    Ok(out)
}

/// The registry with the process alive check, for the window.
#[derive(Debug, Clone, Serialize)]
pub struct Fleet {
    /// A Claude Code home folder is approved and its registry could be read.
    pub registry_available: bool,
    pub entries: Vec<RegistryEntry>,
    pub stats: Vec<SessionStat>,
}

pub fn fleet(
    conn: &Connection,
    config_roots: &[PathBuf],
    now: i64,
    show_automated: bool,
) -> Result<Fleet> {
    let available = config_roots
        .iter()
        .any(|r: &PathBuf| Path::new(r).join("sessions").is_dir());
    Ok(Fleet {
        registry_available: available,
        entries: registry(config_roots),
        stats: stats(conn, now, show_automated)?,
    })
}
