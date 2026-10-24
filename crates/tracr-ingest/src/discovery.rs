//! Discovery of transcript files under approved provider roots (§4.2).
//!
//! Only approved roots are walked (PS-02). Each file is offered to the
//! registered adapters; the one that claims it names its stream. Paths are
//! recorded as locators; a moved file keeps its source.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tracr_core::evidence::{CollectorFacts, IngestTrigger};
use tracr_core::ids::MachineId;
use tracr_platform::created_at;
use tracr_store::ledger::{self, LocatorRow, NewSource};

use crate::source::{RunContext, new_uuid_v7, write_collector_record};
use crate::{IngestEnv, Registry, Result};

/// Directory depth limit below a root (workflow agents sit at depth 6).
const MAX_DEPTH: usize = 12;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryReport {
    pub roots: Vec<PathBuf>,
    pub missing_roots: Vec<PathBuf>,
    pub files_seen: u64,
    pub files_claimed: u64,
    pub files_unclaimed: u64,
    pub new_sources: u64,
    pub unreadable_dirs: u64,
}

/// Collects `*.jsonl` and `*.jsonl.zst` files below `dir`. Unreadable directories are skipped
/// and counted: one unreadable directory never aborts discovery.
fn walk(
    dir: &Path,
    rel: &mut Vec<String>,
    out: &mut Vec<(PathBuf, Vec<String>)>,
    unreadable: &mut u64,
) {
    let mut entries: Vec<fs::DirEntry> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(_) => {
            *unreadable += 1;
            return;
        }
    };
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() && rel.len() < MAX_DEPTH {
            rel.push(name);
            walk(&entry.path(), rel, out, unreadable);
            rel.pop();
        } else if ft.is_file() && (name.ends_with(".jsonl") || name.ends_with(".jsonl.zst")) {
            let mut r = rel.clone();
            r.push(name);
            out.push((entry.path(), r));
        }
    }
}

pub fn discover(
    conn: &mut Connection,
    env: &IngestEnv<'_>,
    registry: &Registry,
    run: &mut RunContext,
    machine: &MachineId,
    roots: &[PathBuf],
    trigger: IngestTrigger,
) -> Result<DiscoveryReport> {
    let now = env.clock.now();
    let mut report = DiscoveryReport {
        roots: roots.to_vec(),
        ..DiscoveryReport::default()
    };
    let tx = conn.transaction()?;
    for root in roots {
        if !root.is_dir() {
            report.missing_roots.push(root.clone());
            continue;
        }
        let mut files = Vec::new();
        walk(
            root,
            &mut Vec::new(),
            &mut files,
            &mut report.unreadable_dirs,
        );
        for (path, rel) in files {
            report.files_seen += 1;
            let rel_refs: Vec<&str> = rel.iter().map(String::as_str).collect();
            let Some((adapter, stream)) = registry
                .adapters()
                .iter()
                .find_map(|a| a.classify_path(machine, &rel_refs).map(|s| (a.clone(), s)))
            else {
                report.files_unclaimed += 1;
                continue;
            };
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            report.files_claimed += 1;
            let path_text = path.to_string_lossy().into_owned();
            let (row, created) = ledger::ensure_source(
                &tx,
                &NewSource {
                    id: &new_uuid_v7(env),
                    kind: adapter.source_kind().as_str(),
                    machine_id: machine,
                    stream_key: stream.stream_key.as_str(),
                    adapter: adapter.manifest().adapter,
                    adapter_version: adapter.adapter_version(),
                    created_at: now,
                    stream_meta: Some(&serde_json::to_string(&stream)?),
                },
            )?;
            if created {
                report.new_sources += 1;
            } else if let Some(c) = ledger::cursor(&tx, &row.id)?
                && let Some(current) = c.path.as_deref()
                && current != path_text
                && Path::new(current).is_file()
            {
                // Two live files claim one stream: keep reading the current one, record the other.
                write_collector_record(
                    &tx,
                    env,
                    run,
                    CollectorFacts::LocatorConflict {
                        source_id: row.id.to_string(),
                        kept_path: current.to_owned(),
                        other_path: path_text.clone(),
                    },
                    trigger,
                    Some(format!("locator_conflict:{}:{path_text}", row.id)),
                )?;
            }
            ledger::upsert_locator(
                &tx,
                &LocatorRow {
                    source_id: row.id.clone(),
                    path: path_text,
                    file_identity: env.file_identity.identity(&meta).map(|i| i.0),
                    birth_time: created_at(&meta),
                    first_seen: Some(now),
                    last_seen: Some(now),
                },
            )?;
        }
    }
    write_collector_record(
        &tx,
        env,
        run,
        CollectorFacts::DiscoveryCompleted {
            roots: roots
                .iter()
                .map(|r| r.to_string_lossy().into_owned())
                .collect(),
            missing_roots: report
                .missing_roots
                .iter()
                .map(|r| r.to_string_lossy().into_owned())
                .collect(),
            files_seen: report.files_seen,
            files_claimed: report.files_claimed,
            files_unclaimed: report.files_unclaimed,
            new_sources: report.new_sources,
            unreadable_dirs: report.unreadable_dirs,
        },
        trigger,
        None,
    )?;
    tx.commit()?;
    Ok(report)
}
