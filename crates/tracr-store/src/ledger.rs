//! Typed access to `ledger.sqlite` (§3.2).
//!
//! Functions take `&Connection`, so they work inside a `Transaction` too.
//! The ledger offers no update or delete for evidence or user records; the
//! schema's triggers would reject them anyway (§3.3).

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use tracr_core::evidence::{
    Channel, IngestTrigger, NativeIds, ParseStatus, PayloadState, RetentionClass, TimeQuality,
};
use tracr_core::ids::{ContentHash, MachineId, SourceId};
use tracr_core::time::UtcMillis;

use crate::{Result, StoreError};

// ---------------------------------------------------------------------------
// Machine
// ---------------------------------------------------------------------------

/// Returns the ledger's machine identity, creating it on first use. A ledger
/// belongs to exactly one machine in 0.9 (multi-device merge is OQ-P-11).
pub fn ensure_machine(
    conn: &Connection,
    new_id: &str,
    label: Option<&str>,
    at: UtcMillis,
) -> Result<MachineId> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM machine ORDER BY created_at LIMIT 1",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(MachineId::new(id));
    }
    conn.execute(
        "INSERT INTO machine (id, label, created_at) VALUES (?1, ?2, ?3)",
        params![new_id, label, at.0],
    )?;
    Ok(MachineId::new(new_id))
}

pub fn machine_id(conn: &Connection) -> Result<Option<MachineId>> {
    Ok(conn
        .query_row(
            "SELECT id FROM machine ORDER BY created_at LIMIT 1",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .map(MachineId::new))
}

// ---------------------------------------------------------------------------
// Sources and locators
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    pub id: SourceId,
    pub kind: String,
    pub machine_id: MachineId,
    pub stream_key: String,
    pub adapter: String,
    pub adapter_version: String,
    pub created_at: UtcMillis,
    pub stream_meta: Option<String>,
}

fn source_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SourceRow> {
    Ok(SourceRow {
        id: SourceId::new(r.get::<_, String>(0)?),
        kind: r.get(1)?,
        machine_id: MachineId::new(r.get::<_, String>(2)?),
        stream_key: r.get(3)?,
        adapter: r.get(4)?,
        adapter_version: r.get(5)?,
        created_at: UtcMillis(r.get(6)?),
        stream_meta: r.get(7)?,
    })
}

const SOURCE_COLUMNS: &str =
    "id, kind, machine_id, stream_key, adapter, adapter_version, created_at, stream_meta";

pub struct NewSource<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub machine_id: &'a MachineId,
    pub stream_key: &'a str,
    pub adapter: &'a str,
    pub adapter_version: &'a str,
    pub created_at: UtcMillis,
    pub stream_meta: Option<&'a str>,
}

/// Inserts the source if its `(kind, machine, stream_key)` is new. Returns
/// the stored row and whether it was created now.
pub fn ensure_source(conn: &Connection, s: &NewSource<'_>) -> Result<(SourceRow, bool)> {
    let created = conn.execute(
        "INSERT OR IGNORE INTO source (id, kind, machine_id, stream_key, adapter, adapter_version, created_at, stream_meta)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            s.id,
            s.kind,
            s.machine_id.as_str(),
            s.stream_key,
            s.adapter,
            s.adapter_version,
            s.created_at.0,
            s.stream_meta
        ],
    )? == 1;
    let row = conn.query_row(
        &format!("SELECT {SOURCE_COLUMNS} FROM source WHERE kind = ?1 AND machine_id = ?2 AND stream_key = ?3"),
        params![s.kind, s.machine_id.as_str(), s.stream_key],
        source_from_row,
    )?;
    Ok((row, created))
}

pub fn source(conn: &Connection, id: &SourceId) -> Result<Option<SourceRow>> {
    Ok(conn
        .query_row(
            &format!("SELECT {SOURCE_COLUMNS} FROM source WHERE id = ?1"),
            [id.as_str()],
            source_from_row,
        )
        .optional()?)
}

pub fn sources(conn: &Connection, kind: Option<&str>) -> Result<Vec<SourceRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SOURCE_COLUMNS} FROM source WHERE (?1 IS NULL OR kind = ?1) ORDER BY created_at, stream_key"
    ))?;
    let rows = stmt.query_map([kind], source_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatorRow {
    pub source_id: SourceId,
    pub path: String,
    pub file_identity: Option<String>,
    pub birth_time: Option<UtcMillis>,
    pub first_seen: Option<UtcMillis>,
    pub last_seen: Option<UtcMillis>,
}

/// Records that `path` currently locates the source. `first_seen` is kept
/// from the first sighting.
pub fn upsert_locator(conn: &Connection, l: &LocatorRow) -> Result<()> {
    conn.execute(
        "INSERT INTO source_locator (source_id, path, file_identity, birth_time, first_seen, last_seen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (source_id, path) DO UPDATE SET
           file_identity = excluded.file_identity,
           birth_time = COALESCE(source_locator.birth_time, excluded.birth_time),
           last_seen = excluded.last_seen",
        params![
            l.source_id.as_str(),
            l.path,
            l.file_identity,
            l.birth_time.map(|t| t.0),
            l.first_seen.map(|t| t.0),
            l.last_seen.map(|t| t.0)
        ],
    )?;
    Ok(())
}

/// Locators of a source, most recently seen first.
pub fn locators(conn: &Connection, source_id: &SourceId) -> Result<Vec<LocatorRow>> {
    let mut stmt = conn.prepare(
        "SELECT source_id, path, file_identity, birth_time, first_seen, last_seen
         FROM source_locator WHERE source_id = ?1 ORDER BY last_seen DESC, path",
    )?;
    let rows = stmt.query_map([source_id.as_str()], |r| {
        Ok(LocatorRow {
            source_id: SourceId::new(r.get::<_, String>(0)?),
            path: r.get(1)?,
            file_identity: r.get(2)?,
            birth_time: r.get::<_, Option<i64>>(3)?.map(UtcMillis),
            first_seen: r.get::<_, Option<i64>>(4)?.map(UtcMillis),
            last_seen: r.get::<_, Option<i64>>(5)?.map(UtcMillis),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ---------------------------------------------------------------------------
// Cursors (mutable operational state; never an input to derivation)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCursor {
    pub source_id: SourceId,
    pub path: Option<String>,
    pub file_identity: Option<String>,
    pub byte_offset: u64,
    pub size_at_read: Option<u64>,
    pub mtime_at_read: Option<UtcMillis>,
    pub head_fp: Option<ContentHash>,
    pub head_len: Option<u64>,
    pub last_line_fp: Option<ContentHash>,
    pub last_record_key: Option<String>,
    pub last_record_hash: Option<ContentHash>,
    pub last_record_at: Option<UtcMillis>,
    pub last_read_at: Option<UtcMillis>,
    pub ended_at: Option<UtcMillis>,
    pub updated_at: UtcMillis,
}

fn hash_col(v: Option<Vec<u8>>) -> Option<ContentHash> {
    v.and_then(|b| ContentHash::from_slice(&b))
}

fn to_i64(v: u64) -> Result<i64> {
    i64::try_from(v).map_err(|_| StoreError::Invalid(format!("value {v} exceeds i64")))
}

pub fn cursor(conn: &Connection, source_id: &SourceId) -> Result<Option<SourceCursor>> {
    Ok(conn
        .query_row(
            "SELECT source_id, path, file_identity, byte_offset, size_at_read, mtime_at_read, head_fp, head_len,
                    last_line_fp, last_record_key, last_record_hash, last_record_at, last_read_at, ended_at, updated_at
             FROM source_cursor WHERE source_id = ?1",
            [source_id.as_str()],
            |r| {
                Ok(SourceCursor {
                    source_id: SourceId::new(r.get::<_, String>(0)?),
                    path: r.get(1)?,
                    file_identity: r.get(2)?,
                    byte_offset: r.get::<_, i64>(3)?.max(0) as u64,
                    size_at_read: r.get::<_, Option<i64>>(4)?.map(|v| v.max(0) as u64),
                    mtime_at_read: r.get::<_, Option<i64>>(5)?.map(UtcMillis),
                    head_fp: hash_col(r.get(6)?),
                    head_len: r.get::<_, Option<i64>>(7)?.map(|v| v.max(0) as u64),
                    last_line_fp: hash_col(r.get(8)?),
                    last_record_key: r.get(9)?,
                    last_record_hash: hash_col(r.get(10)?),
                    last_record_at: r.get::<_, Option<i64>>(11)?.map(UtcMillis),
                    last_read_at: r.get::<_, Option<i64>>(12)?.map(UtcMillis),
                    ended_at: r.get::<_, Option<i64>>(13)?.map(UtcMillis),
                    updated_at: UtcMillis(r.get(14)?),
                })
            },
        )
        .optional()?)
}

pub fn put_cursor(conn: &Connection, c: &SourceCursor) -> Result<()> {
    conn.prepare_cached(
        "INSERT INTO source_cursor (source_id, path, file_identity, byte_offset, size_at_read, mtime_at_read,
             head_fp, head_len, last_line_fp, last_record_key, last_record_hash, last_record_at, last_read_at,
             ended_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
         ON CONFLICT (source_id) DO UPDATE SET
           path = excluded.path, file_identity = excluded.file_identity, byte_offset = excluded.byte_offset,
           size_at_read = excluded.size_at_read, mtime_at_read = excluded.mtime_at_read,
           head_fp = excluded.head_fp, head_len = excluded.head_len, last_line_fp = excluded.last_line_fp,
           last_record_key = excluded.last_record_key, last_record_hash = excluded.last_record_hash,
           last_record_at = excluded.last_record_at, last_read_at = excluded.last_read_at,
           ended_at = excluded.ended_at, updated_at = excluded.updated_at",
    )?
    .execute(params![
        c.source_id.as_str(),
        c.path,
        c.file_identity,
        to_i64(c.byte_offset)?,
        c.size_at_read.map(to_i64).transpose()?,
        c.mtime_at_read.map(|t| t.0),
        c.head_fp.map(|h| h.0.to_vec()),
        c.head_len.map(to_i64).transpose()?,
        c.last_line_fp.map(|h| h.0.to_vec()),
        c.last_record_key,
        c.last_record_hash.map(|h| h.0.to_vec()),
        c.last_record_at.map(|t| t.0),
        c.last_read_at.map(|t| t.0),
        c.ended_at.map(|t| t.0),
        c.updated_at.0,
    ])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Observations
// ---------------------------------------------------------------------------

/// One observation envelope plus its retained-fact row.
pub struct NewObservation<'a> {
    pub id: &'a str,
    pub source_id: &'a SourceId,
    pub record_key: &'a str,
    pub content_hash: &'a ContentHash,
    pub channel: Channel,
    pub locator_path: Option<&'a str>,
    pub cursor_offset: Option<u64>,
    pub producer: Option<&'a str>,
    pub producer_version: Option<&'a str>,
    pub adapter_version: &'a str,
    pub manifest_range: Option<&'a str>,
    pub native: &'a NativeIds,
    pub occurred_at: Option<UtcMillis>,
    pub observed_at: UtcMillis,
    pub collector_run_id: &'a str,
    pub receipt_seq: i64,
    pub ingest_trigger: IngestTrigger,
    pub parse_status: ParseStatus,
    pub retention_class: RetentionClass,
    pub extractor_version: &'a str,
    pub kind: &'a str,
    pub subtype: Option<&'a str>,
    pub time_quality: TimeQuality,
    pub facts_json: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted {
        seq: i64,
    },
    /// Same source, record key and content hash already stored (EV-02).
    Duplicate,
}

/// `INSERT OR IGNORE` on `UNIQUE(source_id, record_key, content_hash)` (§4.1).
pub fn insert_observation(conn: &Connection, o: &NewObservation<'_>) -> Result<InsertOutcome> {
    let n = conn
        .prepare_cached(
            "INSERT OR IGNORE INTO observation (id, source_id, record_key, content_hash, channel, locator_path,
                 cursor_offset, producer, producer_version, adapter_version, manifest_range, native_session,
                 native_parent_session, native_agent, native_turn, native_item, native_request, native_message,
                 native_tool_call, occurred_at, observed_at, collector_run_id, receipt_seq, ingest_trigger,
                 parse_status, payload_state, retention_class)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                 ?21, ?22, ?23, ?24, ?25, ?26, ?27)",
        )?
        .execute(params![
            o.id,
            o.source_id.as_str(),
            o.record_key,
            o.content_hash.0.to_vec(),
            o.channel.as_str(),
            o.locator_path,
            o.cursor_offset.map(to_i64).transpose()?,
            o.producer,
            o.producer_version,
            o.adapter_version,
            o.manifest_range,
            o.native.session,
            o.native.parent_session,
            o.native.agent,
            o.native.turn,
            o.native.item,
            o.native.request,
            o.native.message,
            o.native.tool_call,
            o.occurred_at.map(|t| t.0),
            o.observed_at.0,
            o.collector_run_id,
            o.receipt_seq,
            o.ingest_trigger.as_str(),
            o.parse_status.as_str(),
            PayloadState::NeverStored.as_str(),
            o.retention_class.as_str(),
        ])?;
    if n == 0 {
        return Ok(InsertOutcome::Duplicate);
    }
    let seq = conn.last_insert_rowid();
    conn.prepare_cached(
        "INSERT INTO observation_facts (observation_id, extractor_version, kind, subtype, time_quality, facts_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        o.id,
        o.extractor_version,
        o.kind,
        o.subtype,
        o.time_quality.as_str(),
        o.facts_json
    ])?;
    Ok(InsertOutcome::Inserted { seq })
}

/// Whether this exact record (key and content) is already stored.
pub fn observation_exists(
    conn: &Connection,
    source_id: &SourceId,
    record_key: &str,
    hash: &ContentHash,
) -> Result<bool> {
    Ok(conn
        .prepare_cached(
            "SELECT 1 FROM observation WHERE source_id = ?1 AND record_key = ?2 AND content_hash = ?3",
        )?
        .query_row(params![source_id.as_str(), record_key, hash.0.to_vec()], |_| Ok(()))
        .optional()?
        .is_some())
}

/// Whether the source holds any observation with this record key (any content).
pub fn record_key_exists(
    conn: &Connection,
    source_id: &SourceId,
    record_key: &str,
) -> Result<bool> {
    Ok(conn
        .prepare_cached(
            "SELECT 1 FROM observation WHERE source_id = ?1 AND record_key = ?2 LIMIT 1",
        )?
        .query_row(params![source_id.as_str(), record_key], |_| Ok(()))
        .optional()?
        .is_some())
}

/// Content hash of another observation with the same key but different content, if any.
pub fn conflicting_hash(
    conn: &Connection,
    source_id: &SourceId,
    record_key: &str,
    hash: &ContentHash,
) -> Result<Option<ContentHash>> {
    Ok(conn
        .prepare_cached(
            "SELECT content_hash FROM observation WHERE source_id = ?1 AND record_key = ?2 AND content_hash <> ?3
             ORDER BY seq LIMIT 1",
        )?
        .query_row(params![source_id.as_str(), record_key, hash.0.to_vec()], |r| r.get::<_, Vec<u8>>(0))
        .optional()?
        .and_then(|b| ContentHash::from_slice(&b)))
}

/// The producer version of the source's latest observation that has one
/// (AD-19: a record's producer version is its stream's version).
pub fn latest_producer_version(conn: &Connection, source_id: &SourceId) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT producer_version FROM observation WHERE source_id = ?1 AND producer_version IS NOT NULL
             ORDER BY seq DESC LIMIT 1",
            [source_id.as_str()],
            |r| r.get(0),
        )
        .optional()?)
}

/// Content-keyed lines of this content hash that the source's current file
/// generation holds before the cursor. It numbers `h:<hash>#<occurrence>`
/// keys in file order, and is committed in the same transaction as the cursor.
pub fn occurrences_seen(
    conn: &Connection,
    source_id: &SourceId,
    hash: &ContentHash,
) -> Result<u32> {
    let n: Option<i64> = conn
        .prepare_cached(
            "SELECT seen FROM source_cursor_occurrence WHERE source_id = ?1 AND content_hash = ?2",
        )?
        .query_row(params![source_id.as_str(), hash.0.to_vec()], |r| r.get(0))
        .optional()?;
    Ok(n.map_or(0, |v| u32::try_from(v).unwrap_or(u32::MAX)))
}

pub fn put_occurrences_seen(
    conn: &Connection,
    source_id: &SourceId,
    hash: &ContentHash,
    seen: u32,
) -> Result<()> {
    conn.prepare_cached(
        "INSERT INTO source_cursor_occurrence (source_id, content_hash, seen) VALUES (?1, ?2, ?3)
         ON CONFLICT (source_id, content_hash) DO UPDATE SET seen = excluded.seen",
    )?
    .execute(params![source_id.as_str(), hash.0.to_vec(), seen])?;
    Ok(())
}

/// Starts a new file generation: numbering restarts at 0 in file order.
pub fn reset_occurrences(conn: &Connection, source_id: &SourceId) -> Result<()> {
    conn.execute(
        "DELETE FROM source_cursor_occurrence WHERE source_id = ?1",
        [source_id.as_str()],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// User actions and settings
// ---------------------------------------------------------------------------

pub struct NewUserAction<'a> {
    pub id: &'a str,
    pub kind: &'a str,
    pub actor: &'a str,
    pub at: UtcMillis,
    pub reason: Option<&'a str>,
    pub client: &'a str,
}

pub fn append_user_action(conn: &Connection, a: &NewUserAction<'_>) -> Result<i64> {
    conn.execute(
        "INSERT INTO user_action (id, kind, actor, at, reason, client) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![a.id, a.kind, a.actor, a.at.0, a.reason, a.client],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Appends a new revision of a setting. Returns the revision number.
pub fn append_setting(conn: &Connection, key: &str, value: &Value, action_seq: i64) -> Result<i64> {
    let rev: i64 = conn.query_row(
        "SELECT COALESCE(MAX(rev), 0) + 1 FROM setting_rev WHERE setting_key = ?1",
        [key],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO setting_rev (setting_key, rev, action_seq, value) VALUES (?1, ?2, ?3, ?4)",
        params![key, rev, action_seq, serde_json::to_string(value)?],
    )?;
    Ok(rev)
}

/// The latest revision of a setting.
pub fn current_setting(conn: &Connection, key: &str) -> Result<Option<Value>> {
    let text: Option<String> = conn
        .query_row(
            "SELECT value FROM setting_rev WHERE setting_key = ?1 ORDER BY rev DESC LIMIT 1",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(text.map(|t| serde_json::from_str(&t)).transpose()?)
}

// ---------------------------------------------------------------------------
// Read-only statistics (CLI `sources` / `stats`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSummary {
    pub source: SourceRow,
    pub path: Option<String>,
    pub byte_offset: Option<u64>,
    pub size_at_read: Option<u64>,
    pub ended_at: Option<UtcMillis>,
    pub observations: u64,
    pub first_occurred_at: Option<UtcMillis>,
    pub last_occurred_at: Option<UtcMillis>,
}

pub fn source_summaries(conn: &Connection, kind: Option<&str>) -> Result<Vec<SourceSummary>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {cols}, c.path, c.byte_offset, c.size_at_read, c.ended_at,
                (SELECT COUNT(*) FROM observation o WHERE o.source_id = s.id),
                (SELECT MIN(occurred_at) FROM observation o WHERE o.source_id = s.id),
                (SELECT MAX(occurred_at) FROM observation o WHERE o.source_id = s.id)
         FROM source s LEFT JOIN source_cursor c ON c.source_id = s.id
         WHERE (?1 IS NULL OR s.kind = ?1)
         ORDER BY s.kind, 14, s.stream_key",
        cols = SOURCE_COLUMNS
            .split(", ")
            .map(|c| format!("s.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    ))?;
    let rows = stmt.query_map([kind], |r| {
        Ok(SourceSummary {
            source: source_from_row(r)?,
            path: r.get(8)?,
            byte_offset: r.get::<_, Option<i64>>(9)?.map(|v| v.max(0) as u64),
            size_at_read: r.get::<_, Option<i64>>(10)?.map(|v| v.max(0) as u64),
            ended_at: r.get::<_, Option<i64>>(11)?.map(UtcMillis),
            observations: r.get::<_, i64>(12)?.max(0) as u64,
            first_occurred_at: r.get::<_, Option<i64>>(13)?.map(UtcMillis),
            last_occurred_at: r.get::<_, Option<i64>>(14)?.map(UtcMillis),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn pairs(conn: &Connection, sql: &str) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?.unwrap_or_default(),
            r.get(1)?,
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn observation_counts_by_channel(conn: &Connection) -> Result<Vec<(String, i64)>> {
    pairs(
        conn,
        "SELECT channel, COUNT(*) FROM observation GROUP BY channel ORDER BY channel",
    )
}

pub fn source_counts_by_kind(conn: &Connection) -> Result<Vec<(String, i64)>> {
    pairs(
        conn,
        "SELECT kind, COUNT(*) FROM source GROUP BY kind ORDER BY kind",
    )
}

/// Observation counts per retained-fact kind and subtype on one channel.
pub fn fact_counts(conn: &Connection, channel: Channel) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT f.kind || COALESCE('/' || f.subtype, ''), COUNT(*)
         FROM observation o JOIN observation_facts f ON f.observation_id = o.id
         WHERE o.channel = ?1 GROUP BY 1 ORDER BY 2 DESC, 1",
    )?;
    let rows = stmt.query_map([channel.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn parse_status_counts(conn: &Connection) -> Result<Vec<(String, i64)>> {
    pairs(
        conn,
        "SELECT parse_status, COUNT(*) FROM observation GROUP BY parse_status ORDER BY parse_status",
    )
}

pub fn quarantine_counts(conn: &Connection) -> Result<Vec<(String, i64)>> {
    pairs(
        conn,
        "SELECT COALESCE(json_extract(f.facts_json, '$.quarantine.native_type'), '(no type)') || ' / ' ||
                json_extract(f.facts_json, '$.quarantine.reason'), COUNT(*)
         FROM observation_facts f WHERE f.kind = 'quarantined' GROUP BY 1 ORDER BY 2 DESC",
    )
}

/// `(producer version, manifest range) → count` for provider observations.
pub fn producer_version_counts(conn: &Connection) -> Result<Vec<(String, i64)>> {
    pairs(
        conn,
        "SELECT COALESCE(producer_version, '(none)') || '  ' || COALESCE(manifest_range, ''), COUNT(*)
         FROM observation WHERE channel <> 'collector' GROUP BY 1 ORDER BY 1",
    )
}

pub fn collector_record_counts(conn: &Connection) -> Result<Vec<(String, i64)>> {
    pairs(
        conn,
        "SELECT f.kind, COUNT(*) FROM observation o JOIN observation_facts f ON f.observation_id = o.id
         WHERE o.channel = 'collector' GROUP BY f.kind ORDER BY f.kind",
    )
}

/// Lines ignored by manifest rule, summed over all reconcile runs.
pub fn ignored_line_counts(conn: &Connection) -> Result<Vec<(String, i64)>> {
    pairs(
        conn,
        "SELECT j.key, SUM(j.value) FROM observation_facts f,
                json_each(json_extract(f.facts_json, '$.collector.ignored_by_type')) j
         WHERE f.kind = 'reconcile_run' GROUP BY j.key ORDER BY 2 DESC",
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapRow {
    pub source_id: String,
    pub start: UtcMillis,
    pub end: UtcMillis,
    pub reason: String,
}

pub fn coverage_gaps(conn: &Connection) -> Result<Vec<GapRow>> {
    let mut stmt = conn.prepare(
        "SELECT json_extract(facts_json, '$.collector.source_id'), json_extract(facts_json, '$.collector.start'),
                json_extract(facts_json, '$.collector.end'), json_extract(facts_json, '$.collector.reason')
         FROM observation_facts WHERE kind = 'coverage_gap' ORDER BY 2",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(GapRow {
            source_id: r.get(0)?,
            start: UtcMillis(r.get(1)?),
            end: UtcMillis(r.get(2)?),
            reason: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Usage-bearing observations versus distinct usage identities. These are
/// counts of records, not token totals: streamed re-emissions are elected
/// later (§8.1), so summing record usage here would double count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageRecordStats {
    pub usage_bearing_observations: u64,
    pub distinct_identities: u64,
    pub without_identity: u64,
}

pub fn usage_record_stats(conn: &Connection) -> Result<UsageRecordStats> {
    Ok(conn.query_row(
        "SELECT COUNT(*),
                COUNT(DISTINCT CASE WHEN json_extract(facts_json, '$.usage.identity.request_id') IS NOT NULL
                                      OR json_extract(facts_json, '$.usage.identity.message_id') IS NOT NULL
                    THEN COALESCE(json_extract(facts_json, '$.usage.identity.request_id'), '') || '|' ||
                         COALESCE(json_extract(facts_json, '$.usage.identity.message_id'), '') END),
                SUM(CASE WHEN json_extract(facts_json, '$.usage.identity.request_id') IS NULL
                          AND json_extract(facts_json, '$.usage.identity.message_id') IS NULL THEN 1 ELSE 0 END)
         FROM observation_facts WHERE json_extract(facts_json, '$.usage') IS NOT NULL",
        [],
        |r| {
            Ok(UsageRecordStats {
                usage_bearing_observations: r.get::<_, i64>(0)?.max(0) as u64,
                distinct_identities: r.get::<_, i64>(1)?.max(0) as u64,
                without_identity: r.get::<_, Option<i64>>(2)?.unwrap_or(0).max(0) as u64,
            })
        },
    )?)
}

pub fn count_observations(conn: &Connection, exclude_channel: Option<Channel>) -> Result<u64> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM observation WHERE (?1 IS NULL OR channel <> ?1)",
        [exclude_channel.map(|c| c.as_str())],
        |r| r.get(0),
    )?;
    Ok(n.max(0) as u64)
}
