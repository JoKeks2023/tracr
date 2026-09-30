//! CSV export (reporting §5, pulled forward from S4 at the user's request).
//!
//! * `agent-sessions`: one row per reporting day × agent session × actor;
//! * `timesheet`: human time, one row per atom (counted and unknown apart);
//! * everything: one CSV per derived table, into a folder.
//!
//! Agent time is never human time.
//!
//! Conventions (documented in `docs/export.md`):
//! * UTF-8, comma-separated, RFC 4180 quoting, header row, `\n` line ends;
//! * times are local ISO 8601 in the reporting timezone, with offset;
//! * an empty cell means *unknown or not applicable*, never zero;
//! * `human_prompts` is on the session's main-chain row only, so summing
//!   the column never double-counts;
//! * a token column sums the records that state it; `*_unknown_records`
//!   counts records that do not (unknown, not zero).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use anyhow::{Result, bail};
use jiff::civil::Date;

use crate::Reader;
use crate::day::{DayAgents, ReportingZone};

pub const AGENT_SESSIONS_COLUMNS: &[&str] = &[
    "day",
    "timezone",
    "provider",
    "session_id",
    "native_session_id",
    "project_label",
    "repository",
    "cwd",
    "git_branch",
    "actor_kind",
    "agent_id",
    "first_activity",
    "last_activity",
    "execution_minutes",
    "segments",
    "turns",
    "tool_calls",
    "human_prompts",
    "models",
    "input_tokens",
    "cache_write_tokens",
    "cache_read_tokens",
    "output_tokens",
    "input_unknown_records",
    "cache_write_unknown_records",
    "cache_read_unknown_records",
    "output_unknown_records",
    "unidentified_usage_records",
    "derived_as_of",
];

/// RFC 4180 field quoting.
fn field(out: &mut String, value: &str) {
    if value.contains([',', '"', '\n', '\r']) {
        out.push('"');
        out.push_str(&value.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(value);
    }
}

fn row(out: &mut String, values: &[String]) {
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        field(out, v);
    }
    out.push('\n');
}

fn local_iso(zone: &ReportingZone, ms: i64) -> String {
    jiff::Timestamp::from_millisecond(ms)
        .map(|t| {
            t.to_zoned(zone.tz.clone())
                .strftime("%Y-%m-%dT%H:%M:%S%:z")
                .to_string()
        })
        .unwrap_or_default()
}

fn minutes(ms: i64) -> String {
    format!("{:.1}", ms as f64 / 60_000.0)
}

fn day_rows(out: &mut String, zone: &ReportingZone, v: &DayAgents) {
    let as_of = v
        .derived_as_of
        .map(|t| local_iso(zone, t))
        .unwrap_or_default();
    for s in &v.sessions {
        for a in &s.actors {
            let first = a.segments.iter().map(|g| g.start).min();
            let last = a.segments.iter().map(|g| g.end).max();
            let usage: Vec<_> = s
                .usage
                .iter()
                .filter(|u| u.actor_id.as_deref() == Some(&a.actor_id))
                .collect();
            let models: BTreeSet<&str> = usage.iter().filter_map(|u| u.model.as_deref()).collect();
            let sum = |f: fn(&crate::day::UsageLine) -> crate::day::TokenCount| {
                usage.iter().fold((0i64, 0i64), |(s, u), l| {
                    (s + f(l).sum, u + f(l).unknown_records)
                })
            };
            let (input, input_u) = sum(|l| l.input_fresh);
            let (cw, cw_u) = sum(|l| l.cache_creation);
            let (cr, cr_u) = sum(|l| l.cache_read);
            let (out_t, out_u) = sum(|l| l.output);
            let unidentified: i64 = usage
                .iter()
                .filter(|u| !u.identified)
                .map(|u| u.records)
                .sum();
            let main_row = a.actor_kind != "known";
            let has_usage = !usage.is_empty();
            let tok = |n: i64| {
                if has_usage {
                    n.to_string()
                } else {
                    String::new()
                }
            };
            row(
                out,
                &[
                    v.day.clone(),
                    v.timezone.clone(),
                    s.provider.clone(),
                    s.session_id.clone(),
                    s.native_session_id.clone(),
                    s.label.clone().unwrap_or_default(),
                    s.repository.clone().unwrap_or_default(),
                    s.cwd.clone().unwrap_or_default(),
                    s.git_branch.clone().unwrap_or_default(),
                    a.actor_kind.clone(),
                    a.native_agent_id.clone().unwrap_or_default(),
                    first.map(|t| local_iso(zone, t)).unwrap_or_default(),
                    last.map(|t| local_iso(zone, t)).unwrap_or_default(),
                    minutes(a.execution_ms),
                    a.segments.len().to_string(),
                    a.turns.to_string(),
                    a.tool_calls.to_string(),
                    if main_row {
                        s.engagement_instants.len().to_string()
                    } else {
                        String::new()
                    },
                    models.into_iter().collect::<Vec<_>>().join(";"),
                    tok(input),
                    tok(cw),
                    tok(cr),
                    tok(out_t),
                    tok(input_u),
                    tok(cw_u),
                    tok(cr_u),
                    tok(out_u),
                    tok(unidentified),
                    as_of.clone(),
                ],
            );
        }
    }
}

/// Agent sessions of every day with data in `[from, to]` (inclusive), as CSV.
pub fn agent_sessions_csv(reader: &Reader, from: &str, to: &str) -> Result<String> {
    let (days, zone) = days_in(reader, from, to)?;
    let mut out = String::new();
    row(
        &mut out,
        &AGENT_SESSIONS_COLUMNS
            .iter()
            .map(|c| (*c).to_owned())
            .collect::<Vec<_>>(),
    );
    for d in days {
        day_rows(&mut out, &zone, &reader.day_agents(&d)?);
    }
    Ok(out)
}

/// A default file name, e.g. `tracr-agent-sessions_2026-09-01_2026-09-30.csv`.
pub fn default_file_name(from: &str, to: &str) -> String {
    let mut s = String::new();
    let _ = write!(s, "tracr-agent-sessions_{from}_{to}.csv");
    s
}

// ---------------------------------------------------------------------------
// Timesheet: human time, one row per atom
// ---------------------------------------------------------------------------

pub const TIMESHEET_COLUMNS: &[&str] = &[
    "day",
    "timezone",
    "start",
    "end",
    "status",
    "counted_minutes",
    "unknown_minutes",
    "counting_basis",
    "unknown_reason",
    "repository",
    "worktree",
    "attribution_state",
    "session_id",
    "native_session_id",
    "project_label",
    "cwd",
    "git_branch",
    "start_boundary",
    "end_boundary",
    "reasons",
    "derived_as_of",
];

/// Human time of every day with data in `[from, to]`: one row per atom.
/// Counted and unknown minutes are separate columns and never one total;
/// blank time (no evidence, locked, asleep) has no row.
pub fn timesheet_csv(reader: &Reader, from: &str, to: &str) -> Result<String> {
    let (days, zone) = days_in(reader, from, to)?;
    let mut out = String::new();
    row(
        &mut out,
        &TIMESHEET_COLUMNS
            .iter()
            .map(|c| (*c).to_owned())
            .collect::<Vec<_>>(),
    );
    for d in days {
        let v = reader.day_agents(&d)?;
        let as_of = v
            .derived_as_of
            .map(|t| local_iso(&zone, t))
            .unwrap_or_default();
        for a in &v.human.atoms {
            let session = a
                .engaged_session
                .as_ref()
                .and_then(|id| v.sessions.iter().find(|s| &s.session_id == id));
            let mins = minutes(a.end - a.start);
            row(
                &mut out,
                &[
                    v.day.clone(),
                    v.timezone.clone(),
                    local_iso(&zone, a.start),
                    local_iso(&zone, a.end),
                    a.status.clone(),
                    if a.status == "work" {
                        mins.clone()
                    } else {
                        String::new()
                    },
                    if a.status == "unknown" {
                        mins
                    } else {
                        String::new()
                    },
                    if a.status == "work" {
                        if a.duration_basis == "inferred_continuity" {
                            "inferred"
                        } else {
                            "user_asserted"
                        }
                        .to_owned()
                    } else {
                        String::new()
                    },
                    a.unknown_reason.clone().unwrap_or_default(),
                    a.attribution
                        .as_ref()
                        .filter(|at| at.level == "repository")
                        .map(|at| at.targets.join(";"))
                        .unwrap_or_default(),
                    session
                        .and_then(|s| s.cwd.clone())
                        .filter(|_| a.attribution.as_ref().is_some_and(|at| at.level != "none"))
                        .unwrap_or_default(),
                    match (&a.attribution, a.status.as_str()) {
                        (Some(at), _) => at.state.clone(),
                        (None, "work") => "unattributed".to_owned(),
                        _ => String::new(),
                    },
                    a.engaged_session.clone().unwrap_or_default(),
                    session
                        .map(|s| s.native_session_id.clone())
                        .unwrap_or_default(),
                    session.and_then(|s| s.label.clone()).unwrap_or_default(),
                    session.and_then(|s| s.cwd.clone()).unwrap_or_default(),
                    session
                        .and_then(|s| s.git_branch.clone())
                        .unwrap_or_default(),
                    a.start_boundary.clone().unwrap_or_default(),
                    a.end_boundary.clone().unwrap_or_default(),
                    a.reasons.join(";"),
                    as_of.clone(),
                ],
            );
        }
    }
    Ok(out)
}

fn days_in(reader: &Reader, from: &str, to: &str) -> Result<(Vec<String>, ReportingZone)> {
    let parse = |d: &str| -> Result<Date> {
        d.parse()
            .map_err(|e| anyhow::anyhow!("{d}: day must be YYYY-MM-DD: {e}"))
    };
    let (from_d, to_d) = (parse(from)?, parse(to)?);
    if to_d < from_d {
        bail!("`to` ({to}) is before `from` ({from})");
    }
    let zone = reader.reporting_zone()?;
    let mut days: Vec<String> = reader
        .days()?
        .days
        .into_iter()
        .filter(|d| d.parse::<Date>().is_ok_and(|x| x >= from_d && x <= to_d))
        .collect();
    days.sort();
    Ok((days, zone))
}

// ---------------------------------------------------------------------------
// Everything: one CSV per derived table
// ---------------------------------------------------------------------------

/// Derived tables exported by "export everything", with the file name used.
pub const EVERYTHING: &[(&str, &str)] = &[
    ("agent_session", "sessions.csv"),
    ("agent_actor", "agents.csv"),
    ("agent_segment", "agent_execution_segments.csv"),
    ("turn", "turns.csv"),
    ("tool_call", "tool_calls.csv"),
    ("usage_record", "token_usage.csv"),
    ("engagement_instant", "human_prompts.csv"),
    ("human_atom", "human_time.csv"),
    ("human_atom_context", "human_time_contexts.csv"),
    ("human_proposal", "human_time_proposals.csv"),
    ("device_interval", "device.csv"),
    ("coverage_interval", "coverage.csv"),
    ("norm_event", "events.csv"),
];

pub const EVERYTHING_README: &str = "tracr export: everything (derived data)

One CSV per data set, UTF-8, RFC 4180, with a header row. IDs link the files:
e.g. agents.csv session_id -> sessions.csv id. Every *_at column is epoch
milliseconds (UTC); the *_at_local column next to it is the same instant in
the reporting timezone. An empty cell means unknown or not applicable,
never zero.

Human time (human_time.csv): work_status 'work' rows are counted
(counting_basis 'inferred' = provisional until reviewed); 'unknown' rows are
visible but never counted. Blank time has no row. Agent execution
(agent_execution_segments.csv) is agent time, never human time.

token_usage.csv: rows with canonical = 0 are copies of another stream's
occurrence (duplicate_of); count only canonical = 1 rows. There is no cost
column: estimated API cost is not derived yet.

No prompt, message or tool content exists in tracr, so none is exported.
Column meanings: docs/export.md and docs/architecture.md section 3.4.
";

/// Writes every derived table as CSV into `dir`. Returns (file, rows).
pub fn export_everything(
    conn: &rusqlite::Connection,
    zone: &ReportingZone,
    dir: &std::path::Path,
) -> Result<Vec<(String, usize)>> {
    std::fs::create_dir_all(dir)?;
    let mut written = Vec::new();
    for (table, file) in EVERYTHING {
        let cols: Vec<String> = conn
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|c| c != "derivation_run_id")
            .collect();
        let mut header: Vec<String> = Vec::new();
        for c in &cols {
            header.push(c.clone());
            if c.ends_with("_at") {
                header.push(format!("{c}_local"));
            }
        }
        let mut out = String::new();
        row(&mut out, &header);
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM {table} ORDER BY 1",
            cols.join(", ")
        ))?;
        let mut rows = stmt.query([])?;
        let mut n = 0;
        while let Some(r) = rows.next()? {
            let mut values = Vec::with_capacity(header.len());
            for (i, c) in cols.iter().enumerate() {
                let v: rusqlite::types::Value = r.get(i)?;
                let text = match &v {
                    rusqlite::types::Value::Null => String::new(),
                    rusqlite::types::Value::Integer(x) => x.to_string(),
                    rusqlite::types::Value::Real(x) => x.to_string(),
                    rusqlite::types::Value::Text(t) => t.clone(),
                    rusqlite::types::Value::Blob(b) => {
                        b.iter().map(|x| format!("{x:02x}")).collect()
                    }
                };
                values.push(text);
                if c.ends_with("_at") {
                    values.push(match v {
                        rusqlite::types::Value::Integer(ms) => local_iso(zone, ms),
                        _ => String::new(),
                    });
                }
            }
            row(&mut out, &values);
            n += 1;
        }
        std::fs::write(dir.join(file), out)?;
        written.push(((*file).to_owned(), n));
    }
    std::fs::write(dir.join("README.txt"), EVERYTHING_README)?;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_quoted_per_rfc4180() {
        let mut s = String::new();
        row(
            &mut s,
            &["a".into(), "b,c".into(), "say \"hi\"".into(), String::new()],
        );
        assert_eq!(s, "a,\"b,c\",\"say \"\"hi\"\"\",\n");
    }
}
