//! CSV export (reporting §5, pulled forward from S4 at the user's request).
//!
//! `agent-sessions` export: one row per reporting day × agent session ×
//! actor, built from the same day queries the Day view shows. Agent time is
//! never human time; a timesheet export of human time follows with the
//! human timeline (S1.4 M2).
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
