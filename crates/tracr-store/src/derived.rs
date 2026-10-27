//! `derived.sqlite` (architecture §3.1, §3.4): rebuildable, never migrated.
//!
//! A rebuild writes `derived.next.sqlite` from scratch and renames it over
//! `derived.sqlite` (only the holder of `tracr.lock` does this); an update
//! writes the installed file in place. The store is in WAL mode, so readers
//! (the UI) never wait for a writer, and an open handle keeps reading the
//! file it opened across a rename.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use tracr_core::time::UtcMillis;

use crate::{Result, StoreError};

pub const DERIVED_FILE: &str = "derived.sqlite";
pub const DERIVED_NEXT_FILE: &str = "derived.next.sqlite";
/// Bumped whenever `schema/derived.sql` changes; a mismatch means "rebuild".
pub const DERIVED_SCHEMA_VERSION: i64 = 6;
const SCHEMA: &str = include_str!("../schema/derived.sql");

fn sidecars(path: &Path) -> [PathBuf; 3] {
    let s = path.to_string_lossy();
    [
        PathBuf::from(format!("{s}-wal")),
        PathBuf::from(format!("{s}-shm")),
        PathBuf::from(format!("{s}-journal")),
    ]
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Creates an empty `derived.next.sqlite` with the current schema,
/// replacing any leftover from an interrupted rebuild.
pub fn create_next(data_dir: &Path) -> Result<Connection> {
    let path = data_dir.join(DERIVED_NEXT_FILE);
    remove_if_exists(&path)?;
    for s in sidecars(&path) {
        remove_if_exists(&s)?;
    }
    let conn = Connection::open(&path)?;
    // WAL: readers (the UI) never wait for an update in progress. Closing the
    // last connection checkpoints, so the installed file carries no -wal.
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
    conn.execute_batch(SCHEMA)?;
    conn.pragma_update(None, "user_version", DERIVED_SCHEMA_VERSION)?;
    Ok(conn)
}

/// Installs the finished next store. The caller must have closed its
/// connection to it; the rename is atomic on the same filesystem.
pub fn install_next(data_dir: &Path) -> Result<()> {
    let next = data_dir.join(DERIVED_NEXT_FILE);
    let live = data_dir.join(DERIVED_FILE);
    for s in sidecars(&live) {
        remove_if_exists(&s)?;
    }
    fs::rename(&next, &live)?;
    Ok(())
}

/// Opens the installed derived store for an in-place update by the lock
/// holder. `Ok(None)` if it is missing or from another schema version: the
/// caller then does a full rebuild.
pub fn open_writer(data_dir: &Path) -> Result<Option<Connection>> {
    let path = data_dir.join(DERIVED_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let conn = Connection::open(&path)?;
    conn.busy_timeout(Duration::from_secs(10))?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    Ok((version == DERIVED_SCHEMA_VERSION).then_some(conn))
}

/// Opens the installed derived store read-only. `Ok(None)` if none was
/// built yet; an error if it was built by a different schema version.
pub fn open_read_only(data_dir: &Path) -> Result<Option<Connection>> {
    let path = data_dir.join(DERIVED_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_secs(10))?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version != DERIVED_SCHEMA_VERSION {
        return Err(StoreError::Invalid(format!(
            "derived store out of date (schema v{version}, this build v{DERIVED_SCHEMA_VERSION}); rebuild pending: run `tracr rebuild`"
        )));
    }
    Ok(Some(conn))
}

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct RunRow {
    pub id: String,
    pub unit: String,
    pub unit_key: String,
    pub as_of: i64,
    pub obs_watermark: i64,
    pub versions: String,
    pub input_fingerprint: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct EventRow {
    pub id: String,
    pub kind: String,
    pub subtype: Option<String>,
    pub provider: String,
    pub session_id: Option<String>,
    pub actor_id: Option<String>,
    pub turn_id: Option<String>,
    pub occurred_at: Option<i64>,
    pub time_quality: String,
    pub observed_at: i64,
    pub native_id: Option<String>,
    pub context: Option<String>,
    pub model: Option<String>,
    pub model_source: Option<String>,
    pub normalizer_version: String,
    pub copied_from: Option<String>,
    pub quality: Option<String>,
    pub observation_id: String,
    pub run: String,
}

#[derive(Debug, Clone, Default)]
pub struct SessionRow {
    pub id: String,
    pub provider: String,
    pub machine_id: String,
    pub native_session_id: String,
    pub first_evidence_at: Option<i64>,
    pub last_evidence_at: Option<i64>,
    pub end_basis: Option<String>,
    pub diagnostics: Option<String>,
    pub label: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub run: String,
}

#[derive(Debug, Clone, Default)]
pub struct ActorRow {
    pub id: String,
    pub session_id: String,
    pub actor_kind: String,
    pub native_agent_id: Option<String>,
    pub run: String,
}

#[derive(Debug, Clone, Default)]
pub struct TurnRow {
    pub id: String,
    pub session_id: String,
    pub actor_id: Option<String>,
    pub native_turn_id: String,
    pub ordinal: i64,
    pub origin: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    pub end_kind: Option<String>,
    pub end_basis: String,
    pub run: String,
}

#[derive(Debug, Clone, Default)]
pub struct ToolCallRow {
    pub id: String,
    pub session_id: String,
    pub actor_id: Option<String>,
    pub turn_id: Option<String>,
    pub native_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_kind: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub run: String,
}

#[derive(Debug, Clone, Default)]
pub struct SegmentRow {
    pub id: String,
    pub session_id: String,
    pub actor_id: String,
    pub turn_id: Option<String>,
    pub start_at: i64,
    pub end_at: i64,
    pub start_basis: String,
    pub end_basis: String,
    pub kind: String,
    pub rule_ref: String,
    pub run: String,
}

#[derive(Debug, Clone, Default)]
pub struct InstantRow {
    pub id: String,
    pub at: i64,
    pub session_id: String,
    pub event_id: String,
    pub subtype: Option<String>,
    pub context: Option<String>,
    pub run: String,
}

#[derive(Debug, Clone, Default)]
pub struct UsageRow {
    pub id: String,
    pub provider: String,
    pub stream_key: String,
    pub dedup_identity: Option<String>,
    pub identified: bool,
    pub duplicate_of: Option<String>,
    pub session_id: Option<String>,
    pub actor_id: Option<String>,
    pub turn_id: Option<String>,
    pub model: Option<String>,
    pub model_source: Option<String>,
    pub service_tier: Option<String>,
    pub semantics: String,
    pub scope: String,
    pub input_fresh: Option<i64>,
    pub input_total: Option<i64>,
    pub cache_read: Option<i64>,
    pub cache_creation: Option<i64>,
    pub cache_creation_tiers: Option<String>,
    pub output: Option<i64>,
    pub reasoning_output: Option<i64>,
    pub tool_charges: Option<String>,
    pub extent_start: Option<i64>,
    pub extent_end: Option<i64>,
    pub election_rule: String,
    pub quality: Option<String>,
    pub sources: Vec<String>,
    pub run: String,
}

/// One estimate of one usage record (architecture §8.2).
#[derive(Debug, Clone, Default)]
pub struct CostRow {
    pub usage_id: String,
    pub session_id: Option<String>,
    pub pricing_revision: String,
    pub currency: String,
    pub status: String,
    pub amount_nano: Option<i64>,
    pub priced_tokens: i64,
    pub unpriced_tokens: i64,
    pub reasons: Option<String>,
    pub assumptions: Option<String>,
    pub price_basis: Option<String>,
    pub rule_id: Option<String>,
}

/// The latest provider-reported figure of one session and field.
#[derive(Debug, Clone, Default)]
pub struct ReportedRow {
    pub session_id: String,
    pub source_field: String,
    pub value_as_stated: String,
    pub value_nano: Option<i64>,
    pub currency: String,
    pub semantics: String,
    pub scope: String,
    pub observation_id: String,
    pub observed_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct AtomRow {
    pub id: String,
    pub day: String,
    pub start_at: i64,
    pub end_at: i64,
    pub work_status: String,
    pub counting_basis: Option<String>,
    pub unknown_reason: Option<String>,
    pub duration_basis: String,
    pub candidate_status: Option<String>,
    pub candidate_reason: Option<String>,
    pub candidate_basis: Option<String>,
    pub start_boundary: Option<String>,
    pub end_boundary: Option<String>,
    pub reasons: Option<String>,
    pub decided_by: String,
    pub rule_ref: String,
    pub run: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalRow {
    pub kind: String,
    pub start_at: i64,
    pub end_at: i64,
    pub state: String,
    pub reason: Option<String>,
}

/// `(session id, first evidence, cwd)`.
pub type SessionCwd = (String, Option<i64>, Option<String>);

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Inserts derived rows inside one transaction on the next store.
pub struct DerivedWriter<'c> {
    tx: Transaction<'c>,
}

impl<'c> DerivedWriter<'c> {
    pub fn begin(conn: &'c mut Connection) -> Result<Self> {
        Ok(Self {
            tx: conn.transaction()?,
        })
    }

    pub fn commit(self) -> Result<()> {
        Ok(self.tx.commit()?)
    }

    /// Removes every row derived for one session (before re-deriving it).
    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        self.tx.execute(
            "DELETE FROM norm_event_source WHERE event_id IN (SELECT id FROM norm_event WHERE session_id = ?1)",
            [session_id],
        )?;
        self.tx.execute(
            "DELETE FROM usage_record_source WHERE usage_id IN (SELECT id FROM usage_record WHERE session_id = ?1)",
            [session_id],
        )?;
        for table in [
            "norm_event",
            "usage_record",
            "cost_estimate",
            "reported_cost",
            "agent_actor",
            "turn",
            "tool_call",
            "agent_segment",
            "engagement_instant",
        ] {
            self.tx.execute(
                &format!("DELETE FROM {table} WHERE session_id = ?1"),
                [session_id],
            )?;
        }
        self.tx
            .execute("DELETE FROM agent_session WHERE id = ?1", [session_id])?;
        self.tx.execute(
            "DELETE FROM derivation_run WHERE unit = 'session' AND unit_key = ?1",
            [session_id],
        )?;
        Ok(())
    }

    /// Removes the coverage, device and human layers (recomputed as a whole).
    pub fn clear_human(&self) -> Result<()> {
        self.tx.execute_batch(
            "DELETE FROM human_proposal; DELETE FROM human_atom_context; DELETE FROM human_atom;
             DELETE FROM device_interval; DELETE FROM coverage_interval; DELETE FROM attribution; DELETE FROM day_quality;
             DELETE FROM derivation_run WHERE unit = 'human';",
        )?;
        Ok(())
    }

    /// Every engagement instant `(session_id, at, context JSON)` currently in the store.
    pub fn all_instants(&self) -> Result<Vec<(String, i64, Option<String>)>> {
        let mut stmt = self
            .tx
            .prepare("SELECT session_id, at, context FROM engagement_instant")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every session `(id, first_evidence_at, cwd)`.
    pub fn all_session_cwds(&self) -> Result<Vec<SessionCwd>> {
        let mut stmt = self
            .tx
            .prepare("SELECT id, first_evidence_at, cwd FROM agent_session")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_session_repository(
        &self,
        id: &str,
        key: Option<&str>,
        label: Option<&str>,
    ) -> Result<()> {
        self.tx
            .prepare_cached(
                "UPDATE agent_session SET repository_key = ?2, repository_label = ?3 WHERE id = ?1",
            )?
            .execute(params![id, key, label])?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn attribution(
        &self,
        subject_kind: &str,
        subject_id: &str,
        level: &str,
        state: &str,
        targets: &str,
        origin: &str,
        evidence: Option<&str>,
        effective: bool,
        report_target: bool,
        run: &str,
    ) -> Result<()> {
        self.tx
            .prepare_cached(
                "INSERT INTO attribution (subject_kind, subject_id, level, state, targets, origin, evidence,
                   effective, report_target, derivation_run_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?
            .execute(params![
                subject_kind,
                subject_id,
                level,
                state,
                targets,
                origin,
                evidence,
                effective,
                report_target,
                run
            ])?;
        Ok(())
    }

    /// Every execution segment `(session_id, start, end)` currently in the store.
    pub fn all_segments(&self) -> Result<Vec<(String, i64, i64)>> {
        let mut stmt = self
            .tx
            .prepare("SELECT session_id, start_at, end_at FROM agent_segment")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn meta(&self, key: &str, value: &str) -> Result<()> {
        self.tx.execute(
            "INSERT OR REPLACE INTO derived_meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn run(&self, r: &RunRow) -> Result<()> {
        self.tx.prepare_cached(
            "INSERT INTO derivation_run (id, unit, unit_key, as_of, obs_watermark, versions, input_fingerprint)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?
        .execute(params![r.id, r.unit, r.unit_key, r.as_of, r.obs_watermark, r.versions, r.input_fingerprint])?;
        Ok(())
    }

    pub fn event(&self, e: &EventRow) -> Result<()> {
        self.tx.prepare_cached(
            "INSERT INTO norm_event (id, kind, subtype, provider, session_id, actor_id, turn_id, occurred_at,
               time_quality, observed_at, native_id, context, model, model_source, normalizer_version,
               copied_from, quality, derivation_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        )?
        .execute(params![
            e.id, e.kind, e.subtype, e.provider, e.session_id, e.actor_id, e.turn_id, e.occurred_at,
            e.time_quality, e.observed_at, e.native_id, e.context, e.model, e.model_source,
            e.normalizer_version, e.copied_from, e.quality, e.run
        ])?;
        self.tx
            .prepare_cached(
                "INSERT INTO norm_event_source (event_id, observation_id) VALUES (?1, ?2)",
            )?
            .execute(params![e.id, e.observation_id])?;
        Ok(())
    }

    pub fn session(&self, s: &SessionRow) -> Result<()> {
        self.tx.prepare_cached(
            "INSERT INTO agent_session (id, provider, machine_id, native_session_id, first_evidence_at,
               last_evidence_at, end_basis, diagnostics, label, cwd, git_branch, derivation_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?
        .execute(params![
            s.id, s.provider, s.machine_id, s.native_session_id, s.first_evidence_at, s.last_evidence_at,
            s.end_basis, s.diagnostics, s.label, s.cwd, s.git_branch, s.run
        ])?;
        Ok(())
    }

    pub fn actor(&self, a: &ActorRow) -> Result<()> {
        self.tx.prepare_cached(
            "INSERT INTO agent_actor (id, session_id, actor_kind, native_agent_id, derivation_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?
        .execute(params![a.id, a.session_id, a.actor_kind, a.native_agent_id, a.run])?;
        Ok(())
    }

    pub fn turn(&self, t: &TurnRow) -> Result<()> {
        self.tx.prepare_cached(
            "INSERT INTO turn (id, session_id, actor_id, native_turn_id, ordinal, origin, started_at, ended_at,
               end_kind, end_basis, derivation_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?
        .execute(params![
            t.id, t.session_id, t.actor_id, t.native_turn_id, t.ordinal, t.origin, t.started_at,
            t.ended_at, t.end_kind, t.end_basis, t.run
        ])?;
        Ok(())
    }

    pub fn tool_call(&self, t: &ToolCallRow) -> Result<()> {
        self.tx.prepare_cached(
            "INSERT INTO tool_call (id, session_id, actor_id, turn_id, native_id, tool_name, tool_kind,
               started_at, finished_at, derivation_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?
        .execute(params![
            t.id, t.session_id, t.actor_id, t.turn_id, t.native_id, t.tool_name, t.tool_kind,
            t.started_at, t.finished_at, t.run
        ])?;
        Ok(())
    }

    pub fn segment(&self, s: &SegmentRow) -> Result<()> {
        self.tx.prepare_cached(
            "INSERT INTO agent_segment (id, session_id, actor_id, turn_id, start_at, end_at, start_basis,
               end_basis, kind, rule_ref, derivation_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?
        .execute(params![
            s.id, s.session_id, s.actor_id, s.turn_id, s.start_at, s.end_at, s.start_basis,
            s.end_basis, s.kind, s.rule_ref, s.run
        ])?;
        Ok(())
    }

    pub fn instant(&self, i: &InstantRow) -> Result<()> {
        self.tx.prepare_cached(
            "INSERT INTO engagement_instant (id, at, session_id, event_id, subtype, context, derivation_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?
        .execute(params![i.id, i.at, i.session_id, i.event_id, i.subtype, i.context, i.run])?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn coverage(
        &self,
        machine: &str,
        kind: &str,
        start: i64,
        end: i64,
        state: &str,
        reason: Option<&str>,
        run: &str,
    ) -> Result<()> {
        self.tx
            .prepare_cached(
                "INSERT INTO coverage_interval (machine_id, source_kind, start_at, end_at, state, reason, derivation_run_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?
            .execute(params![machine, kind, start, end, state, reason, run])?;
        Ok(())
    }

    /// A stream-level extent: a completeness gap or a degraded extent.
    #[allow(clippy::too_many_arguments)]
    pub fn stream_extent(
        &self,
        machine: &str,
        kind: &str,
        source_id: &str,
        start: i64,
        end: i64,
        state: &str,
        completeness: Option<&str>,
        reason: Option<&str>,
        run: &str,
    ) -> Result<()> {
        self.tx
            .prepare_cached(
                "INSERT INTO coverage_interval (machine_id, source_kind, source_id, start_at, end_at, state,
                   completeness, reason, derivation_run_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?
            .execute(params![
                machine,
                kind,
                source_id,
                start,
                end,
                state,
                completeness,
                reason,
                run
            ])?;
        Ok(())
    }

    pub fn day_quality(&self, day: &str, recovered: i64, run: &str) -> Result<()> {
        self.tx
            .prepare_cached(
                "INSERT OR REPLACE INTO day_quality (day, recovered_records, derivation_run_id)
                 VALUES (?1, ?2, ?3)",
            )?
            .execute(params![day, recovered, run])?;
        Ok(())
    }

    pub fn device(
        &self,
        machine: &str,
        start: i64,
        end: i64,
        state: &str,
        run: &str,
    ) -> Result<()> {
        self.tx
            .prepare_cached(
                "INSERT INTO device_interval (machine_id, start_at, end_at, state, derivation_run_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?
            .execute(params![machine, start, end, state, run])?;
        Ok(())
    }

    pub fn atom(&self, a: &AtomRow) -> Result<()> {
        self.tx.prepare_cached(
            "INSERT INTO human_atom (id, day, start_at, end_at, work_status, counting_basis, unknown_reason,
               duration_basis, candidate_status, candidate_reason, candidate_basis, start_boundary, end_boundary,
               reasons, decided_by, rule_ref, derivation_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        )?
        .execute(params![
            a.id, a.day, a.start_at, a.end_at, a.work_status, a.counting_basis, a.unknown_reason,
            a.duration_basis, a.candidate_status, a.candidate_reason, a.candidate_basis, a.start_boundary,
            a.end_boundary, a.reasons, a.decided_by, a.rule_ref, a.run
        ])?;
        Ok(())
    }

    pub fn atom_context(&self, atom: &str, role: &str, kind: &str, key: &str) -> Result<()> {
        self.tx
            .prepare_cached(
                "INSERT OR IGNORE INTO human_atom_context (atom_id, role, context_kind, context_key)
                 VALUES (?1, ?2, ?3, ?4)",
            )?
            .execute(params![atom, role, kind, key])?;
        Ok(())
    }

    pub fn proposal(&self, atom: &str, at: i64, kind: &str) -> Result<()> {
        self.tx
            .prepare_cached(
                "INSERT OR IGNORE INTO human_proposal (atom_id, at, kind) VALUES (?1, ?2, ?3)",
            )?
            .execute(params![atom, at, kind])?;
        Ok(())
    }

    pub fn usage(&self, u: &UsageRow) -> Result<()> {
        self.tx.prepare_cached(
            "INSERT INTO usage_record (id, provider, stream_key, dedup_identity, identified, canonical,
               duplicate_of, session_id, actor_id, turn_id, model, model_source, service_tier, semantics,
               scope, input_fresh, input_total, cache_read, cache_creation, cache_creation_tiers, output,
               reasoning_output, tool_charges, extent_start, extent_end, election_rule, quality,
               derivation_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
               ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)",
        )?
        .execute(params![
            u.id, u.provider, u.stream_key, u.dedup_identity, u.identified, u.duplicate_of.is_none(),
            u.duplicate_of, u.session_id, u.actor_id, u.turn_id, u.model, u.model_source,
            u.service_tier, u.semantics, u.scope, u.input_fresh, u.input_total, u.cache_read,
            u.cache_creation, u.cache_creation_tiers, u.output, u.reasoning_output, u.tool_charges,
            u.extent_start, u.extent_end, u.election_rule, u.quality, u.run
        ])?;
        let mut stmt = self.tx.prepare_cached(
            "INSERT OR IGNORE INTO usage_record_source (usage_id, observation_id) VALUES (?1, ?2)",
        )?;
        for o in &u.sources {
            stmt.execute(params![u.id, o])?;
        }
        Ok(())
    }
}

impl DerivedWriter<'_> {
    pub fn cost(&self, c: &CostRow) -> Result<()> {
        self.tx
            .prepare_cached(
                "INSERT INTO cost_estimate (usage_id, session_id, pricing_revision, currency, status,
                   amount_nano, priced_tokens, unpriced_tokens, reasons, assumptions, price_basis, rule_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?
            .execute(params![
                c.usage_id, c.session_id, c.pricing_revision, c.currency, c.status, c.amount_nano,
                c.priced_tokens, c.unpriced_tokens, c.reasons, c.assumptions, c.price_basis, c.rule_id
            ])?;
        Ok(())
    }

    pub fn reported(&self, r: &ReportedRow) -> Result<()> {
        self.tx
            .prepare_cached(
                "INSERT OR REPLACE INTO reported_cost (session_id, source_field, value_as_stated,
                   value_nano, currency, semantics, scope, observation_id, observed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?
            .execute(params![
                r.session_id,
                r.source_field,
                r.value_as_stated,
                r.value_nano,
                r.currency,
                r.semantics,
                r.scope,
                r.observation_id,
                r.observed_at
            ])?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Readers
// ---------------------------------------------------------------------------

pub fn meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM derived_meta WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .optional()?)
}

/// Instants at which derived activity exists: segment bounds, engagement
/// instants and canonical usage extents (for listing days with data).
pub fn activity_instants(conn: &Connection) -> Result<Vec<UtcMillis>> {
    let mut stmt = conn.prepare(
        "SELECT start_at FROM agent_segment UNION SELECT end_at FROM agent_segment
         UNION SELECT at FROM engagement_instant
         UNION SELECT extent_start FROM usage_record WHERE canonical = 1 AND extent_start IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, i64>(0).map(UtcMillis))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Sessions with a segment, an engagement instant or canonical usage in `[start, end)`.
pub fn sessions_active_in(conn: &Connection, start: i64, end: i64) -> Result<Vec<SessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider, machine_id, native_session_id, first_evidence_at, last_evidence_at,
                end_basis, diagnostics, label, cwd, git_branch, derivation_run_id
         FROM agent_session s
         WHERE EXISTS (SELECT 1 FROM agent_segment g WHERE g.session_id = s.id AND g.start_at < ?2 AND g.end_at > ?1)
            OR EXISTS (SELECT 1 FROM engagement_instant i WHERE i.session_id = s.id AND i.at >= ?1 AND i.at < ?2)
            OR EXISTS (SELECT 1 FROM usage_record u WHERE u.session_id = s.id AND u.canonical = 1
                         AND u.extent_start >= ?1 AND u.extent_start < ?2)
         ORDER BY first_evidence_at, id",
    )?;
    let rows = stmt.query_map(params![start, end], |r| {
        Ok(SessionRow {
            id: r.get(0)?,
            provider: r.get(1)?,
            machine_id: r.get(2)?,
            native_session_id: r.get(3)?,
            first_evidence_at: r.get(4)?,
            last_evidence_at: r.get(5)?,
            end_basis: r.get(6)?,
            diagnostics: r.get(7)?,
            label: r.get(8)?,
            cwd: r.get(9)?,
            git_branch: r.get(10)?,
            run: r.get(11)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn actors_of(conn: &Connection, session_id: &str) -> Result<Vec<ActorRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, actor_kind, native_agent_id, derivation_run_id FROM agent_actor
         WHERE session_id = ?1 ORDER BY actor_kind DESC, native_agent_id, id",
    )?;
    let rows = stmt.query_map([session_id], |r| {
        Ok(ActorRow {
            id: r.get(0)?,
            session_id: r.get(1)?,
            actor_kind: r.get(2)?,
            native_agent_id: r.get(3)?,
            run: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Segments of a session overlapping `[start, end)` (unclipped).
pub fn segments_in(
    conn: &Connection,
    session_id: &str,
    start: i64,
    end: i64,
) -> Result<Vec<SegmentRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, actor_id, turn_id, start_at, end_at, start_basis, end_basis, kind, rule_ref,
                derivation_run_id
         FROM agent_segment WHERE session_id = ?1 AND start_at < ?3 AND end_at > ?2 ORDER BY start_at, id",
    )?;
    let rows = stmt.query_map(params![session_id, start, end], |r| {
        Ok(SegmentRow {
            id: r.get(0)?,
            session_id: r.get(1)?,
            actor_id: r.get(2)?,
            turn_id: r.get(3)?,
            start_at: r.get(4)?,
            end_at: r.get(5)?,
            start_basis: r.get(6)?,
            end_basis: r.get(7)?,
            kind: r.get(8)?,
            rule_ref: r.get(9)?,
            run: r.get(10)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn instants_in(
    conn: &Connection,
    session_id: &str,
    start: i64,
    end: i64,
) -> Result<Vec<InstantRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, at, session_id, event_id, subtype, context, derivation_run_id FROM engagement_instant
         WHERE session_id = ?1 AND at >= ?2 AND at < ?3 ORDER BY at, id",
    )?;
    let rows = stmt.query_map(params![session_id, start, end], |r| {
        Ok(InstantRow {
            id: r.get(0)?,
            at: r.get(1)?,
            session_id: r.get(2)?,
            event_id: r.get(3)?,
            subtype: r.get(4)?,
            context: r.get(5)?,
            run: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// (turns started, tool calls started) of a session in `[start, end)`, by actor ID.
pub fn activity_counts_in(
    conn: &Connection,
    session_id: &str,
    start: i64,
    end: i64,
) -> Result<Vec<(Option<String>, i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT actor_id, SUM(t), SUM(c) FROM (
           SELECT actor_id, 1 AS t, 0 AS c FROM turn WHERE session_id = ?1 AND started_at >= ?2 AND started_at < ?3
           UNION ALL
           SELECT actor_id, 0, 1 FROM tool_call WHERE session_id = ?1 AND started_at >= ?2 AND started_at < ?3)
         GROUP BY actor_id",
    )?;
    let rows = stmt.query_map(params![session_id, start, end], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Token sums of canonical usage records, grouped by actor and model. Each
/// category has its sum over records that state it, and the number of
/// records that do not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageSum {
    pub actor_id: Option<String>,
    pub model: Option<String>,
    pub identified: bool,
    pub records: i64,
    pub input_fresh: (i64, i64),
    pub cache_creation: (i64, i64),
    pub cache_read: (i64, i64),
    pub output: (i64, i64),
    pub cost: CostSum,
}

/// Estimates of a group of usage records under one pricing revision.
/// `amount_nano` sums the priced parts only; it is `None` when nothing in
/// the group was priced (unpriced, not zero).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CostSum {
    pub amount_nano: Option<i64>,
    pub priced_records: i64,
    pub partially_priced_records: i64,
    pub unpriced_records: i64,
    /// Records without an estimate under this revision.
    pub not_estimated_records: i64,
    pub priced_tokens: i64,
    pub unpriced_tokens: i64,
    pub reasons: Vec<String>,
    pub assumptions: Vec<String>,
}

const COST_COLUMNS: &str = "SUM(c.amount_nano), SUM(c.status = 'priced'), SUM(c.status = 'partially_priced'),
     SUM(c.status = 'unpriced'), SUM(c.usage_id IS NULL), COALESCE(SUM(c.priced_tokens), 0),
     COALESCE(SUM(c.unpriced_tokens), 0), GROUP_CONCAT(c.reasons, '\x1f'), GROUP_CONCAT(c.assumptions, '\x1f')";

fn json_lists(joined: Option<String>) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for part in joined.iter().flat_map(|j| j.split('\x1f')) {
        if let Ok(v) = serde_json::from_str::<Vec<String>>(part) {
            out.extend(v);
        }
    }
    out.into_iter().collect()
}

fn cost_sum(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<CostSum> {
    Ok(CostSum {
        amount_nano: r.get(at)?,
        priced_records: r.get::<_, Option<i64>>(at + 1)?.unwrap_or(0),
        partially_priced_records: r.get::<_, Option<i64>>(at + 2)?.unwrap_or(0),
        unpriced_records: r.get::<_, Option<i64>>(at + 3)?.unwrap_or(0),
        not_estimated_records: r.get::<_, Option<i64>>(at + 4)?.unwrap_or(0),
        priced_tokens: r.get(at + 5)?,
        unpriced_tokens: r.get(at + 6)?,
        reasons: json_lists(r.get(at + 7)?),
        assumptions: json_lists(r.get(at + 8)?),
    })
}

/// Canonical usage of one session whose extent lies within `[start, end)`
/// (AD-16), grouped by actor and model, with estimates under `revision`.
pub fn usage_sums_in(
    conn: &Connection,
    session_id: &str,
    start: i64,
    end: i64,
    revision: &str,
) -> Result<Vec<UsageSum>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT u.actor_id, u.model, u.identified, COUNT(*),
                COALESCE(SUM(u.input_fresh), 0), SUM(u.input_fresh IS NULL),
                COALESCE(SUM(u.cache_creation), 0), SUM(u.cache_creation IS NULL),
                COALESCE(SUM(u.cache_read), 0), SUM(u.cache_read IS NULL),
                COALESCE(SUM(u.output), 0), SUM(u.output IS NULL), {COST_COLUMNS}
         FROM usage_record u
         LEFT JOIN cost_estimate c ON c.usage_id = u.id AND c.pricing_revision = ?4
         WHERE u.session_id = ?1 AND u.canonical = 1 AND u.extent_start >= ?2 AND u.extent_start < ?3
           AND COALESCE(u.extent_end, u.extent_start) <= ?3
         GROUP BY u.actor_id, u.model, u.identified ORDER BY u.actor_id, u.model, u.identified DESC"
    ))?;
    let rows = stmt.query_map(params![session_id, start, end, revision], |r| {
        Ok(UsageSum {
            actor_id: r.get(0)?,
            model: r.get(1)?,
            identified: r.get(2)?,
            records: r.get(3)?,
            input_fresh: (r.get(4)?, r.get(5)?),
            cache_creation: (r.get(6)?, r.get(7)?),
            cache_read: (r.get(8)?, r.get(9)?),
            output: (r.get(10)?, r.get(11)?),
            cost: cost_sum(r, 12)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Canonical usage whose extent crosses a boundary of `[start, end)`: it
/// belongs to no single day (AD-16), so it is shown on its own line.
pub fn spanning_usage(
    conn: &Connection,
    start: i64,
    end: i64,
    revision: &str,
) -> Result<(i64, CostSum)> {
    let sql = format!(
        "SELECT COUNT(*), {COST_COLUMNS}
         FROM usage_record u
         LEFT JOIN cost_estimate c ON c.usage_id = u.id AND c.pricing_revision = ?3
         WHERE u.canonical = 1 AND u.extent_start < ?2 AND u.extent_end > ?1
           AND (u.extent_start < ?1 OR u.extent_end > ?2)"
    );
    Ok(conn.query_row(&sql, params![start, end, revision], |r| {
        Ok((r.get(0)?, cost_sum(r, 1)?))
    })?)
}

/// The latest provider-reported figure per field of a session.
pub fn reported_costs(conn: &Connection, session_id: &str) -> Result<Vec<ReportedRow>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, source_field, value_as_stated, value_nano, currency, semantics, scope,
                observation_id, observed_at
         FROM reported_cost WHERE session_id = ?1 ORDER BY source_field",
    )?;
    let rows = stmt.query_map([session_id], |r| {
        Ok(ReportedRow {
            session_id: r.get(0)?,
            source_field: r.get(1)?,
            value_as_stated: r.get(2)?,
            value_nano: r.get(3)?,
            currency: r.get(4)?,
            semantics: r.get(5)?,
            scope: r.get(6)?,
            observation_id: r.get(7)?,
            observed_at: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Coverage intervals (kind level) overlapping `[start, end)`, clipped.
pub fn coverage_in(conn: &Connection, start: i64, end: i64) -> Result<Vec<IntervalRow>> {
    let mut stmt = conn.prepare(
        "SELECT source_kind, MAX(start_at, ?1), MIN(end_at, ?2), state, reason FROM coverage_interval
         WHERE source_id IS NULL AND start_at < ?2 AND end_at > ?1 ORDER BY source_kind, start_at",
    )?;
    let rows = stmt.query_map(params![start, end], |r| {
        Ok(IntervalRow {
            kind: r.get(0)?,
            start_at: r.get(1)?,
            end_at: r.get(2)?,
            state: r.get(3)?,
            reason: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Device-lane intervals overlapping `[start, end)`, clipped.
pub fn device_in(conn: &Connection, start: i64, end: i64) -> Result<Vec<IntervalRow>> {
    let mut stmt = conn.prepare(
        "SELECT 'device', MAX(start_at, ?1), MIN(end_at, ?2), state, NULL FROM device_interval
         WHERE start_at < ?2 AND end_at > ?1 ORDER BY start_at",
    )?;
    let rows = stmt.query_map(params![start, end], |r| {
        Ok(IntervalRow {
            kind: r.get(0)?,
            start_at: r.get(1)?,
            end_at: r.get(2)?,
            state: r.get(3)?,
            reason: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// An atom, its engaged session (if any) and its review proposals `(at, kind)`.
pub type DayAtom = (AtomRow, Option<String>, Vec<(i64, String)>);

/// Human atoms of a reporting day with their engaged session, if any.
pub fn atoms_of_day(conn: &Connection, day: &str) -> Result<Vec<DayAtom>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.day, a.start_at, a.end_at, a.work_status, a.counting_basis, a.unknown_reason,
                a.duration_basis, a.candidate_status, a.candidate_reason, a.candidate_basis, a.start_boundary,
                a.end_boundary, a.reasons, a.decided_by, a.rule_ref, a.derivation_run_id,
                (SELECT c.context_key FROM human_atom_context c WHERE c.atom_id = a.id AND c.role = 'engaged' LIMIT 1)
         FROM human_atom a WHERE a.day = ?1 ORDER BY a.start_at",
    )?;
    let rows: Vec<(AtomRow, Option<String>)> = stmt
        .query_map([day], |r| {
            Ok((
                AtomRow {
                    id: r.get(0)?,
                    day: r.get(1)?,
                    start_at: r.get(2)?,
                    end_at: r.get(3)?,
                    work_status: r.get(4)?,
                    counting_basis: r.get(5)?,
                    unknown_reason: r.get(6)?,
                    duration_basis: r.get(7)?,
                    candidate_status: r.get(8)?,
                    candidate_reason: r.get(9)?,
                    candidate_basis: r.get(10)?,
                    start_boundary: r.get(11)?,
                    end_boundary: r.get(12)?,
                    reasons: r.get(13)?,
                    decided_by: r.get(14)?,
                    rule_ref: r.get(15)?,
                    run: r.get(16)?,
                },
                r.get(17)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut prop =
        conn.prepare("SELECT at, kind FROM human_proposal WHERE atom_id = ?1 ORDER BY at")?;
    let mut out = Vec::new();
    for (a, engaged) in rows {
        let proposals = prop
            .query_map([&a.id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        out.push((a, engaged, proposals));
    }
    Ok(out)
}

/// Every row of every table, sorted, without `derivation_run` and the
/// `derivation_run_id` columns: the canonical dump INV-6 compares (§12.1).
pub fn canonical_dump(conn: &Connection) -> Result<String> {
    let mut out = String::new();
    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT IN ('derivation_run', 'derived_meta')
             ORDER BY name",
        )?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for t in tables {
        let cols: Vec<String> = conn
            .prepare(&format!("SELECT name FROM pragma_table_info('{t}')"))?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|c| c != "derivation_run_id")
            .collect();
        let select = cols
            .iter()
            .map(|c| format!("quote({c})"))
            .collect::<Vec<_>>()
            .join(" || '|' || ");
        let mut stmt = conn.prepare(&format!("SELECT {select} FROM {t} ORDER BY 1"))?;
        let mut lines: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<_>>()?;
        lines.sort();
        out.push_str(&format!("## {t}\n"));
        for l in lines {
            out.push_str(&l);
            out.push('\n');
        }
    }
    Ok(out)
}

/// The report-target attribution of a subject: `(level, state, targets JSON)`.
pub fn report_attribution(
    conn: &Connection,
    subject_id: &str,
) -> Result<Option<(String, String, String)>> {
    Ok(conn
        .query_row(
            "SELECT level, state, COALESCE(targets, '[]') FROM attribution WHERE subject_id = ?1 AND report_target = 1",
            [subject_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
}

/// `(repository_key, repository_label)` of a session.
pub fn session_repository(
    conn: &Connection,
    session_id: &str,
) -> Result<(Option<String>, Option<String>)> {
    Ok(conn
        .query_row(
            "SELECT repository_key, repository_label FROM agent_session WHERE id = ?1",
            [session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or((None, None)))
}

/// Stream-level extents (gaps, degraded) overlapping `[start, end)`, clipped.
pub fn stream_extents_in(conn: &Connection, start: i64, end: i64) -> Result<Vec<IntervalRow>> {
    let mut stmt = conn.prepare(
        "SELECT source_kind, MAX(start_at, ?1), MIN(end_at, ?2), COALESCE(completeness, state), reason
         FROM coverage_interval WHERE source_id IS NOT NULL AND start_at < ?2 AND end_at > ?1 ORDER BY start_at",
    )?;
    let rows = stmt.query_map(params![start, end], |r| {
        Ok(IntervalRow {
            kind: r.get(0)?,
            start_at: r.get(1)?,
            end_at: r.get(2)?,
            state: r.get(3)?,
            reason: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn recovered_records(conn: &Connection, day: &str) -> Result<i64> {
    Ok(conn
        .query_row(
            "SELECT recovered_records FROM day_quality WHERE day = ?1",
            [day],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

#[derive(Debug, Clone)]
pub struct ToolCallTime {
    pub actor_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_kind: Option<String>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

/// Every tool call of a session (names and times only), in time order.
pub fn tool_call_times_of(conn: &Connection, session_id: &str) -> Result<Vec<ToolCallTime>> {
    let mut stmt = conn.prepare(
        "SELECT actor_id, tool_name, tool_kind, started_at, finished_at FROM tool_call
         WHERE session_id = ?1 ORDER BY started_at, id",
    )?;
    let rows = stmt.query_map([session_id], |r| {
        Ok(ToolCallTime {
            actor_id: r.get(0)?,
            tool_name: r.get(1)?,
            tool_kind: r.get(2)?,
            started_at: r.get(3)?,
            finished_at: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[derive(Debug, Clone)]
pub struct UsagePointRow {
    pub actor_id: Option<String>,
    pub model: Option<String>,
    pub at: i64,
    pub input_fresh: Option<i64>,
    pub input_total: Option<i64>,
    pub cache_read: Option<i64>,
    pub cache_creation: Option<i64>,
    pub output: Option<i64>,
}

/// Canonical, identified usage records of a session in time order (what each request carried).
pub fn usage_points_of(conn: &Connection, session_id: &str) -> Result<Vec<UsagePointRow>> {
    let mut stmt = conn.prepare(
        "SELECT actor_id, model, extent_start, input_fresh, input_total, cache_read, cache_creation, output
         FROM usage_record
         WHERE session_id = ?1 AND canonical = 1 AND identified = 1 AND extent_start IS NOT NULL
         ORDER BY extent_start, id",
    )?;
    let rows = stmt.query_map([session_id], |r| {
        Ok(UsagePointRow {
            actor_id: r.get(0)?,
            model: r.get(1)?,
            at: r.get(2)?,
            input_fresh: r.get(3)?,
            input_total: r.get(4)?,
            cache_read: r.get(5)?,
            cache_creation: r.get(6)?,
            output: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Every engagement (human prompt) instant of a session.
pub fn all_instants_of(conn: &Connection, session_id: &str) -> Result<Vec<i64>> {
    let mut stmt =
        conn.prepare("SELECT at FROM engagement_instant WHERE session_id = ?1 ORDER BY at, id")?;
    let rows = stmt.query_map([session_id], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
