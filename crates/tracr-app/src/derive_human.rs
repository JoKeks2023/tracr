//! Coverage, device lane, human timeline and counting inside a rebuild
//! (architecture §4.7, §4.8, §7.1–§7.4).
//!
//! The timeline is derived over the whole history at once and then split at
//! local midnight, so a window crossing midnight is classified the same on
//! both days (TW-06) without a day-span search. Counting has no user records
//! yet (S2): every candidate is an atom decided by inference.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use tracr_core::counting::{Atom, Correction, ManualEntry, TargetRef, count};
use tracr_core::coverage::{CoverageInterval, CoverageState, Run, coverage_for_key};
use tracr_core::device::{DeviceEvent, DeviceFacts, device_lane};
use tracr_core::evidence::{CollectorFacts, Facts, GitFacts};
use tracr_core::human::{
    Candidate, Evidence, HumanParams, Instant, WorkBasis, WorkStatus, counted, human_timeline,
    unknown,
};
use tracr_core::ids::derived_id;
use tracr_core::time::UtcMillis;
use tracr_ingest::source::OS_SENSOR_KIND;
use tracr_store::derived::{AtomRow, DerivedWriter, RunRow};
use tracr_store::ledger;

use crate::day::ReportingZone;

#[derive(Debug, Clone, Default, Serialize)]
pub struct HumanReport {
    pub collector_runs: u64,
    pub device_transitions: u64,
    pub device_intervals: u64,
    pub coverage_intervals: u64,
    pub atoms: u64,
    pub counted_minutes: f64,
    pub unknown_minutes: f64,
}

/// Inputs gathered while deriving agent timelines.
pub struct AgentContext {
    pub instants: Vec<Instant>,
    /// (session ID, at, cwd) of every engagement instant (event-time context).
    pub instant_cwds: Vec<(String, i64, Option<String>)>,
    /// (session ID, start, end) of every execution segment.
    pub segments: Vec<(String, i64, i64)>,
    /// (session ID, first evidence, cwd) of every session.
    pub sessions: Vec<(String, Option<i64>, Option<String>)>,
}

/// A counted atom as written: the candidate fields (status, engaged session,
/// boundaries, reasons, proposals) plus what corrections decided.
#[derive(Clone)]
struct Piece {
    cand: Candidate,
    /// Status of the derivation underneath (`None` = blank).
    candidate_status: Option<WorkStatus>,
    decided_by: Option<String>,
    target: Option<(TargetRef, String)>,
    /// Covering manual entries (`id@rev`) and their targets.
    manual: Vec<String>,
    manual_targets: BTreeSet<TargetRef>,
}

impl Piece {
    fn from_atom(a: &Atom) -> Self {
        let mut cand = a.candidate.clone().unwrap_or(Candidate {
            start: a.start,
            end: a.end,
            status: a.status,
            engaged: None,
            reasons: Vec::new(),
            start_boundary: "user",
            end_boundary: "user",
            proposals: Vec::new(),
        });
        if a.candidate.as_ref().is_some_and(|c| c.start != a.start) {
            cand.start_boundary = "user";
        }
        if a.candidate.as_ref().is_some_and(|c| c.end != a.end) {
            cand.end_boundary = "user";
        }
        cand.start = a.start;
        cand.end = a.end;
        cand.status = a.status;
        cand.proposals.retain(|(t, _)| *t >= a.start && *t < a.end);
        if a.decided_by.is_some() {
            cand.reasons.push("user_correction");
        } else if !a.manual.is_empty() {
            cand.reasons.push("manual_entry");
        }
        Self {
            cand,
            candidate_status: a.candidate.as_ref().map(|c| c.status),
            decided_by: a.decided_by.clone(),
            target: a.target.clone(),
            manual: a.manual.clone(),
            manual_targets: a.manual_targets.clone(),
        }
    }
}

fn status_text(
    s: WorkStatus,
) -> (
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    &'static str,
) {
    match s {
        WorkStatus::Work(WorkBasis::InferredContinuity) => {
            ("work", Some("inferred"), None, "inferred_continuity")
        }
        WorkStatus::Work(WorkBasis::TimerMeasured) => {
            ("work", Some("user_asserted"), None, "timer_measured")
        }
        WorkStatus::Work(WorkBasis::UserAsserted) => {
            ("work", Some("user_asserted"), None, "user_asserted")
        }
        WorkStatus::Unknown(r) => ("unknown", None, Some(r.as_str()), "none"),
        WorkStatus::NonWork => ("non_work", None, None, "none"),
    }
}

/// Git context per working directory over time (§9): `(observed_at, facts)`.
struct GitMap(BTreeMap<String, Vec<(i64, GitFacts)>>);

impl GitMap {
    fn load(ledger: &Connection) -> Result<Self> {
        let mut m: BTreeMap<String, Vec<(i64, GitFacts)>> = BTreeMap::new();
        for (_, at, json) in ledger::git_contexts(ledger)? {
            if let Some(g) = serde_json::from_str::<Facts>(&json)
                .ok()
                .and_then(|f| f.git)
            {
                m.entry(g.cwd.clone()).or_default().push((at, g));
            }
        }
        Ok(Self(m))
    }

    /// The repository context of `cwd` as of `at`: the latest probe at or
    /// before it, else the earliest after it (identity rarely changes).
    fn resolve(&self, cwd: &str, at: i64) -> Option<&GitFacts> {
        let probes = self.0.get(cwd)?;
        probes
            .iter()
            .rev()
            .find(|(t, _)| *t <= at)
            .or_else(|| probes.first())
            .map(|(_, g)| g)
            .filter(|g| g.is_repo)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn derive(
    ledger: &Connection,
    w: &DerivedWriter<'_>,
    zone: &ReportingZone,
    as_of: UtcMillis,
    obs_watermark: i64,
    agent_kinds: &[String],
    agents: &AgentContext,
) -> Result<HumanReport> {
    let params = HumanParams::default();
    let rule_ref = params.rule_ref();
    let machine = ledger::machine_id(ledger)?
        .map(|m| m.to_string())
        .unwrap_or_default();
    let mut report = HumanReport::default();

    // Runs, device transitions and completeness gaps from the ledger.
    let mut runs: BTreeMap<String, Run> = BTreeMap::new();
    let mut device_rows: Vec<(String, DeviceFacts)> = Vec::new();
    let mut blocked: Vec<(UtcMillis, UtcMillis)> = Vec::new();
    // Completeness evidence (§4.7).
    let mut gaps: Vec<(String, UtcMillis, UtcMillis, String)> = Vec::new();
    let mut verified: BTreeMap<String, i64> = BTreeMap::new();
    let mut sweeps: Vec<(i64, BTreeSet<String>)> = Vec::new();
    let mut ended: BTreeSet<String> = BTreeSet::new();
    let mut last_discovery: Option<i64> = None;
    let mut last_row: Option<String> = None;
    for r in ledger::collector_and_sensor_rows(ledger)? {
        if last_row.as_deref() == Some(r.id.as_str()) {
            continue; // one fact row per observation is enough here
        }
        last_row = Some(r.id.clone());
        let Ok(facts) = serde_json::from_str::<Facts>(&r.facts_json) else {
            continue;
        };
        let at = r.occurred_at.unwrap_or(r.observed_at);
        if let Some(d) = facts.device {
            device_rows.push((r.collector_run_id.clone(), d));
            continue;
        }
        match facts.collector {
            Some(CollectorFacts::CoverageStarted { keys, .. }) => {
                runs.insert(
                    r.id.clone(),
                    Run {
                        started_at: at,
                        keys,
                        heartbeats: Vec::new(),
                        ended_at: None,
                    },
                );
            }
            Some(CollectorFacts::CollectorHeartbeat { keys }) => {
                if let Some(run) = runs.get_mut(&r.collector_run_id) {
                    run.heartbeats.push((at, keys));
                }
            }
            Some(CollectorFacts::CoverageEnded { .. }) => {
                if let Some(run) = runs.get_mut(&r.collector_run_id) {
                    run.ended_at = Some(at);
                }
            }
            Some(CollectorFacts::CoverageGap {
                source_id,
                start,
                end,
                reason,
            }) if end > start => {
                blocked.push((start, end));
                gaps.push((source_id, start, end, reason));
            }
            Some(CollectorFacts::ReconcileRun {
                source_id,
                reached_eof: true,
                error: None,
                ..
            }) => {
                let v = verified.entry(source_id).or_default();
                *v = (*v).max(at.0);
            }
            Some(CollectorFacts::SweepCompleted {
                stat_time,
                changed_sources,
                ..
            }) => sweeps.push((stat_time.0, changed_sources.into_iter().collect())),
            Some(CollectorFacts::SourceEnded { source_id, .. }) => {
                ended.insert(source_id);
            }
            Some(CollectorFacts::DiscoveryCompleted { .. }) => {
                last_discovery = Some(last_discovery.map_or(at.0, |d: i64| d.max(at.0)));
            }
            _ => {}
        }
    }
    let degraded = ledger::degraded_extents(ledger)?;
    for (_, lo, hi) in &degraded {
        blocked.push((*lo, *hi));
    }
    // A transition belongs to the run that recorded it and applies from that
    // run's start at the earliest: a run's first sample places the initial
    // state at the last input, which usually lies before the run began.
    let mut transitions: Vec<DeviceFacts> = device_rows
        .into_iter()
        .map(|(run, mut d)| {
            if let Some(r) = runs.get(&run) {
                d.at = d.at.max(r.started_at);
                d.sample_at = d.sample_at.max(r.started_at);
            }
            d
        })
        .collect();
    transitions.sort_by_key(|t| t.at);
    let runs: Vec<Run> = runs.into_values().collect();
    report.collector_runs = runs.len() as u64;
    report.device_transitions = transitions.len() as u64;

    let mut suspends: Vec<(UtcMillis, UtcMillis)> = Vec::new();
    let mut open: Option<UtcMillis> = None;
    for t in &transitions {
        match t.event {
            DeviceEvent::Suspended => open = Some(t.at),
            DeviceEvent::Resumed => {
                if let Some(s) = open.take()
                    && t.at >= s
                {
                    suspends.push((s, t.at));
                }
            }
            _ => {}
        }
    }

    // Coverage per key: the OS sensor and every configured agent source kind.
    let mut keys: Vec<String> = agent_kinds.to_vec();
    keys.push(OS_SENSOR_KIND.to_owned());
    let coverage: BTreeMap<String, Vec<CoverageInterval>> = keys
        .iter()
        .map(|k| (k.clone(), coverage_for_key(k, &runs, &suspends, as_of)))
        .collect();
    let sensor_collecting: Vec<(UtcMillis, UtcMillis)> = coverage[OS_SENSOR_KIND]
        .iter()
        .filter(|c| c.state == CoverageState::Collecting)
        .map(|c| (c.start, c.end))
        .collect();
    let run_starts: Vec<UtcMillis> = runs.iter().map(|r| r.started_at).collect();
    let lane = device_lane(&transitions, &run_starts, &sensor_collecting, as_of);

    let run_id = derived_id(
        "run_",
        &[
            "human",
            &obs_watermark.to_string(),
            &rule_ref,
            &as_of.0.to_string(),
        ],
    );
    w.run(&RunRow {
        id: run_id.clone(),
        unit: "human".into(),
        unit_key: "all".into(),
        as_of: as_of.0,
        obs_watermark,
        versions: serde_json::json!({ "human_rule": rule_ref }).to_string(),
        input_fingerprint: obs_watermark.to_le_bytes().to_vec(),
    })?;
    for (k, intervals) in &coverage {
        for c in intervals {
            w.coverage(
                &machine,
                k,
                c.start.0,
                c.end.0,
                c.state.as_str(),
                c.reason,
                &run_id,
            )?;
            report.coverage_intervals += 1;
        }
    }
    for d in &lane {
        w.device(&machine, d.start.0, d.end.0, d.state.as_str(), &run_id)?;
        report.device_intervals += 1;
    }

    // Completeness (§4.7): gaps and degraded extents per stream, the
    // ingestion watermark, and records recovered late per day.
    let transcript_sources: Vec<(String, String)> = ledger::sources(ledger, None)?
        .into_iter()
        .filter(|s| agent_kinds.contains(&s.kind))
        .map(|s| (s.id.to_string(), s.kind))
        .collect();
    let kind_of: BTreeMap<&str, &str> = transcript_sources
        .iter()
        .map(|(id, k)| (id.as_str(), k.as_str()))
        .collect();
    for (src, start, end, reason) in &gaps {
        let kind = kind_of.get(src.as_str()).copied().unwrap_or("transcript");
        w.stream_extent(
            &machine,
            kind,
            src,
            start.0,
            end.0,
            "missing",
            Some("gap"),
            Some(reason),
            &run_id,
        )?;
    }
    for (src, lo, hi) in &degraded {
        let kind = kind_of.get(src.as_str()).copied().unwrap_or("transcript");
        w.stream_extent(
            &machine,
            kind,
            src,
            lo.0,
            hi.0,
            "degraded",
            None,
            Some("unreadable_record"),
            &run_id,
        )?;
    }
    for (stat_time, changed) in &sweeps {
        for (src, _) in &transcript_sources {
            if !changed.contains(src) {
                let v = verified.entry(src.clone()).or_default();
                *v = (*v).max(*stat_time);
            }
        }
    }
    let watermark = transcript_sources
        .iter()
        .filter(|(src, _)| !ended.contains(src))
        .map(|(src, _)| verified.get(src).copied().unwrap_or(0))
        .chain(last_discovery)
        .min();
    if let Some(wm) = watermark.filter(|_| last_discovery.is_some()) {
        w.meta("ingestion_watermark", &wm.to_string())?;
    }
    if let Some(first_run) = run_starts.iter().min() {
        let mut per_day: BTreeMap<String, i64> = BTreeMap::new();
        for t in ledger::recovered_record_times(ledger, first_run.0)? {
            if let Some(d) = zone.day_of(t) {
                *per_day.entry(d.to_string()).or_default() += 1;
            }
        }
        for (day, n) in per_day {
            w.day_quality(&day, n, &run_id)?;
        }
    }

    // Human timeline over all sessions at once.
    let evidence = Evidence {
        device: &lane,
        coverage: coverage
            .iter()
            .map(|(k, v)| (k.clone(), v.as_slice()))
            .collect(),
        blocked: &blocked,
    };
    let candidates = human_timeline(&agents.instants, &evidence, &params);
    report.counted_minutes = counted(&candidates).0 as f64 / 60_000.0;
    report.unknown_minutes = unknown(&candidates).0 as f64 / 60_000.0;

    // Counting (§7.4): user corrections over the candidates.
    let corrections: Vec<Correction> = ledger::corrections(ledger)?
        .iter()
        .filter_map(crate::correct::from_row)
        .collect();
    // Manual entries (current revisions, not deleted); a running timer counts up to `as_of`.
    let manual: Vec<ManualEntry> = ledger::manual_entries(ledger)?
        .into_iter()
        .filter(|m| !m.deleted)
        .map(|m| ManualEntry {
            id: m.entry_id,
            rev: m.rev,
            start: UtcMillis(m.start_at),
            end: UtcMillis(m.end_at.unwrap_or(as_of.0).max(m.start_at)),
            timer: m.kind == "timer",
            target: m.target.as_deref().and_then(TargetRef::parse),
        })
        .collect();
    let atoms = count(&candidates, &manual, &corrections);
    let git = GitMap::load(ledger)?;
    for atom in &atoms {
        let piece = Piece::from_atom(atom);
        for day_piece in split_by_day(&piece.cand, zone) {
            let p = Piece {
                cand: day_piece,
                ..piece.clone()
            };
            let id = write_atom(w, &p, zone, &rule_ref, &run_id, agents)?;
            attribute_atom(w, &p, &id, &run_id, agents, &git)?;
            report.atoms += 1;
        }
    }
    for (session, first, cwd) in &agents.sessions {
        let repo = cwd
            .as_deref()
            .zip(*first)
            .and_then(|(c, t)| git.resolve(c, t))
            .and_then(|g| g.repository_key.clone());
        let label = repo.as_deref().map(tracr_git::repository_label);
        w.set_session_repository(session, repo.as_deref(), label.as_deref())?;
    }
    Ok(report)
}

/// Splits a candidate at local midnights; each piece keeps its status.
fn split_by_day(c: &Candidate, zone: &ReportingZone) -> Vec<Candidate> {
    let mut out = Vec::new();
    let mut start = c.start.0;
    while start < c.end.0 {
        let Some(day) = zone.day_of(start) else { break };
        let Ok((_, day_end)) = zone.bounds(day) else {
            break;
        };
        let end = day_end.min(c.end.0);
        let mut piece = c.clone();
        piece.start = UtcMillis(start);
        piece.end = UtcMillis(end);
        if start > c.start.0 {
            piece.start_boundary = "day_boundary";
        }
        if end < c.end.0 {
            piece.end_boundary = "day_boundary";
        }
        piece.proposals.retain(|(t, _)| t.0 >= start && t.0 < end);
        out.push(piece);
        start = end;
    }
    out
}

/// Structural attribution of a work atom (§7.5) from the event-time context
/// of its engaged session's instants: worktree and repository levels, then
/// the report target (repository > worktree > conflicting > unattributed).
fn attribute_atom(
    w: &DerivedWriter<'_>,
    p: &Piece,
    atom_id: &str,
    run_id: &str,
    agents: &AgentContext,
    git: &GitMap,
) -> Result<()> {
    let c = &p.cand;
    if !matches!(c.status, WorkStatus::Work(_)) {
        return Ok(()); // unknown and non-work carry no effective attribution
    }
    // Explicit level first (§7.5): a target the user set wins the report;
    // then the targets of covering manual entries (one: confirmed; several:
    // conflicting, TW-04).
    let manual_explicit = p.target.is_none() && !p.manual_targets.is_empty();
    if manual_explicit {
        let keys: Vec<String> = p.manual_targets.iter().map(TargetRef::key).collect();
        let state = if keys.len() == 1 {
            "confirmed"
        } else {
            "conflicting"
        };
        w.attribution(
            "human_atom",
            atom_id,
            "explicit",
            state,
            &serde_json::to_string(&keys)?,
            "manual",
            p.manual
                .first()
                .map(|m| format!("manual_entry:{m}"))
                .as_deref(),
            true,
            true,
            run_id,
        )?;
    }
    let explicit = p.target.as_ref();
    if let Some((t, by)) = explicit {
        let targets = serde_json::to_string(&[t.key()])?;
        w.attribution(
            "human_atom",
            atom_id,
            "explicit",
            "confirmed",
            &targets,
            "correction",
            Some(&format!("correction:{by}")),
            true,
            true,
            run_id,
        )?;
    }
    let explicit_any = explicit.is_some() || manual_explicit;
    let Some(session) = &c.engaged else {
        if !explicit_any {
            w.attribution(
                "human_atom",
                atom_id,
                "none",
                "unattributed",
                "[]",
                "none",
                Some("no engaged session and no target set"),
                true,
                true,
                run_id,
            )?;
        }
        return Ok(());
    };
    let mut repos: BTreeSet<String> = BTreeSet::new();
    let mut worktrees: BTreeSet<String> = BTreeSet::new();
    let mut unresolved = 0;
    for (s, at, cwd) in &agents.instant_cwds {
        if s != session || *at < c.start.0 || *at > c.end.0 {
            continue;
        }
        match cwd.as_deref().and_then(|d| git.resolve(d, *at)) {
            Some(g) => {
                repos.extend(g.repository_key.clone());
                worktrees.extend(g.toplevel.clone());
            }
            None => unresolved += 1,
        }
    }
    let evidence = (unresolved > 0).then(|| format!("{unresolved} instant(s) without git context"));
    let state = |n: usize| if n == 1 { "confirmed" } else { "conflicting" };
    let json = |set: &BTreeSet<String>| serde_json::to_string(set).expect("json");
    let report = if explicit_any {
        "explicit"
    } else if !repos.is_empty() {
        "repository"
    } else if !worktrees.is_empty() {
        "worktree"
    } else {
        "none"
    };
    if !worktrees.is_empty() {
        w.attribution(
            "human_atom",
            atom_id,
            "worktree",
            state(worktrees.len()),
            &json(&worktrees),
            "observation",
            evidence.as_deref(),
            true,
            report == "worktree",
            run_id,
        )?;
    }
    if !repos.is_empty() {
        w.attribution(
            "human_atom",
            atom_id,
            "repository",
            state(repos.len()),
            &json(&repos),
            "observation",
            evidence.as_deref(),
            true,
            report == "repository",
            run_id,
        )?;
    }
    if report == "none" {
        w.attribution(
            "human_atom",
            atom_id,
            "none",
            "unattributed",
            "[]",
            "none",
            Some(evidence.as_deref().unwrap_or("no candidate")),
            true,
            true,
            run_id,
        )?;
    }
    Ok(())
}

fn write_atom(
    w: &DerivedWriter<'_>,
    p: &Piece,
    zone: &ReportingZone,
    rule_ref: &str,
    run_id: &str,
    agents: &AgentContext,
) -> Result<String> {
    let c = &p.cand;
    let day = zone
        .day_of(c.start.0)
        .map(|d| d.to_string())
        .unwrap_or_default();
    let (status, counting, reason, duration) = status_text(c.status);
    let (cand_status, cand_reason, cand_basis) = match p.candidate_status {
        Some(s) => {
            let (st, _, r, b) = status_text(s);
            (st, r, b)
        }
        None => ("blank", None, "none"),
    };
    let decided_by = match (&p.decided_by, p.manual.first()) {
        (Some(id), _) => format!("correction:{id}"),
        (None, Some(m)) => format!("manual_entry:{m}"),
        (None, None) => "inference".to_owned(),
    };
    let target_key = p.target.as_ref().map(|t| t.0.key()).unwrap_or_default();
    let id = derived_id(
        "atm_",
        &[
            rule_ref,
            &day,
            &c.start.0.to_string(),
            &c.end.0.to_string(),
            status,
            c.engaged.as_deref().unwrap_or(""),
            &decided_by,
            &target_key,
        ],
    );
    w.atom(&AtomRow {
        id: id.clone(),
        day,
        start_at: c.start.0,
        end_at: c.end.0,
        work_status: status.into(),
        counting_basis: counting.map(str::to_owned),
        unknown_reason: reason.map(str::to_owned),
        duration_basis: duration.into(),
        // The derivation underneath stays on the atom ("overridden by …").
        candidate_status: Some(cand_status.into()),
        candidate_reason: cand_reason.map(str::to_owned),
        candidate_basis: Some(cand_basis.into()),
        start_boundary: Some(c.start_boundary.into()),
        end_boundary: Some(c.end_boundary.into()),
        reasons: Some(serde_json::to_string(&c.reasons)?),
        decided_by,
        rule_ref: rule_ref.into(),
        run: run_id.into(),
    })?;
    if let Some(session) = &c.engaged {
        w.atom_context(&id, "engaged", "agent_session", session)?;
    }
    // Agents running during the atom: informational only, never attribution.
    for (session, s, e) in &agents.segments {
        if *s < c.end.0 && *e > c.start.0 {
            w.atom_context(&id, "concurrent_agent", "agent_session", session)?;
        }
    }
    for (at, kind) in &c.proposals {
        w.proposal(&id, at.0, kind)?;
    }
    Ok(id)
}
