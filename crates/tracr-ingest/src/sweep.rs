//! The stat sweep (§4.5, §4.7): `stat` every known, non-ended source. A
//! changed identity, size, mtime or path, or unread bytes left by an earlier
//! read (an unfinished read or a partial trailing line), triggers a
//! reconcile. One `sweep_completed` record lists the sources that were not
//! verified by this sweep; all others count as unchanged at its stat time.

use rusqlite::Connection;
use tracr_core::evidence::{CollectorFacts, IngestTrigger};
use tracr_core::ids::SourceId;
use tracr_store::ledger;

use crate::reconcile::{ReconcileOptions, ReconcileReport, locate, reconcile, record_failed_run};
use crate::source::{RunContext, write_collector_record};
use crate::{IngestEnv, IngestError, Registry, Result};

#[derive(Debug, Default)]
pub struct SweepReport {
    pub checked: u64,
    /// Sources not verified by this sweep: changed, missing or failed.
    pub changed: Vec<SourceId>,
    pub reconciles: Vec<ReconcileReport>,
    pub errors: Vec<(SourceId, IngestError)>,
}

pub fn sweep(
    conn: &mut Connection,
    env: &IngestEnv<'_>,
    registry: &Registry,
    run: &mut RunContext,
    opts: &ReconcileOptions,
) -> Result<SweepReport> {
    let stat_time = env.clock.now();
    let mut report = SweepReport::default();
    for source in ledger::sources(conn, None)? {
        if registry.for_kind(&source.kind).is_none() {
            continue;
        }
        let cursor = ledger::cursor(conn, &source.id)?;
        if cursor.as_ref().is_some_and(|c| c.ended_at.is_some()) {
            continue;
        }
        report.checked += 1;
        let changed = match &cursor {
            None => true,
            Some(c) => match locate(conn, env, &source, Some(c))? {
                None => true,
                Some(snap) => {
                    snap.identity.map(|i| i.0) != c.file_identity
                        || Some(snap.size) != c.size_at_read
                        || snap.mtime != c.mtime_at_read
                        || Some(snap.path.to_string_lossy().into_owned()) != c.path
                        || c.size_at_read.is_some_and(|size| c.byte_offset < size)
                }
            },
        };
        if !changed {
            continue;
        }
        report.changed.push(source.id.clone());
        match reconcile(
            conn,
            env,
            registry,
            run,
            &source,
            IngestTrigger::Sweep,
            opts,
        ) {
            Ok(r) => report.reconciles.push(r),
            Err(e) => {
                record_failed_run(conn, env, run, &source, IngestTrigger::Sweep, &e)?;
                report.errors.push((source.id.clone(), e));
            }
        }
    }
    let tx = conn.transaction()?;
    write_collector_record(
        &tx,
        env,
        run,
        CollectorFacts::SweepCompleted {
            stat_time,
            sources_checked: report.checked,
            changed_sources: report.changed.iter().map(ToString::to_string).collect(),
        },
        IngestTrigger::Sweep,
        None,
    )?;
    tx.commit()?;
    Ok(report)
}
