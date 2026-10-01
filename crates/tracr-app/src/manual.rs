//! Manual entries and the timer (time and worklog §3–§4, AD-15).
//!
//! Every change is a new revision of the entry (`manual_entry_rev`,
//! append-only). At most one timer runs: starting a timer stops a running
//! one at the same instant. Start and stop persist immediately; elapsed time
//! is stop − start. Idle and lock never end a manual entry.

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use tracr_core::counting::TargetRef;
use tracr_core::evidence::context_text;
use tracr_ingest::IngestEnv;
use tracr_ingest::source::new_uuid_v7;
use tracr_store::ledger::{self, ManualRev, NewUserAction};

use crate::App;

/// A manual-entry operation as hosts request it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualRequest {
    /// `start_timer`, `stop_timer`, `add`, `edit` or `delete`.
    pub op: String,
    /// Entry ID for `edit` and `delete`.
    pub id: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
    /// Target key (`repository:…`, `category:…`, `other:…`); empty = none.
    pub target: Option<String>,
    pub note: Option<String>,
}

fn clean_target(t: &Option<String>) -> Result<Option<String>> {
    match t.as_deref().filter(|t| !t.is_empty()) {
        None => Ok(None),
        Some(k) => TargetRef::parse(k)
            .map(|t| Some(t.key()))
            .ok_or_else(|| anyhow!("not a valid target: {k}")),
    }
}

fn clean_note(n: &Option<String>) -> Result<Option<String>> {
    match n.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        None => Ok(None),
        Some(n) => context_text(n, 200)
            .map(|n| Some(n.to_owned()))
            .ok_or_else(|| anyhow!("a note is one line of at most 200 characters")),
    }
}

impl App {
    /// The running timer, if any.
    pub fn running_timer(&self) -> Result<Option<ManualRev>> {
        Ok(ledger::manual_entries(&self.conn)?
            .into_iter()
            .find(|m| m.kind == "timer" && m.end_at.is_none() && !m.deleted))
    }

    /// Applies one manual-entry operation. Returns the entry ID.
    pub fn manual(&mut self, req: &ManualRequest) -> Result<String> {
        let now = self.clock.now().0;
        let current = ledger::manual_entries(&self.conn)?;
        let running = current
            .iter()
            .find(|m| m.kind == "timer" && m.end_at.is_none() && !m.deleted)
            .cloned();
        let env = IngestEnv {
            clock: &*self.clock,
            file_identity: &*self.fid,
        };
        let mut revs: Vec<ManualRev> = Vec::new();
        let id = match req.op.as_str() {
            "start_timer" => {
                if let Some(r) = &running {
                    // At most one running timer (AD-15): the old one stops now.
                    revs.push(ManualRev {
                        rev: r.rev + 1,
                        end_at: Some(now),
                        ..r.clone()
                    });
                }
                let id = new_uuid_v7(&env);
                revs.push(ManualRev {
                    entry_id: id.clone(),
                    rev: 1,
                    action_seq: 0,
                    kind: "timer".into(),
                    start_at: now,
                    end_at: None,
                    target: clean_target(&req.target)?,
                    note: clean_note(&req.note)?,
                    deleted: false,
                });
                id
            }
            "stop_timer" => {
                let r = running.ok_or_else(|| anyhow!("no timer is running"))?;
                revs.push(ManualRev {
                    rev: r.rev + 1,
                    end_at: Some(now.max(r.start_at + 1)),
                    ..r.clone()
                });
                r.entry_id
            }
            "add" => {
                let (Some(s), Some(e)) = (req.start, req.end) else {
                    bail!("an entry needs a start and an end")
                };
                if e <= s {
                    bail!("the end must be after the start");
                }
                let id = new_uuid_v7(&env);
                revs.push(ManualRev {
                    entry_id: id.clone(),
                    rev: 1,
                    action_seq: 0,
                    kind: "retroactive".into(),
                    start_at: s,
                    end_at: Some(e),
                    target: clean_target(&req.target)?,
                    note: clean_note(&req.note)?,
                    deleted: false,
                });
                id
            }
            "edit" | "delete" => {
                let id = req
                    .id
                    .as_deref()
                    .ok_or_else(|| anyhow!("{} needs an entry ID", req.op))?;
                let r = current
                    .iter()
                    .find(|m| m.entry_id == id && !m.deleted)
                    .ok_or_else(|| anyhow!("no entry {id}"))?;
                let mut next = ManualRev {
                    rev: r.rev + 1,
                    ..r.clone()
                };
                if req.op == "delete" {
                    next.deleted = true;
                } else {
                    next.start_at = req.start.unwrap_or(r.start_at);
                    next.end_at = match (req.end, r.end_at) {
                        (Some(e), _) => Some(e),
                        (None, e) => e,
                    };
                    if next.end_at.is_some_and(|e| e <= next.start_at) {
                        bail!("the end must be after the start");
                    }
                    next.target = clean_target(&req.target)?;
                    next.note = clean_note(&req.note)?;
                }
                revs.push(next);
                id.to_owned()
            }
            other => bail!("unknown manual operation {other}"),
        };
        let tx = self.conn.transaction()?;
        let seq = ledger::append_user_action(
            &tx,
            &NewUserAction {
                id: &new_uuid_v7(&env),
                kind: &format!("manual.{}", req.op),
                actor: "user",
                at: self.clock.now(),
                reason: None,
                client: self.client,
            },
        )?;
        for mut r in revs {
            r.action_seq = seq;
            ledger::append_manual_rev(&tx, &r)?;
        }
        tx.commit()?;
        Ok(id)
    }
}
