//! Day queries over the derived store: which days have data, and one
//! reporting day's lanes (architecture §13.4, AD-14, AD-16).
//!
//! Sessions are listed individually; their segments overlap freely and are
//! never merged across actors or sessions. Agent time is never human time.
//! Human time comes only from human atoms: counted (work) and unknown
//! minutes are separate fields and are never added together (INV-2). Cost
//! is listed as not derived yet (S1.5), never as zero.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, anyhow};
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use rusqlite::Connection;
use serde::Serialize;
use tracr_store::derived;

/// The reporting timezone (AD-14): the `timezone` setting if one was
/// chosen, else the system zone.
#[derive(Debug, Clone)]
pub struct ReportingZone {
    pub tz: TimeZone,
    pub name: String,
    pub from_setting: bool,
}

impl ReportingZone {
    pub fn resolve(setting: Option<&str>) -> Result<Self> {
        if let Some(name) = setting {
            let tz = TimeZone::get(name).with_context(|| format!("timezone setting {name:?}"))?;
            return Ok(Self {
                tz,
                name: name.to_owned(),
                from_setting: true,
            });
        }
        let tz = TimeZone::system();
        let name = tz.iana_name().unwrap_or("system").to_owned();
        Ok(Self {
            tz,
            name,
            from_setting: false,
        })
    }

    pub fn day_of(&self, ms: i64) -> Option<Date> {
        Timestamp::from_millisecond(ms)
            .ok()
            .map(|t| t.to_zoned(self.tz.clone()).date())
    }

    /// `[start, end)` of a local day in epoch ms (23 or 25 hours across DST).
    pub fn bounds(&self, day: Date) -> Result<(i64, i64)> {
        let start = day.to_zoned(self.tz.clone())?.start_of_day()?;
        let end = day.tomorrow()?.to_zoned(self.tz.clone())?.start_of_day()?;
        Ok((
            start.timestamp().as_millisecond(),
            end.timestamp().as_millisecond(),
        ))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DayList {
    pub timezone: String,
    pub timezone_from_setting: bool,
    /// Newest first.
    pub days: Vec<String>,
    pub derived_as_of: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    pub start: i64,
    pub end: i64,
    pub start_basis: String,
    pub end_basis: String,
    /// The segment continues before the day starts / after it ends.
    pub clipped_start: bool,
    pub clipped_end: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActorDay {
    pub actor_id: String,
    /// `known | main | unknown` (AD-21: `unknown` is the unidentified bucket, not an identity).
    pub actor_kind: String,
    pub native_agent_id: Option<String>,
    pub segments: Vec<Segment>,
    /// Union of this actor's segments within the day. Agent time, never human time.
    pub execution_ms: i64,
    pub turns: i64,
    pub tool_calls: i64,
}

/// A token category: the sum over records that state it, and how many
/// records do not state it (unknown, never zero).
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct TokenCount {
    pub sum: i64,
    pub unknown_records: i64,
}

impl From<(i64, i64)> for TokenCount {
    fn from((sum, unknown_records): (i64, i64)) -> Self {
        Self {
            sum,
            unknown_records,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageLine {
    pub actor_id: Option<String>,
    pub model: Option<String>,
    /// Unidentified usage is its own line, never merged (AD-17).
    pub identified: bool,
    pub records: i64,
    pub input_fresh: TokenCount,
    pub cache_creation: TokenCount,
    pub cache_read: TokenCount,
    pub output: TokenCount,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDay {
    pub session_id: String,
    pub provider: String,
    pub native_session_id: String,
    pub label: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub first_evidence_at: Option<i64>,
    pub last_evidence_at: Option<i64>,
    pub diagnostics: BTreeMap<String, u64>,
    /// Human prompts in this session on this day. Instants have no duration.
    pub engagement_instants: Vec<i64>,
    pub actors: Vec<ActorDay>,
    pub usage: Vec<UsageLine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingLane {
    pub lane: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Proposal {
    pub at: i64,
    pub kind: String,
}

/// One human atom within the day.
#[derive(Debug, Clone, Serialize)]
pub struct HumanAtom {
    pub start: i64,
    pub end: i64,
    /// `work`, `unknown` or `non_work`.
    pub status: String,
    /// For `unknown`: `coverage_gap`, `conflicting_evidence` or `unresolved_window`.
    pub unknown_reason: Option<String>,
    /// `inferred_continuity`, `user_asserted`, `timer_measured` or `none`.
    pub duration_basis: String,
    /// The session whose prompts bound inferred work (evidence, not attribution).
    pub engaged_session: Option<String>,
    pub start_boundary: Option<String>,
    pub end_boundary: Option<String>,
    pub reasons: Vec<String>,
    pub proposals: Vec<Proposal>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct HumanDay {
    pub atoms: Vec<HumanAtom>,
    /// Counted human time (inferred, provisional until reviewed).
    pub counted_ms: i64,
    /// Visible, not counted.
    pub unknown_ms: i64,
    pub unknown_by_reason: BTreeMap<String, i64>,
    /// First to last human prompt of the day: an upper bound, never a total.
    pub evidence_span: Option<(i64, i64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaneInterval {
    pub start: i64,
    pub end: i64,
    pub state: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayAgents {
    pub day: String,
    pub timezone: String,
    pub start: i64,
    pub end: i64,
    pub derived_as_of: Option<i64>,
    pub sessions: Vec<SessionDay>,
    pub human: HumanDay,
    pub device: Vec<LaneInterval>,
    /// Coverage per source kind (e.g. `claude_transcript`, `os_sensor`).
    pub coverage: BTreeMap<String, Vec<LaneInterval>>,
    /// Lanes that exist in the product but are not derived yet. Shown as
    /// such, never as zero or as an empty lane.
    pub pending: Vec<PendingLane>,
}

pub const PENDING_LANES: &[PendingLane] = &[PendingLane {
    lane: "cost",
    reason: "pricing catalog and API-equivalent estimates arrive with S1.5",
}];

fn union_ms(mut spans: Vec<(i64, i64)>) -> i64 {
    spans.sort_unstable();
    let mut total = 0;
    let mut cur: Option<(i64, i64)> = None;
    for (s, e) in spans {
        match cur {
            Some((cs, ce)) if s <= ce => cur = Some((cs, ce.max(e))),
            Some((cs, ce)) => {
                total += ce - cs;
                cur = Some((s, e));
            }
            None => cur = Some((s, e)),
        }
    }
    total + cur.map(|(s, e)| e - s).unwrap_or(0)
}

fn derived_as_of(conn: &Connection) -> Result<Option<i64>> {
    Ok(derived::meta(conn, "as_of")?.and_then(|v| v.parse().ok()))
}

pub fn days(conn: &Connection, zone: &ReportingZone) -> Result<DayList> {
    let mut days: BTreeSet<Date> = BTreeSet::new();
    for t in derived::activity_instants(conn)? {
        if let Some(d) = zone.day_of(t.0) {
            days.insert(d);
        }
    }
    Ok(DayList {
        timezone: zone.name.clone(),
        timezone_from_setting: zone.from_setting,
        days: days.iter().rev().map(|d| d.to_string()).collect(),
        derived_as_of: derived_as_of(conn)?,
    })
}

pub fn day_agents(conn: &Connection, zone: &ReportingZone, day: &str) -> Result<DayAgents> {
    let date: Date = day
        .parse()
        .map_err(|e| anyhow!("day must be YYYY-MM-DD: {e}"))?;
    let (start, end) = zone.bounds(date)?;
    let mut sessions = Vec::new();
    for s in derived::sessions_active_in(conn, start, end)? {
        let counts: BTreeMap<Option<String>, (i64, i64)> =
            derived::activity_counts_in(conn, &s.id, start, end)?
                .into_iter()
                .map(|(a, t, c)| (a, (t, c)))
                .collect();
        let segments = derived::segments_in(conn, &s.id, start, end)?;
        let mut actors = Vec::new();
        for a in derived::actors_of(conn, &s.id)? {
            let mine: Vec<Segment> = segments
                .iter()
                .filter(|g| g.actor_id == a.id)
                .map(|g| Segment {
                    start: g.start_at.max(start),
                    end: g.end_at.min(end),
                    start_basis: g.start_basis.clone(),
                    end_basis: g.end_basis.clone(),
                    clipped_start: g.start_at < start,
                    clipped_end: g.end_at > end,
                })
                .collect();
            let (turns, tool_calls) = counts.get(&Some(a.id.clone())).copied().unwrap_or((0, 0));
            if mine.is_empty() && turns == 0 && tool_calls == 0 {
                continue; // not active on this day
            }
            actors.push(ActorDay {
                execution_ms: union_ms(mine.iter().map(|g| (g.start, g.end)).collect()),
                actor_id: a.id,
                actor_kind: a.actor_kind,
                native_agent_id: a.native_agent_id,
                segments: mine,
                turns,
                tool_calls,
            });
        }
        let usage = derived::usage_sums_in(conn, &s.id, start, end)?
            .into_iter()
            .map(|u| UsageLine {
                actor_id: u.actor_id,
                model: u.model,
                identified: u.identified,
                records: u.records,
                input_fresh: u.input_fresh.into(),
                cache_creation: u.cache_creation.into(),
                cache_read: u.cache_read.into(),
                output: u.output.into(),
            })
            .collect();
        sessions.push(SessionDay {
            engagement_instants: derived::instants_in(conn, &s.id, start, end)?
                .into_iter()
                .map(|i| i.at)
                .collect(),
            diagnostics: s
                .diagnostics
                .as_deref()
                .and_then(|d| serde_json::from_str(d).ok())
                .unwrap_or_default(),
            session_id: s.id,
            provider: s.provider,
            native_session_id: s.native_session_id,
            label: s.label,
            cwd: s.cwd,
            git_branch: s.git_branch,
            first_evidence_at: s.first_evidence_at,
            last_evidence_at: s.last_evidence_at,
            actors,
            usage,
        });
    }
    let mut human = HumanDay::default();
    for (a, engaged, proposals) in derived::atoms_of_day(conn, &date.to_string())? {
        let len = a.end_at - a.start_at;
        match a.work_status.as_str() {
            "work" => human.counted_ms += len,
            "unknown" => {
                human.unknown_ms += len;
                *human
                    .unknown_by_reason
                    .entry(a.unknown_reason.clone().unwrap_or_default())
                    .or_default() += len;
            }
            _ => {}
        }
        human.atoms.push(HumanAtom {
            start: a.start_at,
            end: a.end_at,
            status: a.work_status,
            unknown_reason: a.unknown_reason,
            duration_basis: a.duration_basis,
            engaged_session: engaged,
            start_boundary: a.start_boundary,
            end_boundary: a.end_boundary,
            reasons: a
                .reasons
                .as_deref()
                .and_then(|r| serde_json::from_str(r).ok())
                .unwrap_or_default(),
            proposals: proposals
                .into_iter()
                .map(|(at, kind)| Proposal { at, kind })
                .collect(),
        });
    }
    let instants: Vec<i64> = sessions
        .iter()
        .flat_map(|s: &SessionDay| s.engagement_instants.iter().copied())
        .collect();
    human.evidence_span = instants
        .iter()
        .min()
        .zip(instants.iter().max())
        .map(|(a, b)| (*a, *b));
    let lane = |r: derived::IntervalRow| LaneInterval {
        start: r.start_at,
        end: r.end_at,
        state: r.state,
        reason: r.reason,
    };
    let mut coverage: BTreeMap<String, Vec<LaneInterval>> = BTreeMap::new();
    for r in derived::coverage_in(conn, start, end)? {
        coverage.entry(r.kind.clone()).or_default().push(lane(r));
    }
    Ok(DayAgents {
        human,
        device: derived::device_in(conn, start, end)?
            .into_iter()
            .map(lane)
            .collect(),
        coverage,
        day: date.to_string(),
        timezone: zone.name.clone(),
        start,
        end,
        derived_as_of: derived_as_of(conn)?,
        sessions,
        pending: PENDING_LANES.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_counts_overlap_once() {
        assert_eq!(union_ms(vec![(0, 10), (5, 15), (20, 30)]), 25);
        assert_eq!(union_ms(vec![]), 0);
    }

    #[test]
    fn dst_days_have_23_and_25_hours() {
        let z = ReportingZone::resolve(Some("Europe/Berlin")).unwrap();
        let (s, e) = z.bounds("2026-03-29".parse().unwrap()).unwrap();
        assert_eq!(e - s, 23 * 3_600_000);
        let (s, e) = z.bounds("2026-10-25".parse().unwrap()).unwrap();
        assert_eq!(e - s, 25 * 3_600_000);
    }
}
