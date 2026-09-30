//! Coverage, device lane, human timeline and counting inside a rebuild
//! (architecture §4.7, §4.8, §7.1–§7.4).
//!
//! The timeline is derived over the whole history at once and then split at
//! local midnight, so a window crossing midnight is classified the same on
//! both days (TW-06) without a day-span search. Counting has no user records
//! yet (S2): every candidate is an atom decided by inference.

use std::collections::BTreeMap;

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use tracr_core::coverage::{CoverageInterval, CoverageState, Run, coverage_for_key};
use tracr_core::device::{DeviceEvent, DeviceFacts, device_lane};
use tracr_core::evidence::{CollectorFacts, Facts};
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
    /// (session ID, start, end) of every execution segment.
    pub segments: Vec<(String, i64, i64)>,
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
            Some(CollectorFacts::CoverageGap { start, end, .. }) if end > start => {
                blocked.push((start, end))
            }
            _ => {}
        }
    }
    for (_, lo, hi) in ledger::degraded_extents(ledger)? {
        blocked.push((lo, hi));
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

    for c in &candidates {
        for piece in split_by_day(c, zone) {
            write_atom(w, &piece, zone, &rule_ref, &run_id, agents)?;
            report.atoms += 1;
        }
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

fn write_atom(
    w: &DerivedWriter<'_>,
    c: &Candidate,
    zone: &ReportingZone,
    rule_ref: &str,
    run_id: &str,
    agents: &AgentContext,
) -> Result<()> {
    let day = zone
        .day_of(c.start.0)
        .map(|d| d.to_string())
        .unwrap_or_default();
    let (status, counting, reason, duration) = match c.status {
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
    };
    let id = derived_id(
        "atm_",
        &[
            rule_ref,
            &day,
            &c.start.0.to_string(),
            &c.end.0.to_string(),
            status,
            c.engaged.as_deref().unwrap_or(""),
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
        // No correction or manual entry exists yet: the candidate stands.
        candidate_status: Some(status.into()),
        candidate_reason: reason.map(str::to_owned),
        candidate_basis: Some(duration.into()),
        start_boundary: Some(c.start_boundary.into()),
        end_boundary: Some(c.end_boundary.into()),
        reasons: Some(serde_json::to_string(&c.reasons)?),
        decided_by: "inference".into(),
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
    Ok(())
}
