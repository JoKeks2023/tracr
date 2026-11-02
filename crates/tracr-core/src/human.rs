//! Human timeline and counting (architecture §7.2–§7.4, activity model §5–§6).
//!
//! 1. Engagement instants of **all** sessions are sorted into one timeline.
//! 2. Reply window (first, user decision 2026-11-02): when an agent segment
//!    of any session ends inside `(t_i, t_j]` and the next prompt `t_j` comes
//!    at most `reply_window` after the latest such end, `[t_i, t_j)` is
//!    inferred work: the person waited for the agent and answered it. No
//!    session, idle, coverage or continuation condition applies; locked and
//!    suspended periods stay blank, and a degraded or gap extent of any agent
//!    stream falls back to the rule below (removed evidence never counts).
//!    Otherwise a gap `[t_i, t_j)` between consecutive instants is bridged as
//!    inferred work only if all of these hold: same session; gap ≤ continuation
//!    maximum; the device lane is active throughout (no idle ≥ the idle
//!    threshold, no lock, no suspend, no unknown); every required coverage
//!    key is collecting throughout; no degraded or gap extent overlaps it.
//! 3. Adjacent unbridged gaps merge into one `unknown` window, minus locked
//!    or suspended periods (blank). Reason, in precedence order:
//!    `coverage_gap` → `conflicting_evidence` (interior instants from ≥ 2
//!    sessions) → `unresolved_window`. Other reasons are kept.
//! 4. Before the first instant and after the last: blank (no interval).
//!    An unbridged gap longer than `max_unknown_gap` is blank too: a break
//!    with no evidence either way (user decision 2026-09-30). It also
//!    separates unknown windows, so they never chain across it.
//! 5. Idle inside an unknown window adds a proposal at the last input. It
//!    never creates minutes.
//!
//! Counting without user records (S1): every candidate is its own atom.
//! Unknown minutes have no route into counted minutes (INV-2).

use std::collections::{BTreeMap, BTreeSet};

use crate::coverage::{CoverageInterval, CoverageState};
use crate::device::{DeviceInterval, LaneState};
use crate::time::UtcMillis;

pub const HUMAN_RULE: &str = "human-timeline@3";

/// Rule parameters (AD-13; provisional dogfooding values, OQ-P-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HumanParams {
    pub continuation_max_ms: i64,
    pub idle_threshold_ms: i64,
    /// Longer unbridged gaps are blank, not unknown.
    pub max_unknown_gap_ms: i64,
    /// A prompt this soon after an agent finished keeps the person active
    /// for the whole gap since the previous prompt.
    pub reply_window_ms: i64,
}

impl Default for HumanParams {
    fn default() -> Self {
        Self {
            continuation_max_ms: 20 * 60_000,
            idle_threshold_ms: 5 * 60_000,
            max_unknown_gap_ms: 2 * 3_600_000,
            reply_window_ms: 5 * 60_000,
        }
    }
}

impl HumanParams {
    pub fn rule_ref(&self) -> String {
        format!(
            "{HUMAN_RULE}#continuation_max_ms={},idle_threshold_ms={},max_unknown_gap_ms={},reply_window_ms={}",
            self.continuation_max_ms,
            self.idle_threshold_ms,
            self.max_unknown_gap_ms,
            self.reply_window_ms
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkBasis {
    TimerMeasured,
    UserAsserted,
    InferredContinuity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UnknownReason {
    CoverageGap,
    ConflictingEvidence,
    UnresolvedWindow,
}

impl UnknownReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CoverageGap => "coverage_gap",
            Self::ConflictingEvidence => "conflicting_evidence",
            Self::UnresolvedWindow => "unresolved_window",
        }
    }
}

/// The basis lives inside `Work`: no work without a basis, and inferred
/// work can never carry a user-asserted basis (AC-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkStatus {
    Work(WorkBasis),
    Unknown(UnknownReason),
    NonWork,
}

/// One engagement instant: a human prompt in a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instant {
    pub at: UtcMillis,
    pub session: String,
}

/// A candidate interval of the human timeline (blank has no interval).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub start: UtcMillis,
    pub end: UtcMillis,
    pub status: WorkStatus,
    /// The session whose instants bound a bridged interval (evidence, not attribution).
    pub engaged: Option<String>,
    pub reasons: Vec<&'static str>,
    pub start_boundary: &'static str,
    pub end_boundary: &'static str,
    /// Review proposals (e.g. `idle_end_proposed` at the last input). Never minutes.
    pub proposals: Vec<(UtcMillis, &'static str)>,
}

/// Counted, unknown and non-work minutes are distinct types: they cannot be added together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CountedMs(pub i64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnknownMs(pub i64);

/// The only source of counted time: `Work` atoms.
pub fn counted(atoms: &[Candidate]) -> CountedMs {
    CountedMs(
        atoms
            .iter()
            .filter(|a| matches!(a.status, WorkStatus::Work(_)))
            .map(|a| a.end.0 - a.start.0)
            .sum(),
    )
}

pub fn unknown(atoms: &[Candidate]) -> UnknownMs {
    UnknownMs(
        atoms
            .iter()
            .filter(|a| matches!(a.status, WorkStatus::Unknown(_)))
            .map(|a| a.end.0 - a.start.0)
            .sum(),
    )
}

/// What the timeline rule reads besides the instants.
#[derive(Debug, Clone, Default)]
pub struct Evidence<'a> {
    pub device: &'a [DeviceInterval],
    /// Coverage per required key (the OS sensor and every configured agent source kind).
    pub coverage: BTreeMap<String, &'a [CoverageInterval]>,
    /// Degraded extents and completeness gaps of any agent stream.
    pub blocked: &'a [(UtcMillis, UtcMillis)],
    /// Ends of agent execution segments (any session, any actor) with their
    /// session: "the agent finished here".
    pub agent_ends: &'a [(UtcMillis, String)],
}

fn overlaps(a: i64, b: i64, s: i64, e: i64) -> bool {
    s < b && e > a
}

struct Gap {
    a: i64,
    b: i64,
    session_b: String,
    /// The session the person waited for and answered within the reply window.
    reply: Option<String>,
    bridged: bool,
    coverage_failed: bool,
    reasons: Vec<&'static str>,
}

fn assess(a: &Instant, b: &Instant, ev: &Evidence<'_>, p: &HumanParams) -> Gap {
    let (s, e) = (a.at.0, b.at.0);
    let mut reasons = Vec::new();
    let mut coverage_failed = false;
    for intervals in ev.coverage.values() {
        let ok = intervals
            .iter()
            .filter(|c| overlaps(s, e, c.start.0, c.end.0))
            .all(|c| c.state == CoverageState::Collecting)
            && intervals.iter().any(|c| c.start.0 <= s && c.end.0 >= s);
        if !ok {
            coverage_failed = true;
        }
    }
    if ev.coverage.is_empty() {
        coverage_failed = true;
    }
    let device: Vec<&DeviceInterval> = ev
        .device
        .iter()
        .filter(|d| overlaps(s, e, d.start.0, d.end.0))
        .collect();
    let device_covers = ev.device.iter().any(|d| d.start.0 <= s && d.end.0 >= s);
    if !device_covers || device.iter().any(|d| d.state == LaneState::Unknown) {
        coverage_failed = true;
    }
    if ev.blocked.iter().any(|(bs, be)| overlaps(s, e, bs.0, be.0)) {
        coverage_failed = true;
        reasons.push("degraded_or_gap");
    }
    if coverage_failed {
        reasons.push("coverage");
    }
    let idle = device
        .iter()
        .any(|d| d.state == LaneState::Idle && d.end.0 - d.start.0 >= p.idle_threshold_ms);
    if idle {
        reasons.push("idle");
    }
    let away = device
        .iter()
        .any(|d| matches!(d.state, LaneState::Locked | LaneState::Suspended));
    if away {
        reasons.push("locked_or_suspended");
    }
    let same = a.session == b.session;
    if !same {
        reasons.push("cross_session");
    }
    let too_long = e - s > p.continuation_max_ms;
    if too_long {
        reasons.push("exceeds_continuation_max");
    }
    let blocked = ev.blocked.iter().any(|(bs, be)| overlaps(s, e, bs.0, be.0));
    let reply = (!blocked)
        .then(|| {
            ev.agent_ends
                .iter()
                .filter(|(t, _)| t.0 > s && t.0 <= e)
                .max_by_key(|(t, _)| t.0)
        })
        .flatten()
        .filter(|(t, _)| e - t.0 <= p.reply_window_ms)
        .map(|(_, session)| session.clone());
    Gap {
        a: s,
        b: e,
        session_b: b.session.clone(),
        reply,
        bridged: same && !too_long && !coverage_failed && !idle && !away,
        coverage_failed,
        reasons,
    }
}

/// Removes locked and suspended periods (blank) from an unknown window.
fn minus_away(
    start: i64,
    end: i64,
    device: &[DeviceInterval],
) -> Vec<(i64, i64, &'static str, &'static str)> {
    let mut pieces = vec![(start, end, "observed", "observed")];
    for d in device
        .iter()
        .filter(|d| matches!(d.state, LaneState::Locked | LaneState::Suspended))
        .filter(|d| overlaps(start, end, d.start.0, d.end.0))
    {
        let mut next = Vec::new();
        for (s, e, sb, eb) in pieces {
            if !overlaps(s, e, d.start.0, d.end.0) {
                next.push((s, e, sb, eb));
                continue;
            }
            if d.start.0 > s {
                next.push((s, d.start.0, sb, "device"));
            }
            if d.end.0 < e {
                next.push((d.end.0, e, "device", eb));
            }
        }
        pieces = next;
    }
    pieces
}

/// Appends inferred work, extending the previous piece when it is work of the
/// same session ending where this one starts.
fn push_work(
    out: &mut Vec<Candidate>,
    s: i64,
    e: i64,
    session: &str,
    reason: &'static str,
    start_boundary: &'static str,
    end_boundary: &'static str,
) {
    if let Some(last) = out.last_mut()
        && last.end.0 == s
        && last.engaged.as_deref() == Some(session)
        && matches!(last.status, WorkStatus::Work(_))
    {
        last.end = UtcMillis(e);
        last.end_boundary = end_boundary;
        if !last.reasons.contains(&reason) {
            last.reasons.push(reason);
        }
        return;
    }
    out.push(Candidate {
        start: UtcMillis(s),
        end: UtcMillis(e),
        status: WorkStatus::Work(WorkBasis::InferredContinuity),
        engaged: Some(session.to_owned()),
        reasons: vec![reason],
        start_boundary,
        end_boundary,
        proposals: Vec::new(),
    });
}

pub fn human_timeline(instants: &[Instant], ev: &Evidence<'_>, p: &HumanParams) -> Vec<Candidate> {
    let mut sorted: Vec<&Instant> = instants.iter().collect();
    sorted.sort_by(|a, b| (a.at, &a.session).cmp(&(b.at, &b.session)));
    sorted.dedup_by(|a, b| a.at == b.at && a.session == b.session);

    let gaps: Vec<Gap> = sorted
        .windows(2)
        .filter(|w| w[1].at > w[0].at)
        .map(|w| assess(w[0], w[1], ev, p))
        .collect();

    let mut out: Vec<Candidate> = Vec::new();
    let mut i = 0;
    while i < gaps.len() {
        let g = &gaps[i];
        if let Some(session) = &g.reply {
            for (s, e, sb, eb) in minus_away(g.a, g.b, ev.device) {
                push_work(&mut out, s, e, session, "agent_turn_reply", sb, eb);
            }
            i += 1;
            continue;
        }
        if g.bridged {
            push_work(
                &mut out,
                g.a,
                g.b,
                &g.session_b,
                "bridged_same_session",
                "observed",
                "observed",
            );
            i += 1;
            continue;
        }
        let too_long = |g: &Gap| g.b - g.a > p.max_unknown_gap_ms;
        if too_long(g) {
            i += 1; // a break: blank, and a separator between unknown windows
            continue;
        }
        // A run of adjacent unbridged gaps is one window.
        let mut j = i;
        while j + 1 < gaps.len()
            && !gaps[j + 1].bridged
            && gaps[j + 1].reply.is_none()
            && !too_long(&gaps[j + 1])
            && gaps[j + 1].a == gaps[j].b
        {
            j += 1;
        }
        let run = &gaps[i..=j];
        let (start, end) = (run[0].a, run[run.len() - 1].b);
        let interior: BTreeSet<&str> = run[..run.len() - 1]
            .iter()
            .map(|g| g.session_b.as_str())
            .collect();
        let reason = if run.iter().any(|g| g.coverage_failed) {
            UnknownReason::CoverageGap
        } else if interior.len() >= 2 {
            UnknownReason::ConflictingEvidence
        } else {
            UnknownReason::UnresolvedWindow
        };
        let mut reasons: Vec<&'static str> =
            run.iter().flat_map(|g| g.reasons.iter().copied()).collect();
        reasons.sort_unstable();
        reasons.dedup();
        let proposals: Vec<(UtcMillis, &'static str)> = ev
            .device
            .iter()
            .filter(|d| d.state == LaneState::Idle && d.end.0 - d.start.0 >= p.idle_threshold_ms)
            .filter(|d| overlaps(start, end, d.start.0, d.end.0))
            .map(|d| (d.start, "idle_end_proposed"))
            .collect();
        for (s, e, sb, eb) in minus_away(start, end, ev.device) {
            out.push(Candidate {
                start: UtcMillis(s),
                end: UtcMillis(e),
                status: WorkStatus::Unknown(reason),
                engaged: None,
                reasons: reasons.clone(),
                start_boundary: sb,
                end_boundary: eb,
                proposals: proposals
                    .iter()
                    .copied()
                    .filter(|(t, _)| t.0 >= s && t.0 < e)
                    .collect(),
            });
        }
        i = j + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::CoverageInterval;

    const M: i64 = 60_000;

    fn hm(h: i64, m: i64) -> i64 {
        (h * 60 + m) * M
    }

    fn inst(session: &str, times: &[(i64, i64)]) -> Vec<Instant> {
        times
            .iter()
            .map(|&(h, m)| Instant {
                at: UtcMillis(hm(h, m)),
                session: session.into(),
            })
            .collect()
    }

    struct World {
        device: Vec<DeviceInterval>,
        coverage: Vec<CoverageInterval>,
    }

    fn world(device: Vec<(i64, i64, LaneState)>) -> World {
        World {
            device: device
                .into_iter()
                .map(|(s, e, state)| DeviceInterval {
                    start: UtcMillis(s),
                    end: UtcMillis(e),
                    state,
                })
                .collect(),
            coverage: vec![CoverageInterval {
                start: UtcMillis(0),
                end: UtcMillis(hm(24, 0)),
                state: CoverageState::Collecting,
                reason: None,
            }],
        }
    }

    fn run(instants: &[Instant], w: &World, p: HumanParams) -> Vec<Candidate> {
        let mut coverage = BTreeMap::new();
        coverage.insert("os_sensor".to_owned(), w.coverage.as_slice());
        coverage.insert("transcript".to_owned(), w.coverage.as_slice());
        human_timeline(
            instants,
            &Evidence {
                device: &w.device,
                coverage,
                blocked: &[],
                agent_ends: &[],
            },
            &p,
        )
    }

    /// Like `run`, with agent segment ends and optional coverage.
    fn run_with(
        instants: &[Instant],
        w: &World,
        ends: &[(i64, &str)],
        covered: bool,
        blocked: &[(UtcMillis, UtcMillis)],
    ) -> Vec<Candidate> {
        let ends: Vec<(UtcMillis, String)> = ends
            .iter()
            .map(|&(t, s)| (UtcMillis(t), s.to_owned()))
            .collect();
        let mut coverage = BTreeMap::new();
        if covered {
            coverage.insert("os_sensor".to_owned(), w.coverage.as_slice());
        }
        human_timeline(
            instants,
            &Evidence {
                device: &w.device,
                coverage,
                blocked,
                agent_ends: &ends,
            },
            &HumanParams::default(),
        )
    }

    fn no_sensor() -> World {
        World {
            device: Vec::new(),
            coverage: Vec::new(),
        }
    }

    #[test]
    fn a_reply_within_five_minutes_of_the_agent_keeps_the_whole_gap_active() {
        // Prompt 10:00, Claude works until 10:25 (no input: idle 15 min), reply 10:28.
        let i = inst("A", &[(10, 0), (10, 28)]);
        let w = world(vec![
            (0, hm(10, 5), LaneState::Active),
            (hm(10, 5), hm(10, 25), LaneState::Idle),
            (hm(10, 25), hm(24, 0), LaneState::Active),
        ]);
        let c = run_with(&i, &w, &[(hm(10, 25), "A")], true, &[]);
        assert_eq!(mins(&c), (28, 0));
        assert_eq!(c[0].reasons, vec!["agent_turn_reply"]);
        // The same without any device sensor or coverage.
        let c = run_with(&i, &no_sensor(), &[(hm(10, 25), "A")], false, &[]);
        assert_eq!(mins(&c), (28, 0));
    }

    #[test]
    fn a_reply_after_more_than_five_minutes_stays_unknown() {
        let i = inst("A", &[(10, 0), (10, 31)]);
        let c = run_with(&i, &no_sensor(), &[(hm(10, 25), "A")], false, &[]);
        assert_eq!(mins(&c), (0, 31));
        // Exactly five minutes still counts.
        let i = inst("A", &[(10, 0), (10, 30)]);
        let c = run_with(&i, &no_sensor(), &[(hm(10, 25), "A")], false, &[]);
        assert_eq!(mins(&c), (30, 0));
    }

    #[test]
    fn a_reply_in_another_session_counts_once_for_the_awaited_session() {
        // A works 10:00–10:20; the person answers in B at 10:22, B runs to 10:40, A again 10:43.
        let mut i = inst("A", &[(10, 0), (10, 43)]);
        i.extend(inst("B", &[(10, 22)]));
        let ends = [(hm(10, 20), "A"), (hm(10, 40), "B")];
        let c = run_with(&i, &no_sensor(), &ends, false, &[]);
        assert_eq!(mins(&c), (43, 0));
        let engaged: Vec<_> = c.iter().map(|x| x.engaged.clone().unwrap()).collect();
        assert_eq!(engaged, ["A", "B"]);
    }

    #[test]
    fn lock_and_sleep_inside_a_reply_gap_stay_blank() {
        let i = inst("A", &[(10, 0), (10, 30)]);
        let w = world(vec![
            (0, hm(10, 10), LaneState::Active),
            (hm(10, 10), hm(10, 20), LaneState::Locked),
            (hm(10, 20), hm(24, 0), LaneState::Active),
        ]);
        let c = run_with(&i, &w, &[(hm(10, 27), "A")], true, &[]);
        assert_eq!(mins(&c), (20, 0));
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn removed_evidence_or_no_agent_run_falls_back_to_the_old_rule() {
        let i = inst("A", &[(10, 0), (10, 28)]);
        let blocked = [(UtcMillis(hm(10, 5)), UtcMillis(hm(10, 6)))];
        let c = run_with(&i, &no_sensor(), &[(hm(10, 25), "A")], false, &blocked);
        assert_eq!(mins(&c), (0, 28));
        // No segment ends between the prompts: the old rule (no sensor = unknown).
        let c = run_with(&i, &no_sensor(), &[(hm(9, 50), "A")], false, &[]);
        assert_eq!(mins(&c), (0, 28));
        // ...and with the sensor active the old same-session bridge still applies.
        let i = inst("A", &[(10, 0), (10, 15)]);
        let c = run_with(&i, &active_all_day(), &[], true, &[]);
        assert_eq!(mins(&c), (15, 0));
        assert_eq!(c[0].reasons, vec!["bridged_same_session"]);
    }

    fn mins(c: &[Candidate]) -> (i64, i64) {
        (counted(c).0 / M, unknown(c).0 / M)
    }

    fn active_all_day() -> World {
        world(vec![(0, hm(24, 0), LaneState::Active)])
    }

    #[test]
    fn walkthrough_b_counts_87_and_leaves_20_unknown() {
        let mut i = inst(
            "A",
            &[
                (9, 0),
                (9, 12),
                (9, 21),
                (9, 25),
                (10, 15),
                (10, 30),
                (10, 47),
            ],
        );
        i.extend(inst("B", &[(9, 32), (9, 40), (9, 51), (10, 2)]));
        let p = HumanParams {
            continuation_max_ms: 17 * M,
            ..HumanParams::default()
        };
        let c = run(&i, &active_all_day(), p);
        assert_eq!(mins(&c), (87, 20));
        let unknown: Vec<(i64, i64, UnknownReason)> = c
            .iter()
            .filter_map(|x| match x.status {
                WorkStatus::Unknown(r) => Some((x.start.0, x.end.0, r)),
                _ => None,
            })
            .collect();
        assert_eq!(
            unknown,
            vec![
                (hm(9, 25), hm(9, 32), UnknownReason::UnresolvedWindow),
                (hm(10, 2), hm(10, 15), UnknownReason::UnresolvedWindow)
            ]
        );
        // Evidence span 09:00–10:47 = 107 is only an upper bound: 87 + 20.
    }

    #[test]
    fn interleaved_sessions_count_nothing_and_are_conflicting() {
        let mut i = inst("A", &[(9, 0), (9, 20)]);
        i.extend(inst("B", &[(9, 10), (9, 25)]));
        let c = run(&i, &active_all_day(), HumanParams::default());
        assert_eq!(mins(&c), (0, 25));
        assert_eq!(
            c[0].status,
            WorkStatus::Unknown(UnknownReason::ConflictingEvidence)
        );
    }

    #[test]
    fn a_lock_inside_a_same_session_gap_is_blank_and_the_rest_unknown() {
        // A 11:55, lock 12:00–13:00, A 13:05.
        let w = world(vec![
            (0, hm(12, 0), LaneState::Active),
            (hm(12, 0), hm(13, 0), LaneState::Locked),
            (hm(13, 0), hm(24, 0), LaneState::Active),
        ]);
        let c = run(&inst("A", &[(11, 55), (13, 5)]), &w, HumanParams::default());
        assert_eq!(mins(&c), (0, 10));
        let spans: Vec<(i64, i64)> = c.iter().map(|x| (x.start.0, x.end.0)).collect();
        assert_eq!(spans, vec![(hm(11, 55), hm(12, 0)), (hm(13, 0), hm(13, 5))]);
    }

    #[test]
    fn idle_above_the_threshold_blocks_bridging_and_only_proposes() {
        // A 10:00, last input 10:02, idle until 10:12, A 10:14.
        let w = world(vec![
            (0, hm(10, 2), LaneState::Active),
            (hm(10, 2), hm(10, 12), LaneState::Idle),
            (hm(10, 12), hm(24, 0), LaneState::Active),
        ]);
        let c = run(&inst("A", &[(10, 0), (10, 14)]), &w, HumanParams::default());
        assert_eq!(mins(&c), (0, 14));
        assert_eq!(
            c[0].status,
            WorkStatus::Unknown(UnknownReason::UnresolvedWindow)
        );
        assert_eq!(
            c[0].proposals,
            vec![(UtcMillis(hm(10, 2)), "idle_end_proposed")]
        );
        // Idle below the threshold is bridged.
        let short = world(vec![
            (0, hm(10, 2), LaneState::Active),
            (hm(10, 2), hm(10, 5), LaneState::Idle),
            (hm(10, 5), hm(24, 0), LaneState::Active),
        ]);
        assert_eq!(
            mins(&run(
                &inst("A", &[(10, 0), (10, 14)]),
                &short,
                HumanParams::default()
            )),
            (14, 0)
        );
    }

    #[test]
    fn without_device_coverage_nothing_is_counted() {
        // Backfilled history: the device lane is unknown.
        let w = world(vec![(0, hm(24, 0), LaneState::Unknown)]);
        let c = run(&inst("A", &[(9, 0), (9, 10)]), &w, HumanParams::default());
        assert_eq!(mins(&c), (0, 10));
        assert_eq!(c[0].status, WorkStatus::Unknown(UnknownReason::CoverageGap));
    }

    #[test]
    fn gaps_longer_than_the_maximum_are_blank_and_split_unknown_windows() {
        // A 09:00, B 09:10 (unknown 10), then 3 h nothing, A 12:10, B 12:20 (unknown 10).
        let mut i = inst("A", &[(9, 0), (12, 10)]);
        i.extend(inst("B", &[(9, 10), (12, 20)]));
        let c = run(&i, &active_all_day(), HumanParams::default());
        assert_eq!(mins(&c), (0, 20));
        let spans: Vec<(i64, i64)> = c.iter().map(|x| (x.start.0, x.end.0)).collect();
        assert_eq!(spans, vec![(hm(9, 0), hm(9, 10)), (hm(12, 10), hm(12, 20))]);
        // A 1-hour gap (a meeting) stays unknown for review.
        let c = run(
            &inst("A", &[(9, 0), (10, 0)]),
            &active_all_day(),
            HumanParams::default(),
        );
        assert_eq!(mins(&c), (0, 60));
    }

    #[test]
    fn before_the_first_and_after_the_last_instant_is_blank() {
        let c = run(
            &inst("A", &[(9, 0)]),
            &active_all_day(),
            HumanParams::default(),
        );
        assert!(c.is_empty(), "a single instant has no duration");
    }
}
