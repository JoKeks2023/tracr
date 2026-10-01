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
    /// API-equivalent estimate of these records, never spend.
    pub cost: CostView,
}

/// Estimates of a group of usage records under one pricing revision
/// (architecture §8.2). `amount_nano` covers the priced parts only and is
/// `None` when nothing was priced: unpriced, never zero.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CostView {
    pub amount_nano: Option<i64>,
    pub priced_records: i64,
    pub partially_priced_records: i64,
    pub unpriced_records: i64,
    pub not_estimated_records: i64,
    pub priced_tokens: i64,
    pub unpriced_tokens: i64,
    pub reasons: Vec<String>,
    pub assumptions: Vec<String>,
}

impl From<derived::CostSum> for CostView {
    fn from(c: derived::CostSum) -> Self {
        Self {
            amount_nano: c.amount_nano,
            priced_records: c.priced_records,
            partially_priced_records: c.partially_priced_records,
            unpriced_records: c.unpriced_records,
            not_estimated_records: c.not_estimated_records,
            priced_tokens: c.priced_tokens,
            unpriced_tokens: c.unpriced_tokens,
            reasons: c.reasons,
            assumptions: c.assumptions,
        }
    }
}

impl CostView {
    /// `priced`, `partially_priced` or `unpriced` for the group; `None`
    /// without records.
    pub fn status(&self) -> Option<&'static str> {
        let all = self.priced_records
            + self.partially_priced_records
            + self.unpriced_records
            + self.not_estimated_records;
        if all == 0 {
            None
        } else if self.amount_nano.is_none() {
            Some("unpriced")
        } else if self.priced_records == all {
            Some("priced")
        } else {
            Some("partially_priced")
        }
    }

    /// Adds estimates of one kind and one pricing revision. Unpriced parts
    /// stay unpriced; nothing is added across kinds.
    pub fn absorb(&mut self, o: &CostView) {
        self.amount_nano = match (self.amount_nano, o.amount_nano) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
        };
        self.priced_records += o.priced_records;
        self.partially_priced_records += o.partially_priced_records;
        self.unpriced_records += o.unpriced_records;
        self.not_estimated_records += o.not_estimated_records;
        self.priced_tokens += o.priced_tokens;
        self.unpriced_tokens += o.unpriced_tokens;
        for r in &o.reasons {
            if !self.reasons.contains(r) {
                self.reasons.push(r.clone());
            }
        }
        for a in &o.assumptions {
            if !self.assumptions.contains(a) {
                self.assumptions.push(a.clone());
            }
        }
    }
}

/// A cost figure the provider displays itself: its own running total, never
/// summed, not spend. Its scope is the provider's: Claude Code's figure was
/// observed to carry over into resumed sessions and to restart in others, so
/// it is not "this session's cost".
#[derive(Debug, Clone, Serialize)]
pub struct ReportedView {
    pub source_field: String,
    pub value_as_stated: String,
    pub value_nano: Option<i64>,
    pub currency: String,
    pub semantics: String,
    pub scope: String,
    /// When tracr recorded the figure (the source states no time).
    pub recorded_at: i64,
    pub label: &'static str,
}

pub const REPORTED_LABEL: &str =
    "provider-reported running total (provider's own scope), not spend";

/// The day's estimated API cost: identified usage, unidentified usage on its
/// own line (AD-17), and usage spanning the day boundary (AD-16).
#[derive(Debug, Clone, Serialize)]
pub struct DayCost {
    pub pricing: crate::pricing::PricingInfo,
    pub identified: CostView,
    pub unidentified: CostView,
    pub spanning_records: i64,
    pub spanning: CostView,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDay {
    pub session_id: String,
    pub provider: String,
    pub native_session_id: String,
    pub label: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    /// Repository of the session's working directory (approved roots only).
    pub repository: Option<String>,
    pub repository_label: Option<String>,
    pub first_evidence_at: Option<i64>,
    pub last_evidence_at: Option<i64>,
    pub diagnostics: BTreeMap<String, u64>,
    /// Human prompts in this session on this day. Instants have no duration.
    pub engagement_instants: Vec<i64>,
    pub actors: Vec<ActorDay>,
    pub usage: Vec<UsageLine>,
    /// Identified usage of the session on this day.
    pub cost: CostView,
    pub reported: Vec<ReportedView>,
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
    /// Report-target attribution (work only): level, state and target labels.
    pub attribution: Option<Attribution>,
    /// The correction that decided the status, if any (else inference).
    pub decided_by: Option<String>,
    /// The derivation underneath: `work`, `unknown`, `non_work` or `blank`.
    pub candidate_status: Option<String>,
    /// The manual entry that counts this time, if any.
    pub manual_entry: Option<String>,
    pub candidate_reason: Option<String>,
}

/// A manual entry (timer or retroactive) overlapping the day.
#[derive(Debug, Clone, Serialize)]
pub struct ManualEntryInfo {
    pub id: String,
    pub rev: i64,
    /// `timer` or `retroactive`.
    pub kind: String,
    pub start: i64,
    /// `None` while the timer runs.
    pub end: Option<i64>,
    pub running: bool,
    pub target: Option<String>,
    pub target_label: Option<String>,
    pub note: Option<String>,
}

impl From<&tracr_store::ledger::ManualRev> for ManualEntryInfo {
    fn from(m: &tracr_store::ledger::ManualRev) -> Self {
        Self {
            id: m.entry_id.clone(),
            rev: m.rev,
            kind: m.kind.clone(),
            start: m.start_at,
            end: m.end_at,
            running: m.kind == "timer" && m.end_at.is_none(),
            target_label: m.target.as_deref().map(target_label),
            target: m.target.clone(),
            note: m.note.clone(),
        }
    }
}

/// One user correction overlapping the day, with its current effect.
#[derive(Debug, Clone, Serialize)]
pub struct CorrectionInfo {
    pub id: String,
    pub op: String,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub target: Option<String>,
    pub target_label: Option<String>,
    /// For `revoke`: the correction it undoes.
    pub revokes: Option<String>,
    pub at: i64,
    /// `effective`, `partly superseded`, `superseded` or `revoked`.
    pub status: String,
    pub reason: Option<String>,
}

/// A target the user can choose (dropdown, UI-01).
#[derive(Debug, Clone, Serialize)]
pub struct TargetOption {
    pub key: String,
    pub label: String,
    /// `repository`, `category` or `other`.
    pub group: String,
}

/// Display label of a target key.
pub fn target_label(key: &str) -> String {
    match tracr_core::counting::TargetRef::parse(key) {
        Some(tracr_core::counting::TargetRef::Repository(k)) => tracr_git::repository_label(&k),
        Some(tracr_core::counting::TargetRef::Category(c)) => {
            let mut ch = c.chars();
            ch.next()
                .map(|f| f.to_uppercase().chain(ch).collect())
                .unwrap_or_default()
        }
        Some(tracr_core::counting::TargetRef::Other(t)) => t,
        None => key.to_owned(),
    }
}

/// Corrections overlapping `[start, end)` with their status (AD-10: nothing
/// is replaced silently).
pub fn correction_infos(
    rows: &[tracr_store::ledger::CorrectionRow],
    start: i64,
    end: i64,
) -> Vec<CorrectionInfo> {
    use tracr_core::counting::{Correction, CorrectionOp, revoked};
    let parsed: Vec<Correction> = rows.iter().filter_map(crate::correct::from_row).collect();
    let gone = revoked(&parsed);
    let by_id: BTreeMap<&str, &Correction> = parsed.iter().map(|c| (c.id.as_str(), c)).collect();
    let range_of = |c: &Correction| -> (i64, i64) {
        match &c.op {
            CorrectionOp::Revoke(t) => by_id
                .get(t.as_str())
                .map_or((0, 0), |x| (x.start.0, x.end.0)),
            _ => (c.start.0, c.end.0),
        }
    };
    let mut out = Vec::new();
    for c in &parsed {
        let (s, e) = range_of(c);
        if !(s < end && e > start) {
            continue;
        }
        let status = if gone.contains(&c.id) {
            "revoked".to_owned()
        } else if matches!(c.op, CorrectionOp::Revoke(_)) {
            "effective".to_owned()
        } else {
            // Later live corrections on the same axis covering this range.
            let mut covers: Vec<(i64, i64)> = parsed
                .iter()
                .filter(|o| o.seq > c.seq && !gone.contains(&o.id) && o.op.axis() == c.op.axis())
                .filter(|o| o.start.0 < e && o.end.0 > s)
                .map(|o| (o.start.0.max(s), o.end.0.min(e)))
                .collect();
            covers.sort_unstable();
            let mut covered = 0;
            let mut cur: Option<(i64, i64)> = None;
            for (a, b) in covers {
                match cur {
                    Some((ca, cb)) if a <= cb => cur = Some((ca, cb.max(b))),
                    Some((ca, cb)) => {
                        covered += cb - ca;
                        cur = Some((a, b));
                    }
                    None => cur = Some((a, b)),
                }
            }
            covered += cur.map_or(0, |(a, b)| b - a);
            if covered == 0 {
                "effective"
            } else if covered >= e - s {
                "superseded"
            } else {
                "partly superseded"
            }
            .to_owned()
        };
        let row = rows.iter().find(|r| r.id == c.id);
        let target = match &c.op {
            CorrectionOp::SetTarget(t) | CorrectionOp::RejectTarget(t) => Some(t.key()),
            _ => None,
        };
        out.push(CorrectionInfo {
            id: c.id.clone(),
            op: c.op.name().to_owned(),
            start: (!matches!(c.op, CorrectionOp::Revoke(_))).then_some(c.start.0),
            end: (!matches!(c.op, CorrectionOp::Revoke(_))).then_some(c.end.0),
            target_label: target.as_deref().map(target_label),
            target,
            revokes: match &c.op {
                CorrectionOp::Revoke(t) => Some(t.clone()),
                _ => None,
            },
            at: row.map_or(0, |r| r.at.0),
            status,
            reason: row.and_then(|r| r.reason.clone()),
        });
    }
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct Attribution {
    /// `repository`, `worktree` or `none`.
    pub level: String,
    /// `confirmed`, `conflicting` or `unattributed`.
    pub state: String,
    /// Target keys (repository keys or worktree paths).
    pub targets: Vec<String>,
    /// Short labels for display (e.g. `owner/name`).
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct HumanDay {
    pub atoms: Vec<HumanAtom>,
    /// Counted human time (inferred, provisional until reviewed).
    pub counted_ms: i64,
    /// Visible, not counted.
    pub unknown_ms: i64,
    pub unknown_by_reason: BTreeMap<String, i64>,
    /// Counted time per report target label (`owner/name`, `conflicting`, `unattributed`).
    pub counted_by_target: BTreeMap<String, i64>,
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

/// Whether a day's numbers are final (reporting §3).
#[derive(Debug, Clone, Serialize)]
pub struct DayQuality {
    /// Always true until review exists (S2): drafts are unreviewed.
    pub provisional: bool,
    pub provisional_reasons: Vec<String>,
    /// Every live stream is verified past the end of the day.
    pub ingestion_complete: bool,
    /// Up to when ingestion is verified (min over live streams and discovery).
    pub ingestion_verified_through: Option<i64>,
    /// Records ingested late (startup, sweep, wake) after the collector first ran.
    pub recovered_records: i64,
    /// Completeness gaps and degraded extents of individual streams.
    pub stream_extents: Vec<LaneInterval>,
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
    pub quality: DayQuality,
    pub corrections: Vec<CorrectionInfo>,
    pub manual_entries: Vec<ManualEntryInfo>,
    /// The latest reviewed revision of the day, if reviewed.
    pub review: Option<crate::review::ReviewState>,
    /// The current derivation differs from the reviewed revision.
    pub discrepancy: Option<crate::review::Discrepancy>,
    pub cost: DayCost,
    /// Quarantined and key-conflict records observed within the day (set by the reader).
    pub ledger_counts: crate::report::LedgerCounts,
    /// Lanes that exist in the product but are not derived yet. Shown as
    /// such, never as zero or as an empty lane.
    pub pending: Vec<PendingLane>,
}

pub const PENDING_LANES: &[PendingLane] = &[];

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

pub fn day_agents(
    conn: &Connection,
    zone: &ReportingZone,
    day: &str,
    corrections: &[tracr_store::ledger::CorrectionRow],
    manual: &[tracr_store::ledger::ManualRev],
) -> Result<DayAgents> {
    let date: Date = day
        .parse()
        .map_err(|e| anyhow!("day must be YYYY-MM-DD: {e}"))?;
    let (start, end) = zone.bounds(date)?;
    let pricing = crate::pricing::bundled();
    let mut identified_cost = CostView::default();
    let mut unidentified_cost = CostView::default();
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
        let usage: Vec<UsageLine> =
            derived::usage_sums_in(conn, &s.id, start, end, &pricing.revision)?
                .into_iter()
                .map(|u| UsageLine {
                    cost: u.cost.into(),
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
        let mut cost = CostView::default();
        for u in &usage {
            if u.identified {
                cost.absorb(&u.cost);
            } else {
                unidentified_cost.absorb(&u.cost);
            }
        }
        identified_cost.absorb(&cost);
        let reported = derived::reported_costs(conn, &s.id)?
            .into_iter()
            .map(|r| ReportedView {
                source_field: r.source_field,
                value_as_stated: r.value_as_stated,
                value_nano: r.value_nano,
                currency: r.currency,
                semantics: r.semantics,
                scope: r.scope,
                recorded_at: r.observed_at,
                label: REPORTED_LABEL,
            })
            .collect();
        let (repository, repository_label) = derived::session_repository(conn, &s.id)?;
        sessions.push(SessionDay {
            cost,
            reported,
            repository,
            repository_label,
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
        let attribution =
            derived::report_attribution(conn, &a.id)?.map(|(level, state, targets)| {
                let targets: Vec<String> = serde_json::from_str(&targets).unwrap_or_default();
                let labels = targets
                    .iter()
                    .map(|t| match level.as_str() {
                        "repository" => tracr_git::repository_label(t),
                        "explicit" => target_label(t),
                        _ => t.rsplit('/').next().unwrap_or(t).to_owned(),
                    })
                    .collect();
                Attribution {
                    level,
                    state,
                    targets,
                    labels,
                }
            });
        if a.work_status == "work" {
            let key = match &attribution {
                Some(at) if at.state == "confirmed" => at.labels.join(", "),
                Some(at) => at.state.clone(),
                None => "unattributed".to_owned(),
            };
            *human.counted_by_target.entry(key).or_default() += len;
        }
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
            attribution,
            decided_by: a.decided_by.strip_prefix("correction:").map(str::to_owned),
            manual_entry: a
                .decided_by
                .strip_prefix("manual_entry:")
                .and_then(|m| m.split('@').next())
                .map(str::to_owned),
            candidate_status: a.candidate_status.clone(),
            candidate_reason: a.candidate_reason.clone(),
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
    let verified_through =
        derived::meta(conn, "ingestion_watermark")?.and_then(|v| v.parse::<i64>().ok());
    let ingestion_complete = verified_through.is_some_and(|w| w >= end);
    let stream_extents: Vec<LaneInterval> = derived::stream_extents_in(conn, start, end)?
        .into_iter()
        .map(lane)
        .collect();
    let mut provisional_reasons = vec!["not reviewed".to_owned()];
    if !ingestion_complete {
        provisional_reasons.push("ingestion not verified to the end of the day".to_owned());
    }
    if stream_extents.iter().any(|e| e.state == "gap") {
        provisional_reasons.push("unresolved ingestion gaps".to_owned());
    }
    let quality = DayQuality {
        provisional: true,
        provisional_reasons,
        ingestion_complete,
        ingestion_verified_through: verified_through,
        recovered_records: derived::recovered_records(conn, &date.to_string())?,
        stream_extents,
    };
    Ok(DayAgents {
        review: None,
        discrepancy: None,
        corrections: correction_infos(corrections, start, end),
        manual_entries: manual
            .iter()
            .filter(|m| !m.deleted && m.start_at < end && m.end_at.is_none_or(|e| e > start))
            .map(ManualEntryInfo::from)
            .collect(),
        quality,
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
        cost: {
            let (spanning_records, spanning) =
                derived::spanning_usage(conn, start, end, &pricing.revision)?;
            DayCost {
                pricing: pricing.info(),
                identified: identified_cost,
                unidentified: unidentified_cost,
                spanning_records,
                spanning: spanning.into(),
            }
        },
        ledger_counts: Default::default(),
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
