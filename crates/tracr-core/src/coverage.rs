//! Coverage per (machine, source kind) (architecture §4.7).
//!
//! From the runtime's own records: a run starts with `coverage_started`,
//! reports every key's state in heartbeats, and ends with `coverage_ended`,
//! or silently at its last heartbeat after a crash.
//!
//! * Before the first run: `unknown`.
//! * Inside a run: `collecting` for the keys it lists and reports as collecting.
//!   A heartbeat stall longer than [`STALL_MS`] is `missing` unless a device
//!   suspension explains it.
//! * Between runs, whatever their distance: `missing` (`collector_down`).
//! * The newest run is `collecting` up to `as_of` while its last heartbeat is recent.
//!
//! CLI imports assert no coverage: backfilled history stays `unknown`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::time::UtcMillis;

pub const STALL_MS: i64 = 3 * 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageState {
    Collecting,
    Missing,
    Unknown,
}

impl CoverageState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Collecting => "collecting",
            Self::Missing => "missing",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageInterval {
    pub start: UtcMillis,
    pub end: UtcMillis,
    pub state: CoverageState,
    pub reason: Option<&'static str>,
}

/// One collector run as recorded in the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub started_at: UtcMillis,
    pub keys: Vec<String>,
    /// (time, state per key), in time order.
    pub heartbeats: Vec<(UtcMillis, BTreeMap<String, String>)>,
    pub ended_at: Option<UtcMillis>,
}

fn push(
    out: &mut Vec<CoverageInterval>,
    start: i64,
    end: i64,
    state: CoverageState,
    reason: Option<&'static str>,
) {
    if end <= start {
        return;
    }
    if let Some(last) = out.last_mut()
        && last.end.0 == start
        && last.state == state
        && last.reason == reason
    {
        last.end = UtcMillis(end);
        return;
    }
    out.push(CoverageInterval {
        start: UtcMillis(start),
        end: UtcMillis(end),
        state,
        reason,
    });
}

fn explained_by_suspend(a: i64, b: i64, suspends: &[(UtcMillis, UtcMillis)]) -> bool {
    suspends
        .iter()
        .any(|(s, e)| s.0 <= a + STALL_MS && e.0 >= b - STALL_MS)
}

/// Coverage intervals of `key` from the epoch origin up to `as_of`.
pub fn coverage_for_key(
    key: &str,
    runs: &[Run],
    suspends: &[(UtcMillis, UtcMillis)],
    as_of: UtcMillis,
) -> Vec<CoverageInterval> {
    let mut runs: Vec<&Run> = runs.iter().filter(|r| r.started_at <= as_of).collect();
    runs.sort_by_key(|r| r.started_at);
    let mut out = Vec::new();
    let Some(first) = runs.first() else {
        push(
            &mut out,
            0,
            as_of.0,
            CoverageState::Unknown,
            Some("no_collector_yet"),
        );
        return out;
    };
    push(
        &mut out,
        0,
        first.started_at.0,
        CoverageState::Unknown,
        Some("no_collector_yet"),
    );

    for (i, run) in runs.iter().enumerate() {
        let next_start = runs.get(i + 1).map(|r| r.started_at.0);
        let beats: Vec<&(UtcMillis, BTreeMap<String, String>)> =
            run.heartbeats.iter().filter(|(t, _)| *t <= as_of).collect();
        let last_beat = beats.last().map(|b| b.0.0).unwrap_or(run.started_at.0);
        let end = match run.ended_at {
            Some(e) => e.0.min(as_of.0),
            None if next_start.is_none() && as_of.0 - last_beat <= STALL_MS => as_of.0,
            None => last_beat,
        };
        let end = next_start.map_or(end, |n| end.min(n));
        let listed = run.keys.iter().any(|k| k == key);

        // Points at which the state may change: start, heartbeats, end.
        let mut points: Vec<(i64, Option<&BTreeMap<String, String>>)> =
            vec![(run.started_at.0, None)];
        points.extend(
            beats
                .iter()
                .filter(|b| b.0.0 < end)
                .map(|b| (b.0.0, Some(&b.1))),
        );
        let mut state = if listed {
            CoverageState::Collecting
        } else {
            CoverageState::Missing
        };
        let mut reason = if listed { None } else { Some("not_configured") };
        for (n, (t, beat)) in points.iter().enumerate() {
            if let Some(b) = beat {
                match b.get(key).map(String::as_str) {
                    Some("collecting") => (state, reason) = (CoverageState::Collecting, None),
                    Some(_) => (state, reason) = (CoverageState::Missing, Some("key_unavailable")),
                    None => (state, reason) = (CoverageState::Missing, Some("not_configured")),
                }
            }
            let seg_end = points.get(n + 1).map_or(end, |p| p.0);
            if seg_end - t > STALL_MS && !explained_by_suspend(*t, seg_end, suspends) {
                push(
                    &mut out,
                    *t,
                    seg_end,
                    CoverageState::Missing,
                    Some("heartbeat_stall"),
                );
            } else {
                push(&mut out, *t, seg_end, state, reason);
            }
        }
        let gap_end = next_start.unwrap_or(as_of.0);
        push(
            &mut out,
            end,
            gap_end,
            CoverageState::Missing,
            Some("collector_down"),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: i64 = 60_000;

    fn run(start: i64, beats: &[i64], end: Option<i64>, sensor: &str) -> Run {
        Run {
            started_at: UtcMillis(start * M),
            keys: vec!["os_sensor".into(), "transcript".into()],
            heartbeats: beats
                .iter()
                .map(|b| {
                    let mut k = BTreeMap::new();
                    k.insert("os_sensor".into(), sensor.into());
                    k.insert("transcript".into(), "collecting".into());
                    (UtcMillis(b * M), k)
                })
                .collect(),
            ended_at: end.map(|e| UtcMillis(e * M)),
        }
    }

    fn states(v: &[CoverageInterval]) -> Vec<(i64, i64, &str)> {
        v.iter()
            .map(|c| (c.start.0 / M, c.end.0 / M, c.state.as_str()))
            .collect()
    }

    #[test]
    fn before_the_first_run_is_unknown_and_between_runs_is_missing() {
        let runs = [
            run(100, &[101, 102], Some(103), "collecting"),
            run(110, &[111], None, "collecting"),
        ];
        let c = coverage_for_key("transcript", &runs, &[], UtcMillis(112 * M));
        assert_eq!(
            states(&c),
            vec![
                (0, 100, "unknown"),
                (100, 103, "collecting"),
                (103, 110, "missing"),
                (110, 112, "collecting")
            ]
        );
    }

    #[test]
    fn a_heartbeat_stall_is_missing_unless_the_device_slept() {
        let runs = [run(0, &[1, 2, 30, 31], Some(32), "collecting")];
        let c = coverage_for_key("transcript", &runs, &[], UtcMillis(40 * M));
        assert!(states(&c).contains(&(2, 30, "missing")));
        let slept = coverage_for_key(
            "transcript",
            &runs,
            &[(UtcMillis(2 * M), UtcMillis(30 * M))],
            UtcMillis(40 * M),
        );
        assert!(states(&slept).contains(&(0, 32, "collecting")));
    }

    #[test]
    fn a_crashed_run_ends_at_its_last_heartbeat() {
        let runs = [run(0, &[1, 2], None, "collecting")];
        let c = coverage_for_key("transcript", &runs, &[], UtcMillis(60 * M));
        assert_eq!(states(&c), vec![(0, 2, "collecting"), (2, 60, "missing")]);
    }

    #[test]
    fn an_unavailable_sensor_is_missing_for_its_key_only() {
        let runs = [run(0, &[1, 2], Some(3), "unavailable")];
        let s = coverage_for_key("os_sensor", &runs, &[], UtcMillis(3 * M));
        assert!(states(&s).contains(&(1, 3, "missing")));
        let t = coverage_for_key("transcript", &runs, &[], UtcMillis(3 * M));
        assert!(states(&t).contains(&(0, 3, "collecting")));
    }
}
