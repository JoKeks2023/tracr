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

impl App {
    /// Records one correction. Returns its ID.
    pub fn correct(&mut self, req: &CorrectionRequest) -> Result<String> {
        let existing = ledger::corrections(&self.conn)?;
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
                let t =
                    TargetRef::parse(key).ok_or_else(|| anyhow!("not a valid target: {key}"))?;
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
                kind: &format!("correction.{}", req.op),
                actor: "user",
                at: self.clock.now(),
                reason: req.reason.as_deref(),
                client: self.client,
            },
        )?;
        let data = data.map(|d| d.to_string());
        ledger::append_correction(
            &tx,
            &NewCorrection {
                id: &id,
                action_seq: seq,
                axis,
                op: &req.op,
                ui_op: req.ui_op.as_deref(),
                subject_kind: "human_range",
                range_start: range.map(|r| r.0),
                range_end: range.map(|r| r.1),
                data: data.as_deref(),
                against: None,
            },
        )?;
        tx.commit()?;
        Ok(id)
    }
}
