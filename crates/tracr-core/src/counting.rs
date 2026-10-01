//! Counting and explicit attribution with user corrections (architecture
//! §6.5, §7.4; time and worklog §3).
//!
//! Corrections are anchored to **time ranges** of the human timeline, so they
//! keep applying after any re-derivation and never become orphans.
//!
//! * Counting axis, per instant: the latest (by `seq`) non-revoked
//!   `confirm_work` / `mark_non_work` covering it decides (`work` with basis
//!   `user_asserted`, or `non_work`); otherwise the inferred candidate stands.
//!   Confirming a blank range creates user-asserted work there.
//! * Attribution axis, per instant: non-revoked `set_target` / `reject_target`
//!   in `seq` order; set replaces the target, reject removes it. It never
//!   changes minutes, and the counting axis never reads it.
//! * `revoke(k)` removes k's effect; k stays in history.
//! * Manual entries (timer or retroactive) come second on both axes: they
//!   count their interval (`timer_measured` / `user_asserted`) and give their
//!   target, unless a correction decides. Overlapping entries count once;
//!   different targets stay `conflicting` (TW-04). Idle and lock never end
//!   them.
//!
//! The candidate underneath stays on every atom, so "overridden by …" comes
//! from stored rows.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::human::{Candidate, WorkBasis, WorkStatus};
use crate::time::UtcMillis;

/// Fixed categories for non-code work (UI-01: a dropdown with "Other…").
pub const CATEGORIES: &[&str] = &[
    "meeting", "planning", "review", "support", "admin", "learning",
];

/// What time can be attributed to in S2 (a subset of architecture §9 `TargetRef`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "key", rename_all = "snake_case")]
pub enum TargetRef {
    /// A repository key (normalized remote or `local:<machine>:<dir>`).
    Repository(String),
    /// One of [`CATEGORIES`].
    Category(String),
    /// "Other…" with the user's own short label.
    Other(String),
}

impl TargetRef {
    /// Natural key, stable across rebuilds (`repository:…`, `category:…`, `other:…`).
    pub fn key(&self) -> String {
        match self {
            Self::Repository(k) => format!("repository:{k}"),
            Self::Category(c) => format!("category:{c}"),
            Self::Other(t) => format!("other:{t}"),
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        let (kind, rest) = key.split_once(':')?;
        if rest.is_empty() {
            return None;
        }
        match kind {
            "repository" => Some(Self::Repository(rest.to_owned())),
            "category" if CATEGORIES.contains(&rest) => Some(Self::Category(rest.to_owned())),
            "other" if rest.chars().count() <= 80 && !rest.chars().any(char::is_control) => {
                Some(Self::Other(rest.to_owned()))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionOp {
    ConfirmWork,
    MarkNonWork,
    SetTarget(TargetRef),
    RejectTarget(TargetRef),
    /// Removes the effect of the correction with this ID.
    Revoke(String),
}

impl CorrectionOp {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ConfirmWork => "confirm_work",
            Self::MarkNonWork => "mark_non_work",
            Self::SetTarget(_) => "set_target",
            Self::RejectTarget(_) => "reject_target",
            Self::Revoke(_) => "revoke",
        }
    }

    pub fn axis(&self) -> &'static str {
        match self {
            Self::ConfirmWork | Self::MarkNonWork => "counting",
            Self::SetTarget(_) | Self::RejectTarget(_) => "attribution",
            Self::Revoke(_) => "revoke",
        }
    }
}

/// One user correction as stored in the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    pub id: String,
    /// `user_action.seq`: the global order of user writes.
    pub seq: i64,
    pub start: UtcMillis,
    pub end: UtcMillis,
    pub op: CorrectionOp,
}

/// The current revision of a manual entry. A running timer has its end
/// resolved to `as_of` by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualEntry {
    pub id: String,
    pub rev: i64,
    pub start: UtcMillis,
    pub end: UtcMillis,
    /// A timer (`timer_measured`) or a retroactive entry (`user_asserted`).
    pub timer: bool,
    pub target: Option<TargetRef>,
}

/// A counted atom: constant status, candidate and target over `[start, end)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atom {
    pub start: UtcMillis,
    pub end: UtcMillis,
    pub status: WorkStatus,
    /// The derivation underneath (`None` = blank: no candidate interval).
    pub candidate: Option<Candidate>,
    /// The correction that decided the status, if any.
    pub decided_by: Option<String>,
    /// The explicit target set by corrections, if any, and the deciding one.
    pub target: Option<(TargetRef, String)>,
    /// Targets rejected by corrections covering this atom (a suppression set).
    pub rejected: BTreeSet<TargetRef>,
    /// Manual entries covering this atom, as `id@rev`.
    pub manual: Vec<String>,
    /// Targets of the covering manual entries, minus rejected ones.
    pub manual_targets: BTreeSet<TargetRef>,
}

/// IDs of corrections removed by a non-revoked `revoke` (a revoke can itself be revoked).
pub fn revoked(corrections: &[Correction]) -> BTreeSet<String> {
    let mut sorted: Vec<&Correction> = corrections.iter().collect();
    sorted.sort_by_key(|c| std::cmp::Reverse(c.seq));
    let mut out = BTreeSet::new();
    // Later revokes decide first: a revoked revoke has no effect.
    for c in sorted {
        if let CorrectionOp::Revoke(target) = &c.op
            && !out.contains(&c.id)
        {
            out.insert(target.clone());
        }
    }
    out
}

pub fn apply_corrections(candidates: &[Candidate], corrections: &[Correction]) -> Vec<Atom> {
    count(candidates, &[], corrections)
}

/// Counting precedence (§7.4): correction → manual entry → inferred → unknown/blank.
pub fn count(
    candidates: &[Candidate],
    manual: &[ManualEntry],
    corrections: &[Correction],
) -> Vec<Atom> {
    let gone = revoked(corrections);
    let mut live: Vec<&Correction> = corrections
        .iter()
        .filter(|c| {
            !gone.contains(&c.id) && !matches!(c.op, CorrectionOp::Revoke(_)) && c.end > c.start
        })
        .collect();
    live.sort_by_key(|c| c.seq);

    let mut bounds: Vec<i64> = Vec::new();
    for c in candidates {
        bounds.extend([c.start.0, c.end.0]);
    }
    for c in &live {
        bounds.extend([c.start.0, c.end.0]);
    }
    for m in manual.iter().filter(|m| m.end > m.start) {
        bounds.extend([m.start.0, m.end.0]);
    }
    bounds.sort_unstable();
    bounds.dedup();

    let mut out: Vec<Atom> = Vec::new();
    for w in bounds.windows(2) {
        let (a, b) = (w[0], w[1]);
        let covers = |s: i64, e: i64| s <= a && e >= b;
        let candidate = candidates.iter().find(|c| covers(c.start.0, c.end.0));
        let counting = live.iter().rev().find(|c| {
            covers(c.start.0, c.end.0)
                && matches!(c.op, CorrectionOp::ConfirmWork | CorrectionOp::MarkNonWork)
        });
        let entries: Vec<&ManualEntry> = manual
            .iter()
            .filter(|m| covers(m.start.0, m.end.0))
            .collect();
        let status = match (counting.map(|c| &c.op), entries.is_empty(), candidate) {
            (Some(CorrectionOp::ConfirmWork), _, _) => WorkStatus::Work(WorkBasis::UserAsserted),
            (Some(_), _, _) => WorkStatus::NonWork,
            (None, false, _) if entries.iter().all(|m| m.timer) => {
                WorkStatus::Work(WorkBasis::TimerMeasured)
            }
            (None, false, _) => WorkStatus::Work(WorkBasis::UserAsserted),
            (None, true, Some(c)) => c.status,
            (None, true, None) => continue, // blank
        };
        let mut target: Option<(TargetRef, String)> = None;
        let mut rejected = BTreeSet::new();
        for c in live.iter().filter(|c| covers(c.start.0, c.end.0)) {
            match &c.op {
                CorrectionOp::SetTarget(t) => {
                    rejected.remove(t);
                    target = Some((t.clone(), c.id.clone()));
                }
                CorrectionOp::RejectTarget(t) => {
                    if target.as_ref().is_some_and(|(x, _)| x == t) {
                        target = None;
                    }
                    rejected.insert(t.clone());
                }
                _ => {}
            }
        }
        let manual_targets: BTreeSet<TargetRef> = entries
            .iter()
            .filter_map(|m| m.target.clone())
            .filter(|t| !rejected.contains(t))
            .collect();
        let atom = Atom {
            start: UtcMillis(a),
            end: UtcMillis(b),
            status,
            candidate: candidate.cloned(),
            decided_by: counting.map(|c| c.id.clone()),
            target,
            rejected,
            manual: entries
                .iter()
                .map(|m| format!("{}@{}", m.id, m.rev))
                .collect(),
            manual_targets,
        };
        match out.last_mut() {
            Some(last)
                if last.end.0 == a
                    && last.status == atom.status
                    && last.candidate == atom.candidate
                    && last.decided_by == atom.decided_by
                    && last.target == atom.target
                    && last.rejected == atom.rejected
                    && last.manual == atom.manual =>
            {
                last.end = atom.end;
            }
            _ => out.push(atom),
        }
    }
    out
}

/// Counted minutes by status from atoms, as `(counted, unknown, non_work)` ms.
pub fn totals(atoms: &[Atom]) -> (i64, i64, i64) {
    atoms.iter().fold((0, 0, 0), |(c, u, n), a| {
        let len = a.end.0 - a.start.0;
        match a.status {
            WorkStatus::Work(_) => (c + len, u, n),
            WorkStatus::Unknown(_) => (c, u + len, n),
            WorkStatus::NonWork => (c, u, n + len),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::{CoverageInterval, CoverageState};
    use crate::device::{DeviceInterval, LaneState};
    use crate::human::{Evidence, HumanParams, Instant, human_timeline};
    use std::collections::BTreeMap;

    const M: i64 = 60_000;

    fn hm(h: i64, m: i64) -> i64 {
        (h * 60 + m) * M
    }

    /// Walkthrough B before review: 87 counted, 20 unknown.
    fn walkthrough_b() -> Vec<Candidate> {
        let mut i: Vec<Instant> = [
            (9, 0),
            (9, 12),
            (9, 21),
            (9, 25),
            (10, 15),
            (10, 30),
            (10, 47),
        ]
        .iter()
        .map(|&(h, m)| Instant {
            at: UtcMillis(hm(h, m)),
            session: "A".into(),
        })
        .collect();
        i.extend(
            [(9, 32), (9, 40), (9, 51), (10, 2)]
                .iter()
                .map(|&(h, m)| Instant {
                    at: UtcMillis(hm(h, m)),
                    session: "B".into(),
                }),
        );
        let device = [DeviceInterval {
            start: UtcMillis(0),
            end: UtcMillis(hm(24, 0)),
            state: LaneState::Active,
        }];
        let cov = [CoverageInterval {
            start: UtcMillis(0),
            end: UtcMillis(hm(24, 0)),
            state: CoverageState::Collecting,
            reason: None,
        }];
        let mut coverage = BTreeMap::new();
        coverage.insert("os_sensor".to_owned(), &cov[..]);
        coverage.insert("transcript".to_owned(), &cov[..]);
        let p = HumanParams {
            continuation_max_ms: 17 * M,
            ..HumanParams::default()
        };
        human_timeline(
            &i,
            &Evidence {
                device: &device,
                coverage,
                blocked: &[],
            },
            &p,
        )
    }

    fn c(id: &str, seq: i64, from: (i64, i64), to: (i64, i64), op: CorrectionOp) -> Correction {
        Correction {
            id: id.into(),
            seq,
            start: UtcMillis(hm(from.0, from.1)),
            end: UtcMillis(hm(to.0, to.1)),
            op,
        }
    }

    fn mins(atoms: &[Atom]) -> (i64, i64, i64) {
        let (a, b, n) = totals(atoms);
        (a / M, b / M, n / M)
    }

    fn target_minutes(atoms: &[Atom], t: &TargetRef) -> i64 {
        atoms
            .iter()
            .filter(|a| matches!(a.status, WorkStatus::Work(_)))
            .filter(|a| a.target.as_ref().map(|x| &x.0) == Some(t))
            .map(|a| (a.end.0 - a.start.0) / M)
            .sum()
    }

    #[test]
    fn walkthrough_b_outcomes() {
        let cand = walkthrough_b();
        // Unresolved (default).
        assert_eq!(mins(&apply_corrections(&cand, &[])), (87, 20, 0));
        // Break: both windows marked as break.
        let brk = [
            c("k1", 1, (9, 25), (9, 32), CorrectionOp::MarkNonWork),
            c("k2", 2, (10, 2), (10, 15), CorrectionOp::MarkNonWork),
        ];
        assert_eq!(mins(&apply_corrections(&cand, &brk)), (87, 0, 20));
        // User-confirmed work: two confirm_work + two set_target.
        let a = TargetRef::Category("review".into());
        let b = TargetRef::Category("support".into());
        let confirmed = [
            c("k1", 1, (9, 25), (9, 32), CorrectionOp::ConfirmWork),
            c(
                "k2",
                2,
                (9, 25),
                (9, 32),
                CorrectionOp::SetTarget(a.clone()),
            ),
            c("k3", 3, (10, 2), (10, 15), CorrectionOp::ConfirmWork),
            c(
                "k4",
                4,
                (10, 2),
                (10, 15),
                CorrectionOp::SetTarget(b.clone()),
            ),
        ];
        let atoms = apply_corrections(&cand, &confirmed);
        assert_eq!(mins(&atoms), (107, 0, 0));
        assert_eq!(
            (target_minutes(&atoms, &a), target_minutes(&atoms, &b)),
            (7, 13)
        );
        // Inferred minutes keep their basis; the confirmed ones are user-asserted.
        let asserted: i64 = atoms
            .iter()
            .filter(|x| x.status == WorkStatus::Work(WorkBasis::UserAsserted))
            .map(|x| (x.end.0 - x.start.0) / M)
            .sum();
        assert_eq!(asserted, 20);
        // Partial: confirm the first window, break the second.
        let partial = [
            c("k1", 1, (9, 25), (9, 32), CorrectionOp::ConfirmWork),
            c("k2", 2, (10, 2), (10, 15), CorrectionOp::MarkNonWork),
        ];
        assert_eq!(mins(&apply_corrections(&cand, &partial)), (94, 0, 13));
    }

    #[test]
    fn a_later_correction_supersedes_only_the_overlap_and_revoke_restores() {
        let cand = walkthrough_b();
        let k = [
            c("k1", 1, (9, 25), (9, 32), CorrectionOp::ConfirmWork),
            c("k2", 2, (9, 30), (9, 40), CorrectionOp::MarkNonWork),
        ];
        // 09:25–09:30 confirmed (5), 09:30–09:40 non-work (10, incl. 8 inferred B).
        assert_eq!(
            mins(&apply_corrections(&cand, &k)),
            (87 + 5 - 8, 20 - 7, 10)
        );
        let mut undone = k.to_vec();
        undone.push(c(
            "k3",
            3,
            (0, 0),
            (0, 0),
            CorrectionOp::Revoke("k2".into()),
        ));
        assert_eq!(mins(&apply_corrections(&cand, &undone)), (87 + 7, 13, 0));
        // Revoking the revoke brings k2 back.
        undone.push(c(
            "k4",
            4,
            (0, 0),
            (0, 0),
            CorrectionOp::Revoke("k3".into()),
        ));
        assert_eq!(
            mins(&apply_corrections(&cand, &undone)),
            (87 + 5 - 8, 20 - 7, 10)
        );
    }

    #[test]
    fn confirming_blank_time_counts_it_and_attribution_never_changes_minutes() {
        let cand = walkthrough_b();
        let t = TargetRef::Other("Workshop".into());
        let k = [
            c("k1", 1, (11, 0), (12, 0), CorrectionOp::ConfirmWork),
            c(
                "k2",
                2,
                (9, 0),
                (10, 47),
                CorrectionOp::SetTarget(t.clone()),
            ),
        ];
        let atoms = apply_corrections(&cand, &k);
        assert_eq!(
            mins(&atoms),
            (87 + 60, 20, 0),
            "a target never adds minutes"
        );
        assert!(
            atoms
                .iter()
                .any(|a| a.candidate.is_none() && a.decided_by.as_deref() == Some("k1"))
        );
        // The set target applies to work only for totals; unknown stays unknown.
        assert_eq!(target_minutes(&atoms, &t), 87);
        // Rejecting the target removes it again.
        let mut r = k.to_vec();
        r.push(c(
            "k3",
            3,
            (9, 0),
            (10, 47),
            CorrectionOp::RejectTarget(t.clone()),
        ));
        assert_eq!(target_minutes(&apply_corrections(&cand, &r), &t), 0);
    }

    fn entry(
        id: &str,
        from: (i64, i64),
        to: (i64, i64),
        timer: bool,
        target: Option<TargetRef>,
    ) -> ManualEntry {
        ManualEntry {
            id: id.into(),
            rev: 1,
            start: UtcMillis(hm(from.0, from.1)),
            end: UtcMillis(hm(to.0, to.1)),
            timer,
            target,
        }
    }

    #[test]
    fn walkthrough_a_manual_only_day_counts_135_and_leaves_the_rest_blank() {
        let design = TargetRef::Other("TRACR-12 design".into());
        let meeting = TargetRef::Category("meeting".into());
        let atoms = count(
            &[],
            &[
                entry("t1", (8, 30), (9, 45), true, Some(design.clone())),
                entry("m1", (13, 0), (14, 0), false, Some(meeting.clone())),
            ],
            &[],
        );
        assert_eq!(mins(&atoms), (135, 0, 0));
        assert_eq!(atoms.len(), 2, "nothing else: blank has no atom");
        assert_eq!(atoms[0].status, WorkStatus::Work(WorkBasis::TimerMeasured));
        assert_eq!(atoms[1].status, WorkStatus::Work(WorkBasis::UserAsserted));
        assert_eq!(
            atoms[0].manual_targets.iter().collect::<Vec<_>>(),
            vec![&design]
        );
        assert_eq!(
            atoms[1].manual_targets.iter().collect::<Vec<_>>(),
            vec![&meeting]
        );
    }

    #[test]
    fn a_manual_entry_overrides_inferred_and_unknown_time_and_corrections_override_it() {
        let cand = walkthrough_b();
        // Manual 09:20–09:40 over inferred A (5), unknown (7) and inferred B (8).
        let m = [entry(
            "m1",
            (9, 20),
            (9, 40),
            false,
            Some(TargetRef::Category("meeting".into())),
        )];
        let atoms = count(&cand, &m, &[]);
        assert_eq!(
            mins(&atoms),
            (87 + 7, 13, 0),
            "counted once, the unknown 7 becomes asserted"
        );
        // A later break correction wins over the manual entry for its overlap.
        let k = [c("k1", 1, (9, 30), (9, 40), CorrectionOp::MarkNonWork)];
        assert_eq!(mins(&count(&cand, &m, &k)), (87 + 7 - 10, 13, 10));
    }

    #[test]
    fn overlapping_manual_entries_count_once_and_conflict_on_targets() {
        let a = TargetRef::Category("meeting".into());
        let b = TargetRef::Category("support".into());
        let atoms = count(
            &[],
            &[
                entry("x", (10, 0), (11, 0), false, Some(a)),
                entry("y", (10, 30), (11, 30), false, Some(b)),
            ],
            &[],
        );
        assert_eq!(mins(&atoms), (90, 0, 0), "the overlap counts once");
        let overlap = atoms.iter().find(|x| x.start.0 == hm(10, 30)).unwrap();
        assert_eq!(
            overlap.manual_targets.len(),
            2,
            "two targets: conflicting until resolved"
        );
    }

    #[test]
    fn target_keys_round_trip_and_reject_unknown_categories() {
        for t in [
            TargetRef::Repository("github.com/o/r".into()),
            TargetRef::Category("meeting".into()),
            TargetRef::Other("Kunde X".into()),
        ] {
            assert_eq!(TargetRef::parse(&t.key()), Some(t));
        }
        assert_eq!(TargetRef::parse("category:gaming"), None);
        assert_eq!(TargetRef::parse("other:"), None);
    }
}
