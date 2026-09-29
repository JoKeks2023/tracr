//! `reconcile(source)`: the only way a file source is read (§4.1, §4.3, §4.4).
//!
//! * The file is opened once; identity, size and mtime come from that handle.
//! * Continuity holds → read complete lines from the cursor offset.
//! * Continuity broken → re-read the whole file. If the last ingested
//!   record (key and content hash) is not in it, a coverage gap is committed
//!   **in the same transaction** as the cursor reset and the first batch.
//! * Observations are inserted with `INSERT OR IGNORE` on
//!   `(source, record key, content hash)`; the cursor and the occurrence
//!   numbering of content-derived keys advance in the same transaction as the
//!   lines they cover.
//! * A trailing partial line is never consumed; it is recorded as an
//!   `incomplete_trailing` diagnostic and ingested once it is complete.
//! * A source whose file is gone is ended only when the caller confirms that
//!   discovery found no other locator (§4.3); otherwise it is only `missing`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use rusqlite::{Connection, Transaction};
use tracr_core::adapter::{ExtractContext, ObservationAdapter, StreamRef};
use tracr_core::events::FactKind;
use tracr_core::evidence::{
    Channel, CollectorFacts, ExtractOutcome, Facts, IngestTrigger, NativeIds, ParseStatus,
    RetentionClass, TimeQuality,
};
use tracr_core::ids::{ContentHash, RecordKey, SourceId, strip_line_terminator};
use tracr_core::time::UtcMillis;
use tracr_store::ledger::{self, InsertOutcome, NewObservation, SourceCursor, SourceRow};

use crate::cursor::{
    Break, Continuity, FileSnapshot, check_continuity, head_fingerprint, snapshot, snapshot_of_open,
};
use crate::reader::{Line, LineReader};
use crate::source::{RunContext, new_uuid_v7, stream_ref, write_collector_record};
use crate::{IngestEnv, IngestError, Registry, Result};

#[derive(Debug, Clone)]
pub struct ReconcileOptions {
    /// Lines per transaction (§4.4: up to 1,000).
    pub batch_size: usize,
    /// The caller ran discovery in this run and it found no other locator
    /// for missing files, so a missing file ends its source (§4.3). Without
    /// that confirmation a missing file only makes the source `missing`.
    pub discovery_confirmed: bool,
    /// Test hook: fail as if the process crashed right after this many
    /// committed batches.
    pub fail_after_batches: Option<usize>,
}

impl Default for ReconcileOptions {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            discovery_confirmed: false,
            fail_after_batches: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadMode {
    /// No cursor yet: read from offset 0.
    First,
    /// Continuity holds: read from the cursor offset.
    Tail,
    /// Continuity broken: re-read the whole file.
    FullReread,
    /// No file, and discovery confirmed there is no other locator: the source ended.
    Gone,
    /// No file, but discovery has not confirmed it yet: nothing is ended.
    Missing,
    /// Already ended and still gone.
    AlreadyEnded,
}

impl ReadMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Tail => "tail",
            Self::FullReread => "full_reread",
            Self::Gone => "gone",
            Self::Missing => "missing",
            Self::AlreadyEnded => "already_ended",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapInfo {
    pub start: UtcMillis,
    pub end: UtcMillis,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct ReconcileReport {
    pub source_id: SourceId,
    pub trigger: IngestTrigger,
    pub mode: ReadMode,
    pub continuity: String,
    pub from_offset: u64,
    pub to_offset: u64,
    pub reached_eof: bool,
    pub inserted: u64,
    pub duplicates: u64,
    pub quarantined: u64,
    pub key_conflicts: u64,
    pub ignored_by_type: BTreeMap<String, u64>,
    pub inserted_by_kind: BTreeMap<String, u64>,
    pub unvalidated_versions: BTreeSet<String>,
    pub gap: Option<GapInfo>,
    pub incomplete_trailing: bool,
    pub ended: bool,
}

impl ReconcileReport {
    fn new(source: &SourceRow, trigger: IngestTrigger, mode: ReadMode, continuity: &str) -> Self {
        Self {
            source_id: source.id.clone(),
            trigger,
            mode,
            continuity: continuity.to_owned(),
            from_offset: 0,
            to_offset: 0,
            reached_eof: false,
            inserted: 0,
            duplicates: 0,
            quarantined: 0,
            key_conflicts: 0,
            ignored_by_type: BTreeMap::new(),
            inserted_by_kind: BTreeMap::new(),
            unvalidated_versions: BTreeSet::new(),
            gap: None,
            incomplete_trailing: false,
            ended: false,
        }
    }

    pub fn ignored_total(&self) -> u64 {
        self.ignored_by_type.values().sum()
    }
}

/// Finds the file currently holding the source: the cursor's path if it
/// still exists, else the most recently seen locator that exists.
pub fn locate(
    conn: &Connection,
    env: &IngestEnv<'_>,
    source: &SourceRow,
    cursor: Option<&SourceCursor>,
) -> Result<Option<FileSnapshot>> {
    let mut candidates: Vec<String> = cursor.and_then(|c| c.path.clone()).into_iter().collect();
    for l in ledger::locators(conn, &source.id)? {
        if !candidates.contains(&l.path) {
            candidates.push(l.path);
        }
    }
    for p in candidates {
        if let Some(snap) = snapshot(Path::new(&p), env.file_identity)? {
            return Ok(Some(snap));
        }
    }
    Ok(None)
}

fn empty_cursor(source: &SourceRow, now: UtcMillis) -> SourceCursor {
    SourceCursor {
        source_id: source.id.clone(),
        path: None,
        file_identity: None,
        byte_offset: 0,
        size_at_read: None,
        mtime_at_read: None,
        head_fp: None,
        head_len: None,
        last_line_fp: None,
        last_record_key: None,
        last_record_hash: None,
        last_record_at: None,
        last_read_at: None,
        ended_at: None,
        updated_at: now,
    }
}

pub fn reconcile(
    conn: &mut Connection,
    env: &IngestEnv<'_>,
    registry: &Registry,
    run: &mut RunContext,
    source: &SourceRow,
    trigger: IngestTrigger,
    opts: &ReconcileOptions,
) -> Result<ReconcileReport> {
    let adapter = registry
        .for_kind(&source.kind)
        .ok_or_else(|| IngestError::NoAdapter(source.kind.clone()))?
        .clone();
    let stream = stream_ref(source)?;
    let cursor = ledger::cursor(conn, &source.id)?;
    let Some(located) = locate(conn, env, source, cursor.as_ref())? else {
        return file_missing(conn, env, run, source, cursor, trigger, opts);
    };
    // Open once; judge identity, size and mtime on this handle (no TOCTOU).
    let file = match File::open(&located.path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return file_missing(conn, env, run, source, cursor, trigger, opts);
        }
        Err(e) => return Err(e.into()),
    };
    let snap = snapshot_of_open(&located.path, &file, env.file_identity)?;
    let mut probe = file.try_clone()?;
    let (mode, brk) = match &cursor {
        None => (ReadMode::First, None),
        Some(c) => match check_continuity(c, &snap, &mut probe)? {
            Continuity::Holds => (ReadMode::Tail, None),
            Continuity::Broken(b) => (ReadMode::FullReread, Some(b)),
        },
    };
    let continuity = match (mode, brk) {
        (ReadMode::First, _) => "first_read",
        (_, Some(b)) => b.as_str(),
        _ => "holds",
    };
    let mut report = ReconcileReport::new(source, trigger, mode, continuity);

    // History check for a broken continuity (pass 1, before anything commits).
    let gap = match (brk, &cursor) {
        (Some(b), Some(c)) => decide_gap(conn, env, &adapter, &stream, source, c, &file, b)?,
        _ => None,
    };
    report.gap = gap.clone();

    let start = match (mode, &cursor) {
        (ReadMode::Tail, Some(c)) => c.byte_offset,
        _ => 0,
    };
    report.from_offset = start;
    let (head_fp, head_len) = head_fingerprint(&mut probe, snap.size)?;
    let now = env.clock.now();
    let mut next = match (&cursor, mode) {
        (Some(c), ReadMode::Tail) => c.clone(),
        (Some(c), _) => SourceCursor {
            byte_offset: 0,
            last_line_fp: None,
            // Keep the last known record until the re-read overwrites it.
            ..c.clone()
        },
        (None, _) => empty_cursor(source, now),
    };
    let reappeared = next.ended_at.is_some();
    next.path = Some(snap.path.to_string_lossy().into_owned());
    next.file_identity = snap.identity.as_ref().map(|i| i.0.clone());
    next.mtime_at_read = snap.mtime;
    next.head_fp = Some(head_fp);
    next.head_len = Some(head_len);
    next.ended_at = None;

    let locator = next.path.clone();
    let mut stream_version = if mode == ReadMode::Tail {
        ledger::latest_producer_version(conn, &source.id)?
    } else {
        None
    };
    let mut occurrences = Occurrences::new(mode == ReadMode::Tail);
    let mut reader = LineReader::new(file.try_clone()?, start)?;
    let mut pending_gap = gap;
    let mut batches = 0usize;
    let mut line_ctx = LineCtx {
        env,
        adapter: &adapter,
        stream: &stream,
        source,
        trigger,
        locator: locator.as_deref(),
    };

    loop {
        let tx = conn.transaction()?;
        if batches == 0 {
            if mode != ReadMode::Tail {
                // A new file generation: occurrence numbering restarts in file order.
                ledger::reset_occurrences(&tx, &source.id)?;
            }
            if let Some(g) = pending_gap.take() {
                write_collector_record(
                    &tx,
                    env,
                    run,
                    CollectorFacts::CoverageGap {
                        source_id: source.id.to_string(),
                        start: g.start,
                        end: g.end,
                        reason: g.reason.clone(),
                    },
                    trigger,
                    None,
                )?;
            }
            if reappeared {
                write_collector_record(
                    &tx,
                    env,
                    run,
                    CollectorFacts::SourceReappeared {
                        source_id: source.id.to_string(),
                        path: locator.clone().unwrap_or_default(),
                    },
                    trigger,
                    None,
                )?;
            }
        }
        let mut done = false;
        let mut lines = 0usize;
        while lines < opts.batch_size.max(1) {
            match reader.next_line()? {
                Line::Eof => {
                    done = true;
                    break;
                }
                Line::Partial { start: at, bytes } => {
                    let hash = ContentHash::of(&bytes);
                    write_collector_record(
                        &tx,
                        env,
                        run,
                        CollectorFacts::IncompleteTrailing {
                            source_id: source.id.to_string(),
                            offset: at,
                            byte_len: bytes.len() as u64,
                            hash: hash.to_hex(),
                        },
                        trigger,
                        Some(format!(
                            "incomplete_trailing:{}:{at}:{}",
                            source.id,
                            hash.to_hex()
                        )),
                    )?;
                    report.incomplete_trailing = true;
                    done = true;
                    break;
                }
                Line::Complete { start: at, bytes } => {
                    lines += 1;
                    process_line(
                        &tx,
                        &mut line_ctx,
                        run,
                        at,
                        &bytes,
                        &mut next,
                        &mut report,
                        &mut occurrences,
                        &mut stream_version,
                    )?;
                    next.byte_offset = at + bytes.len() as u64;
                    next.last_line_fp = Some(ContentHash::of(&bytes));
                }
            }
        }
        occurrences.flush(&tx, &source.id)?;
        // Bytes known at this read, including a partial line (file-gone rule, sweep).
        next.size_at_read = Some(snap.size.max(reader.position()).max(next.byte_offset));
        next.last_read_at = Some(env.clock.now());
        next.updated_at = env.clock.now();
        ledger::put_cursor(&tx, &next)?;
        tx.commit()?;
        batches += 1;
        if opts.fail_after_batches == Some(batches) {
            return Err(IngestError::SimulatedCrash(batches));
        }
        if done {
            break;
        }
    }
    report.reached_eof = true;
    report.to_offset = next.byte_offset;

    if !trigger.is_live() || report.gap.is_some() || mode == ReadMode::FullReread {
        let tx = conn.transaction()?;
        write_run_record(&tx, env, run, &report, None)?;
        tx.commit()?;
    }
    Ok(report)
}

/// Occurrence numbering of content-derived keys (`h:<hash>#<n>`) in file
/// order. Tail reads continue from the persisted counts; first reads and
/// full re-reads start from 0. Counts are flushed with each batch.
struct Occurrences {
    counts: HashMap<ContentHash, u32>,
    dirty: HashSet<ContentHash>,
    seed_from_ledger: bool,
}

impl Occurrences {
    fn new(seed_from_ledger: bool) -> Self {
        Self {
            counts: HashMap::new(),
            dirty: HashSet::new(),
            seed_from_ledger,
        }
    }

    fn next(&mut self, conn: &Connection, source: &SourceId, hash: &ContentHash) -> Result<u32> {
        if !self.counts.contains_key(hash) {
            let seed = if self.seed_from_ledger {
                ledger::occurrences_seen(conn, source, hash)?
            } else {
                0
            };
            self.counts.insert(*hash, seed);
        }
        let n = self.counts.get_mut(hash).expect("seeded");
        let current = *n;
        *n += 1;
        self.dirty.insert(*hash);
        Ok(current)
    }

    fn flush(&mut self, conn: &Connection, source: &SourceId) -> Result<()> {
        for hash in self.dirty.drain() {
            ledger::put_occurrences_seen(conn, source, &hash, self.counts[&hash])?;
        }
        Ok(())
    }
}

struct LineCtx<'a> {
    env: &'a IngestEnv<'a>,
    adapter: &'a Arc<dyn ObservationAdapter>,
    stream: &'a StreamRef,
    source: &'a SourceRow,
    trigger: IngestTrigger,
    locator: Option<&'a str>,
}

/// What one extracted line will be stored as.
struct Prepared {
    key: RecordKey,
    hash: ContentHash,
    native: NativeIds,
    occurred_at: Option<UtcMillis>,
    time_quality: TimeQuality,
    version: Option<String>,
    kind: FactKind,
    subtype: Option<String>,
    facts: Facts,
    parse_status: ParseStatus,
}

enum Extracted {
    Keep(Box<Prepared>),
    Ignore(String),
}

fn prepare(
    conn: &Connection,
    adapter: &Arc<dyn ObservationAdapter>,
    stream: &StreamRef,
    source: &SourceId,
    raw: &[u8],
    occurrences: &mut Occurrences,
) -> Result<Extracted> {
    let line = strip_line_terminator(raw);
    if line.iter().all(u8::is_ascii_whitespace) {
        return Ok(Extracted::Ignore("(blank_line)".into()));
    }
    let hash = ContentHash::of(line);
    let (record_id, p) = match adapter.extract(line, &ExtractContext { stream }) {
        ExtractOutcome::Ignore { record_type } => return Ok(Extracted::Ignore(record_type)),
        ExtractOutcome::Record(r) => {
            let r = *r;
            (
                r.native_record_id,
                Prepared {
                    key: RecordKey::native(""),
                    hash,
                    native: r.native,
                    occurred_at: r.occurred_at,
                    time_quality: r.time_quality,
                    version: r.producer_version,
                    kind: r.kind,
                    subtype: r.subtype,
                    facts: r.facts,
                    parse_status: ParseStatus::Ok,
                },
            )
        }
        ExtractOutcome::Quarantine(q) => {
            let q = *q;
            (
                q.native_record_id,
                Prepared {
                    key: RecordKey::native(""),
                    hash,
                    native: q.native,
                    occurred_at: q.occurred_at,
                    time_quality: if q.occurred_at.is_some() {
                        TimeQuality::Source
                    } else {
                        TimeQuality::Receipt
                    },
                    version: q.producer_version,
                    kind: FactKind::Quarantined,
                    subtype: None,
                    facts: Facts {
                        quarantine: Some(q.facts),
                        ..Facts::default()
                    },
                    parse_status: q.parse_status,
                },
            )
        }
    };
    let key = match record_id {
        Some(id) => RecordKey::native(&id),
        None => RecordKey::content_derived(&hash, occurrences.next(conn, source, &hash)?),
    };
    Ok(Extracted::Keep(Box::new(Prepared { key, ..p })))
}

#[allow(clippy::too_many_arguments)]
fn process_line(
    tx: &Transaction<'_>,
    ctx: &mut LineCtx<'_>,
    run: &mut RunContext,
    offset: u64,
    raw: &[u8],
    cursor: &mut SourceCursor,
    report: &mut ReconcileReport,
    occurrences: &mut Occurrences,
    stream_version: &mut Option<String>,
) -> Result<()> {
    let p = match prepare(
        tx,
        ctx.adapter,
        ctx.stream,
        &ctx.source.id,
        raw,
        occurrences,
    )? {
        Extracted::Ignore(t) => {
            *report.ignored_by_type.entry(t).or_default() += 1;
            return Ok(());
        }
        Extracted::Keep(p) => *p,
    };
    // AD-19: a record's producer version is its stream's version.
    if let Some(v) = &p.version {
        *stream_version = Some(v.clone());
    }
    let version = p.version.clone().or_else(|| stream_version.clone());
    let manifest_match = ctx.adapter.manifest().select(version.as_deref());
    if !manifest_match.is_validated() {
        report
            .unvalidated_versions
            .insert(version.clone().unwrap_or_else(|| "(none)".into()));
    }
    let facts_json = serde_json::to_string(&p.facts)?;
    let id = new_uuid_v7(ctx.env);
    let receipt = run.next_receipt();
    let range = manifest_match.label();
    let outcome = ledger::insert_observation(
        tx,
        &NewObservation {
            id: &id,
            source_id: &ctx.source.id,
            record_key: p.key.as_str(),
            content_hash: &p.hash,
            channel: Channel::Transcript,
            locator_path: ctx.locator,
            cursor_offset: Some(offset),
            producer: Some(ctx.adapter.producer()),
            producer_version: version.as_deref(),
            adapter_version: ctx.adapter.adapter_version(),
            manifest_range: Some(&range),
            native: &p.native,
            occurred_at: p.occurred_at,
            observed_at: ctx.env.clock.now(),
            collector_run_id: &run.run_id,
            receipt_seq: receipt,
            ingest_trigger: ctx.trigger,
            parse_status: p.parse_status,
            retention_class: RetentionClass::EvidenceMetadata,
            extractor_version: ctx.adapter.manifest().extractor_version,
            kind: p.kind.as_str(),
            subtype: p.subtype.as_deref(),
            time_quality: p.time_quality,
            facts_json: &facts_json,
        },
    )?;
    match outcome {
        InsertOutcome::Inserted { .. } => {
            report.inserted += 1;
            *report
                .inserted_by_kind
                .entry(p.kind.as_str().to_owned())
                .or_default() += 1;
            if p.parse_status != ParseStatus::Ok {
                report.quarantined += 1;
            }
            if !p.key.is_content_derived()
                && let Some(previous) =
                    ledger::conflicting_hash(tx, &ctx.source.id, p.key.as_str(), &p.hash)?
            {
                report.key_conflicts += 1;
                write_collector_record(
                    tx,
                    ctx.env,
                    run,
                    CollectorFacts::KeyContentConflict {
                        source_id: ctx.source.id.to_string(),
                        record_key: p.key.as_str().to_owned(),
                        previous_hash: previous.to_hex(),
                        new_hash: p.hash.to_hex(),
                    },
                    ctx.trigger,
                    Some(format!(
                        "key_conflict:{}:{}:{}",
                        ctx.source.id,
                        p.key.as_str(),
                        p.hash.to_hex()
                    )),
                )?;
            }
        }
        InsertOutcome::Duplicate => report.duplicates += 1,
    }
    cursor.last_record_key = Some(p.key.as_str().to_owned());
    cursor.last_record_hash = Some(p.hash);
    if let Some(at) = p.occurred_at {
        cursor.last_record_at = Some(at);
    }
    Ok(())
}

/// Pass 1 of a full re-read (§4.3): does the new content contain the last
/// ingested record (key **and** content hash)? If no record was ingested
/// yet, history cannot be proven. If it is not proven, evidence may be
/// missing: the gap runs from the last record's time (else the last read) to
/// the earliest record whose key is new (else now).
#[allow(clippy::too_many_arguments)]
fn decide_gap(
    conn: &Connection,
    env: &IngestEnv<'_>,
    adapter: &Arc<dyn ObservationAdapter>,
    stream: &StreamRef,
    source: &SourceRow,
    cursor: &SourceCursor,
    file: &File,
    brk: Break,
) -> Result<Option<GapInfo>> {
    let record_anchor = cursor
        .last_record_key
        .as_deref()
        .zip(cursor.last_record_hash);
    let start = cursor
        .last_record_at
        .or(cursor.last_read_at)
        .unwrap_or(cursor.updated_at);
    let mut reader = LineReader::new(file.try_clone()?, 0)?;
    let mut occurrences = Occurrences::new(false);
    let mut earliest_new: Option<UtcMillis> = None;
    // A trailing partial line or EOF ends pass 1: only complete lines count.
    while let Line::Complete { bytes, .. } = reader.next_line()? {
        let Extracted::Keep(p) =
            prepare(conn, adapter, stream, &source.id, &bytes, &mut occurrences)?
        else {
            continue;
        };
        if let Some((key, hash)) = record_anchor
            && p.key.as_str() == key
            && p.hash == hash
        {
            return Ok(None);
        }
        // "Not seen before" is judged by record key: a rewritten version of a
        // record we already hold is no evidence about the unread period, so
        // it must not end the gap (it would shrink the gap to nothing).
        if let Some(at) = p.occurred_at
            && at >= start
            && earliest_new.is_none_or(|e| at < e)
            && !ledger::record_key_exists(conn, &source.id, p.key.as_str())?
        {
            earliest_new = Some(at);
        }
    }
    Ok(Some(GapInfo {
        start,
        end: earliest_new.unwrap_or_else(|| env.clock.now()).max(start),
        reason: brk.gap_reason().to_owned(),
    }))
}

/// No file holds the source any more. Only with discovery's confirmation
/// that no other locator exists is the source ended (§4.3); otherwise it is
/// reported `missing` and nothing irreversible is written.
fn file_missing(
    conn: &mut Connection,
    env: &IngestEnv<'_>,
    run: &mut RunContext,
    source: &SourceRow,
    cursor: Option<SourceCursor>,
    trigger: IngestTrigger,
    opts: &ReconcileOptions,
) -> Result<ReconcileReport> {
    let now = env.clock.now();
    if cursor.as_ref().is_some_and(|c| c.ended_at.is_some()) {
        return Ok(ReconcileReport::new(
            source,
            trigger,
            ReadMode::AlreadyEnded,
            "file_gone",
        ));
    }
    if !opts.discovery_confirmed {
        let mut report = ReconcileReport::new(source, trigger, ReadMode::Missing, "file_missing");
        report.from_offset = cursor.as_ref().map_or(0, |c| c.byte_offset);
        report.to_offset = report.from_offset;
        if !trigger.is_live() {
            let tx = conn.transaction()?;
            write_run_record(&tx, env, run, &report, None)?;
            tx.commit()?;
        }
        return Ok(report);
    }
    let mut report = ReconcileReport::new(source, trigger, ReadMode::Gone, "file_gone");
    let mut c = cursor.unwrap_or_else(|| empty_cursor(source, now));
    let tx = conn.transaction()?;
    // Unread bytes existed at the last read (or the file was never read at all).
    let lost = c.size_at_read.is_none_or(|size| c.byte_offset < size);
    if lost && c.last_read_at.is_some() {
        let start = c.last_record_at.or(c.last_read_at).unwrap_or(c.updated_at);
        let end = c.mtime_at_read.unwrap_or(now).max(start);
        let gap = GapInfo {
            start,
            end,
            reason: "deleted_before_read".into(),
        };
        write_collector_record(
            &tx,
            env,
            run,
            CollectorFacts::CoverageGap {
                source_id: source.id.to_string(),
                start: gap.start,
                end: gap.end,
                reason: gap.reason.clone(),
            },
            trigger,
            None,
        )?;
        report.gap = Some(gap);
    }
    write_collector_record(
        &tx,
        env,
        run,
        CollectorFacts::SourceEnded {
            source_id: source.id.to_string(),
            path: c.path.clone().unwrap_or_default(),
            unread_bytes_lost: lost,
        },
        trigger,
        None,
    )?;
    c.ended_at = Some(now);
    c.updated_at = now;
    ledger::put_cursor(&tx, &c)?;
    report.ended = true;
    report.from_offset = c.byte_offset;
    report.to_offset = c.byte_offset;
    write_run_record(&tx, env, run, &report, None)?;
    tx.commit()?;
    Ok(report)
}

/// Records a reconcile that failed, with an error kind only (never paths or
/// record bytes, §4.4). The failed source is not verified by this run.
pub fn record_failed_run(
    conn: &mut Connection,
    env: &IngestEnv<'_>,
    run: &mut RunContext,
    source: &SourceRow,
    trigger: IngestTrigger,
    error: &IngestError,
) -> Result<()> {
    let kind = match error {
        IngestError::Io(io) => format!("io:{:?}", io.kind()),
        IngestError::Sql(_) | IngestError::Store(_) => "store".into(),
        IngestError::Json(_) => "json".into(),
        IngestError::NoAdapter(k) => format!("no_adapter:{k}"),
        IngestError::StreamMeta(_) => "stream_meta".into(),
        IngestError::SimulatedCrash(n) => format!("simulated_crash:{n}"),
    };
    let report = ReconcileReport::new(source, trigger, ReadMode::Tail, "unknown");
    let tx = conn.transaction()?;
    write_run_record(&tx, env, run, &report, Some(kind))?;
    tx.commit()?;
    Ok(())
}

fn write_run_record(
    tx: &Transaction<'_>,
    env: &IngestEnv<'_>,
    run: &mut RunContext,
    r: &ReconcileReport,
    error: Option<String>,
) -> Result<()> {
    write_collector_record(
        tx,
        env,
        run,
        CollectorFacts::ReconcileRun {
            source_id: r.source_id.to_string(),
            trigger: r.trigger,
            mode: if error.is_some() {
                "error".to_owned()
            } else {
                r.mode.as_str().to_owned()
            },
            from_offset: r.from_offset,
            to_offset: r.to_offset,
            reached_eof: r.reached_eof,
            continuity: r.continuity.clone(),
            inserted: r.inserted,
            duplicates: r.duplicates,
            quarantined: r.quarantined,
            key_conflicts: r.key_conflicts,
            ignored_by_type: r.ignored_by_type.clone(),
            error,
        },
        r.trigger,
        None,
    )?;
    Ok(())
}
