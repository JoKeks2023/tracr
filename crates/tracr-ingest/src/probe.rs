//! Execution of probe adapters (architecture §9): runs the planned read-only
//! commands in an observed working directory **only** if it lies under an
//! approved repository root (PS-02), and records the parsed result as an
//! observation with time quality `receipt`. A result equal to the stream's
//! latest one is not recorded again.

use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::Connection;
use tracr_core::adapter::{ProbeAdapter, ProbeOutput};
use tracr_core::evidence::{Channel, IngestTrigger, TimeQuality};
use tracr_core::ids::{ContentHash, MachineId};
use tracr_store::ledger::{self, NewSource};

use crate::source::{RunContext, insert_record_with_quality, new_uuid_v7};
use crate::{IngestEnv, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeOutcome {
    Recorded,
    Unchanged,
    /// The directory is outside every approved repository root: nothing ran.
    NotApproved,
    /// The directory no longer exists or the program could not be started.
    Unavailable,
}

pub fn is_approved(cwd: &Path, roots: &[PathBuf]) -> bool {
    cwd.is_absolute() && roots.iter().any(|r| cwd.starts_with(r))
}

/// Probes `cwd` with `adapter` and records the result.
#[allow(clippy::too_many_arguments)]
pub fn probe(
    conn: &Connection,
    env: &IngestEnv<'_>,
    run: &mut RunContext,
    adapter: &dyn ProbeAdapter,
    machine: &MachineId,
    cwd: &str,
    approved_roots: &[PathBuf],
    trigger: IngestTrigger,
) -> Result<ProbeOutcome> {
    let dir = Path::new(cwd);
    if !is_approved(dir, approved_roots) {
        return Ok(ProbeOutcome::NotApproved);
    }
    if !dir.is_dir() {
        return Ok(ProbeOutcome::Unavailable);
    }
    let mut outputs = Vec::new();
    for args in adapter.probes() {
        let mut cmd = Command::new(adapter.program());
        cmd.args(*args).current_dir(dir);
        for (k, v) in adapter.env() {
            cmd.env(k, v);
        }
        match cmd.output() {
            Ok(out) => outputs.push(ProbeOutput {
                success: out.status.success(),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            }),
            Err(_) => return Ok(ProbeOutcome::Unavailable),
        }
    }
    let facts = adapter.parse(cwd, machine.as_str(), &outputs);
    let facts_json = serde_json::to_string(&facts)?;

    let (source, _) = ledger::ensure_source(
        conn,
        &NewSource {
            id: &new_uuid_v7(env),
            kind: adapter.source_kind().as_str(),
            machine_id: machine,
            stream_key: &format!("{}:{machine}:{cwd}", adapter.source_kind().as_str()),
            adapter: "tracr-git",
            adapter_version: adapter.adapter_version(),
            created_at: env.clock.now(),
            stream_meta: None,
        },
    )?;
    if ledger::latest_facts_json(conn, &source.id)?.as_deref() == Some(facts_json.as_str()) {
        return Ok(ProbeOutcome::Unchanged);
    }
    let now = env.clock.now();
    let key = ContentHash::of(format!("{facts_json}@{}", now.0).as_bytes()).to_hex();
    insert_record_with_quality(
        conn,
        env,
        run,
        &source.id,
        Channel::Git,
        facts,
        adapter.fact_kind(),
        trigger,
        Some(now),
        Some(key),
        None,
        TimeQuality::Receipt,
    )?;
    Ok(ProbeOutcome::Recorded)
}
