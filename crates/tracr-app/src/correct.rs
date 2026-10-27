//! User corrections on the human timeline (architecture §6.5, time and
//! worklog §3). Each is one append-only `user_action` + `correction` pair,
//! anchored to a time range. Undo is a `revoke`; nothing is deleted.

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use tracr_core::counting::{Correction, CorrectionOp, TargetRef};
use tracr_core::time::UtcMillis;
use tracr_ingest::IngestEnv;
use tracr_ingest::source::new_uuid_v7;
use tracr_store::ledger::{self, CorrectionRow, NewCorrection, NewUserAction};

use crate::App;

/// A correction as hosts request it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionRequest {
    /// `confirm_work`, `mark_non_work`, `set_target`, `reject_target` or `revoke`.
    pub op: String,
    pub start: Option<i64>,
    pub end: Option<i64>,
    /// Target key for `set_target` / `reject_target` (`repository:…`, `category:…`, `other:…`).
    pub target: Option<String>,
    /// Correction ID for `revoke`.
    pub revokes: Option<String>,
    /// The UI gesture that produced it (e.g. `interval_panel`, `review_list`).
    pub ui_op: Option<String>,
    pub reason: Option<String>,
}

/// Parses a stored correction into the core type (`None` if unreadable).
pub fn from_row(r: &CorrectionRow) -> Option<Correction> {
    let data: serde_json::Value = r
        .data
        .as_deref()
        .and_then(|d| serde_json::from_str(d).ok())
        .unwrap_or(serde_json::Value::Null);
    let target = || {
        data.get("target")
            .and_then(|t| t.as_str())
            .and_then(TargetRef::parse)
    };
    let op = match r.op.as_str() {
        "confirm_work" => CorrectionOp::ConfirmWork,
        "mark_non_work" => CorrectionOp::MarkNonWork,
        "set_target" => CorrectionOp::SetTarget(target()?),
        "reject_target" => CorrectionOp::RejectTarget(target()?),
        "revoke" => CorrectionOp::Revoke(data.get("revokes")?.as_str()?.to_owned()),
        _ => return None,
    };
    Some(Correction {
        id: r.id.clone(),
        seq: r.action_seq,
        start: UtcMillis(r.range_start.unwrap_or(0)),
        end: UtcMillis(r.range_end.unwrap_or(0)),
        op,
    })
}

/// A correction checked and ready to append.
struct Prepared {
    axis: &'static str,
    range: Option<(i64, i64)>,
    data: Option<serde_json::Value>,
}

/// The shortest range a correction may cover: a split part under a minute is a slip, not a decision.
const MIN_RANGE_MS: i64 = 60_000;

fn prepare(existing: &[CorrectionRow], req: &CorrectionRequest) -> Result<Prepared> {
    let range = || -> Result<(i64, i64)> {
        match (req.start, req.end) {
            (Some(s), Some(e)) if e > s => Ok((s, e)),
            _ => bail!("a correction needs a range with end after start"),
        }
    };
    let (axis, range, data) = match req.op.as_str() {
        "confirm_work" | "mark_non_work" => ("counting", Some(range()?), None),
        "set_target" | "reject_target" => {
            let key = req
                .target
                .as_deref()
                .ok_or_else(|| anyhow!("{} needs a target", req.op))?;
            let t = TargetRef::parse(key).ok_or_else(|| anyhow!("not a valid target: {key}"))?;
            (
                "attribution",
                Some(range()?),
                Some(serde_json::json!({ "target": t.key() })),
            )
        }
        "revoke" => {
            let id = req
                .revokes
                .as_deref()
                .ok_or_else(|| anyhow!("revoke needs a correction ID"))?;
            let target = existing
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| anyhow!("no correction {id}"))?;
            // A revoke is recorded on the axis of what it undoes.
            (
                if target.axis == "attribution" {
                    "attribution"
                } else {
                    "counting"
                },
                None,
                Some(serde_json::json!({ "revokes": id })),
            )
        }
        other => bail!("unknown correction {other}"),
    };
    Ok(Prepared { axis, range, data })
}

impl App {
    /// Records one correction. Returns its ID.
    pub fn correct(&mut self, req: &CorrectionRequest) -> Result<String> {
        Ok(self.correct_batch(std::slice::from_ref(req))?.remove(0))
    }

    /// Records several corrections as one step: every request is checked first and all are appended in one
    /// transaction, or none is. Within a batch, ranges of one axis must not overlap (a split is parts, not layers)
    /// and none may be shorter than a minute. Returns the IDs in order.
    pub fn correct_batch(&mut self, reqs: &[CorrectionRequest]) -> Result<Vec<String>> {
        if reqs.is_empty() {
            bail!("nothing to record");
        }
        let existing = ledger::corrections(&self.conn)?;
        let prepared: Vec<Prepared> = reqs
            .iter()
            .map(|r| prepare(&existing, r))
            .collect::<Result<_>>()?;
        if reqs.len() > 1 {
            // The decisions of one kind (count or not / put on a target) are layers of the same range: in a batch
            // they must be disjoint parts, or the later one would silently hide the earlier.
            let layer = |op: &str| match op {
                "confirm_work" | "mark_non_work" => Some("count"),
                "set_target" => Some("target"),
                _ => None,
            };
            for (i, a) in prepared.iter().enumerate() {
                let Some((s, e)) = a.range else { continue };
                if layer(&reqs[i].op).is_some() && e - s < MIN_RANGE_MS {
                    bail!("a part must be at least a minute long");
                }
                for (j, b) in prepared.iter().enumerate().skip(i + 1) {
                    if let (Some(x), Some(y), Some((bs, be))) =
                        (layer(&reqs[i].op), layer(&reqs[j].op), b.range)
                        && x == y
                        && s < be
                        && bs < e
                    {
                        bail!("the parts of a split must not overlap");
                    }
                }
            }
        }
        let env = IngestEnv {
            clock: &*self.clock,
            file_identity: &*self.fid,
        };
        let tx = self.conn.transaction()?;
        let mut ids = Vec::with_capacity(reqs.len());
        for (req, p) in reqs.iter().zip(&prepared) {
            let id = new_uuid_v7(&env);
            let seq = ledger::append_user_action(
                &tx,
                &NewUserAction {
                    id: &new_uuid_v7(&env),
                    kind: &format!("correction.{}", req.op),
                    actor: "user",
                    at: self.clock.now(),
                    reason: req.reason.as_deref(),
                    client: self.client,
                },
            )?;
            let data = p.data.as_ref().map(ToString::to_string);
            ledger::append_correction(
                &tx,
                &NewCorrection {
                    id: &id,
                    action_seq: seq,
                    axis: p.axis,
                    op: &req.op,
                    ui_op: req.ui_op.as_deref(),
                    subject_kind: "human_range",
                    range_start: p.range.map(|r| r.0),
                    range_end: p.range.map(|r| r.1),
                    data: data.as_deref(),
                    against: None,
                },
            )?;
            ids.push(id);
        }
        tx.commit()?;
        Ok(ids)
    }
}
