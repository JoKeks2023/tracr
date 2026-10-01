//! Day review (architecture §7.6, time and worklog §5–§6, AD-18).
//!
//! Review stores the resolved day **by value** as a ledger revision. It
//! freezes counting: re-derivation never changes a revision. When the current
//! derivation differs from the latest revision, the day shows a
//! **discrepancy**. Accept creates the next revision; dismiss is recorded
//! against exactly that difference (its fingerprint), so a later, different
//! change shows again. A revision never changes automatically (TW-03).

use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use tracr_ingest::IngestEnv;
use tracr_ingest::source::new_uuid_v7;
use tracr_store::ledger::{self, NewUserAction, WorklogRevision};

use crate::App;
use crate::day::DayAgents;

/// One interval of a reviewed day, stored by value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewedInterval {
    pub start: i64,
    pub end: i64,
    /// `work`, `unknown` or `non_work`.
    pub status: String,
    pub duration_basis: String,
    pub unknown_reason: Option<String>,
    /// Report-target attribution as shown when reviewed (AD-18: stored as shown).
    pub attribution_state: Option<String>,
    pub target_keys: Vec<String>,
    pub target_labels: Vec<String>,
    pub engaged_session: Option<String>,
}

/// The reviewed day: intervals and totals, by value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub day: String,
    pub timezone: String,
    pub intervals: Vec<ReviewedInterval>,
    pub counted_ms: i64,
    pub unknown_ms: i64,
    pub non_work_ms: i64,
    pub counted_by_target: BTreeMap<String, i64>,
    pub counted_by_basis: BTreeMap<String, i64>,
    pub derived_as_of: Option<i64>,
}

/// The resolved day as it would be reviewed now.
pub fn snapshot_of(d: &DayAgents) -> Snapshot {
    let mut s = Snapshot {
        day: d.day.clone(),
        timezone: d.timezone.clone(),
        intervals: Vec::new(),
        counted_ms: d.human.counted_ms,
        unknown_ms: d.human.unknown_ms,
        non_work_ms: 0,
        counted_by_target: d.human.counted_by_target.clone(),
        counted_by_basis: BTreeMap::new(),
        derived_as_of: d.derived_as_of,
    };
    for a in &d.human.atoms {
        let len = a.end - a.start;
        match a.status.as_str() {
            "work" => {
                *s.counted_by_basis
                    .entry(a.duration_basis.clone())
                    .or_default() += len
            }
            "non_work" => s.non_work_ms += len,
            _ => {}
        }
        s.intervals.push(ReviewedInterval {
            start: a.start,
            end: a.end,
            status: a.status.clone(),
            duration_basis: a.duration_basis.clone(),
            unknown_reason: a.unknown_reason.clone(),
            attribution_state: a.attribution.as_ref().map(|x| x.state.clone()),
            target_keys: a
                .attribution
                .as_ref()
                .map(|x| x.targets.clone())
                .unwrap_or_default(),
            target_labels: a
                .attribution
                .as_ref()
                .map(|x| x.labels.clone())
                .unwrap_or_default(),
            engaged_session: a.engaged_session.clone(),
        });
    }
    s
}

/// Fingerprint of what review freezes: the intervals with their status,
/// basis and attribution. The derivation time is not part of it.
pub fn fingerprint(s: &Snapshot) -> String {
    let mut h = blake3::Hasher::new();
    for i in &s.intervals {
        let line = serde_json::to_string(&(
            i.start,
            i.end,
            &i.status,
            &i.duration_basis,
            &i.unknown_reason,
            &i.attribution_state,
            &i.target_keys,
        ))
        .expect("json");
        h.update(line.as_bytes());
        h.update(b"\n");
    }
    h.finalize().to_hex()[..32].to_owned()
}

/// The latest revision of a day with the current difference, if any.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewState {
    pub revision_id: String,
    pub rev: i64,
    pub at: i64,
    pub reason: Option<String>,
    pub snapshot: Snapshot,
}

/// New evidence (or a correction) differs from the reviewed revision.
#[derive(Debug, Clone, Serialize)]
pub struct Discrepancy {
    /// Fingerprint of the current derivation (what accept or dismiss acts on).
    pub fingerprint: String,
    pub counted_delta_ms: i64,
    pub unknown_delta_ms: i64,
    pub non_work_delta_ms: i64,
    /// The current derivation of the day (the draft).
    pub draft: Snapshot,
}

/// Compares the current derivation with the latest revision of the day.
pub fn compare(
    revision: Option<&WorklogRevision>,
    current: &Snapshot,
    dismissed: impl Fn(&str, &str) -> bool,
) -> Result<(Option<ReviewState>, Option<Discrepancy>)> {
    let Some(r) = revision else {
        return Ok((None, None));
    };
    let snapshot: Snapshot = serde_json::from_str(&r.snapshot)?;
    let fp = fingerprint(current);
    let discrepancy = (fp != r.fingerprint && !dismissed(&r.id, &fp)).then(|| Discrepancy {
        counted_delta_ms: current.counted_ms - snapshot.counted_ms,
        unknown_delta_ms: current.unknown_ms - snapshot.unknown_ms,
        non_work_delta_ms: current.non_work_ms - snapshot.non_work_ms,
        fingerprint: fp,
        draft: current.clone(),
    });
    Ok((
        Some(ReviewState {
            revision_id: r.id.clone(),
            rev: r.rev,
            at: r.at.0,
            reason: r.reason.clone(),
            snapshot,
        }),
        discrepancy,
    ))
}

impl App {
    /// The day as currently derived (read from the installed derived store).
    pub fn current_day(&self, day: &str) -> Result<DayAgents> {
        let derived = tracr_store::derived::open_read_only(&self.data_dir)?
            .ok_or_else(|| anyhow!("no derived store yet"))?;
        let setting = ledger::current_setting(&self.conn, crate::TIMEZONE_KEY)?;
        let zone = crate::day::ReportingZone::resolve(setting.as_ref().and_then(|v| v.as_str()))?;
        crate::day::day_agents(
            &derived,
            &zone,
            day,
            &ledger::corrections(&self.conn)?,
            &ledger::manual_entries(&self.conn)?,
        )
    }

    /// Reviews `day`: stores the current resolved day as the next revision.
    /// `reason` is required for every revision after the first.
    pub fn review_day(&mut self, day: &str, reason: Option<&str>) -> Result<String> {
        let current = snapshot_of(&self.current_day(day)?);
        let latest = ledger::latest_worklog_revision(&self.conn, day)?;
        let rev = latest.as_ref().map_or(1, |r| r.rev + 1);
        let reason = reason.map(str::trim).filter(|r| !r.is_empty());
        if rev > 1 && reason.is_none() {
            bail!("a new revision of a reviewed day needs a reason");
        }
        let env = IngestEnv {
            clock: &*self.clock,
            file_identity: &*self.fid,
        };
        let id = new_uuid_v7(&env);
        let tx = self.conn.transaction()?;
        let seq = ledger::append_user_action(
            &tx,
            &NewUserAction {
                id: &new_uuid_v7(&env),
                kind: "worklog.review",
                actor: "user",
                at: self.clock.now(),
                reason,
                client: self.client,
            },
        )?;
        ledger::append_worklog_revision(
            &tx,
            &WorklogRevision {
                id: id.clone(),
                day: day.to_owned(),
                rev,
                action_seq: seq,
                at: self.clock.now(),
                timezone: current.timezone.clone(),
                state: "reviewed".into(),
                reason: reason.map(str::to_owned),
                fingerprint: fingerprint(&current),
                snapshot: serde_json::to_string(&current)?,
            },
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Keeps the reviewed revision for exactly the current difference.
    pub fn dismiss_discrepancy(&mut self, day: &str, fingerprint_now: &str) -> Result<()> {
        let latest = ledger::latest_worklog_revision(&self.conn, day)?
            .ok_or_else(|| anyhow!("{day} was not reviewed"))?;
        let env = IngestEnv {
            clock: &*self.clock,
            file_identity: &*self.fid,
        };
        let tx = self.conn.transaction()?;
        let seq = ledger::append_user_action(
            &tx,
            &NewUserAction {
                id: &new_uuid_v7(&env),
                kind: "worklog.dismiss_discrepancy",
                actor: "user",
                at: self.clock.now(),
                reason: None,
                client: self.client,
            },
        )?;
        ledger::append_dismissal(
            &tx,
            &new_uuid_v7(&env),
            day,
            &latest.id,
            fingerprint_now,
            seq,
        )?;
        tx.commit()?;
        Ok(())
    }
}

/// Adds the latest revision and any discrepancy to a day view, and settles
/// whether the day is still provisional.
pub fn attach(ledger_conn: &rusqlite::Connection, v: &mut DayAgents) -> Result<()> {
    let revision = ledger::latest_worklog_revision(ledger_conn, &v.day)?;
    let current = snapshot_of(v);
    let (review, discrepancy) = compare(revision.as_ref(), &current, |rev, fp| {
        ledger::is_dismissed(ledger_conn, rev, fp).unwrap_or(false)
    })?;
    if let Some(r) = &review {
        v.quality
            .provisional_reasons
            .retain(|x| !x.starts_with("not reviewed"));
        if discrepancy.is_some() {
            v.quality.provisional_reasons.insert(
                0,
                format!("new evidence differs from reviewed revision r{}", r.rev),
            );
        }
        v.quality.provisional = discrepancy.is_some();
    }
    v.review = review;
    v.discrepancy = discrepancy;
    Ok(())
}
