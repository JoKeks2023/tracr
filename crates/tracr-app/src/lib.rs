//! Application use cases (architecture §1.1). Hosts (CLI, desktop) talk
//! only to this crate. It owns the single-writer lock, wires the concrete
//! adapters into ingestion and derivation, and exposes the use cases:
//! approving roots, importing, rebuilding the derived store, and reading
//! statistics and day views.

pub mod correct;
pub mod day;
pub mod derive;
pub mod derive_human;
pub mod export;
pub mod hooks;
pub mod limits;
pub mod manual;
pub mod pricing;
pub mod projects;
pub mod quota;
pub mod report;
pub mod review;
pub mod runtime;
pub mod trace;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use rusqlite::Connection;
use serde::Serialize;
use tracr_core::evidence::{Channel, IngestTrigger};
use tracr_core::ids::MachineId;
use tracr_ingest::source::{RunContext, new_uuid_v7};
use tracr_ingest::{
    DiscoveryReport, GapInfo, IngestEnv, ReconcileOptions, Registry, discover, reconcile,
    record_failed_run,
};
use tracr_platform::{Clock, FileIdentity, file_identity};
use tracr_provider_claude::ClaudeAdapter;
use tracr_provider_codex::CodexAdapter;
use tracr_store::connection::{LEDGER_FILE, open_ledger, open_ledger_read_only};
use tracr_store::ledger::{self, NewUserAction, SourceSummary, UsageRecordStats};
use tracr_store::migrate::{self, migrate_ledger};

pub use tracr_core::time::UtcMillis;
pub use tracr_platform::SystemClock;

pub const LOCK_FILE: &str = "tracr.lock";

/// The platform's default data directory (§3.1), if it has one.
pub fn default_data_dir() -> Option<PathBuf> {
    tracr_platform::app_paths().data_dir()
}
pub const PROVIDER_ROOTS_KEY: &str = "approved_provider_roots";
pub const TIMEZONE_KEY: &str = "timezone";

/// Proposed provider roots that exist on this machine (PS-02: proposals only;
/// nothing is read until the user approves one).
pub fn proposed_provider_roots() -> Vec<PathBuf> {
    tracr_platform::app_paths()
        .proposed_provider_roots()
        .into_iter()
        .filter(|p| p.is_dir())
        .collect()
}
pub const REPOSITORY_ROOTS_KEY: &str = "approved_repository_roots";
const CLIENT: &str = "tracr-cli";

/// The adapters tracr ships (T-0004: Claude Code transcripts).
pub fn registry() -> Registry {
    Registry::new(vec![
        Arc::new(ClaudeAdapter::new()),
        Arc::new(CodexAdapter::new()),
    ])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    Provider,
    Repository,
}

impl RootKind {
    fn key(self) -> &'static str {
        match self {
            Self::Provider => PROVIDER_ROOTS_KEY,
            Self::Repository => REPOSITORY_ROOTS_KEY,
        }
    }
}

fn read_roots(conn: &Connection, kind: RootKind) -> Result<Vec<String>> {
    Ok(match ledger::current_setting(conn, kind.key())? {
        Some(serde_json::Value::Array(items)) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        Some(other) => bail!("setting {} is not a list: {other}", kind.key()),
        None => Vec::new(),
    })
}

/// A writer: holds `tracr.lock` for its whole lifetime (AD-03 single writer).
pub struct App {
    data_dir: PathBuf,
    conn: Connection,
    machine: MachineId,
    clock: Box<dyn Clock>,
    fid: Box<dyn FileIdentity>,
    registry: Registry,
    client: &'static str,
    _lock: File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootsChange {
    Added(Vec<String>),
    AlreadyApproved(Vec<String>),
    /// The root is no longer approved. Nothing already imported is deleted.
    Removed(Vec<String>),
    NotApproved(Vec<String>),
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportReport {
    pub run_id: String,
    pub roots: Vec<String>,
    pub missing_roots: Vec<String>,
    pub files_seen: u64,
    pub files_claimed: u64,
    pub files_unclaimed: u64,
    pub new_sources: u64,
    pub unreadable_dirs: u64,
    pub sources_reconciled: u64,
    pub inserted: u64,
    pub duplicates: u64,
    pub quarantined: u64,
    pub key_conflicts: u64,
    pub incomplete_trailing: u64,
    pub ended: u64,
    /// Sources whose file was not found while discovery was incomplete.
    pub missing: u64,
    pub full_rereads: u64,
    pub inserted_by_kind: BTreeMap<String, u64>,
    pub ignored_by_type: BTreeMap<String, u64>,
    pub unvalidated_versions: BTreeSet<String>,
    pub gaps: Vec<GapSummary>,
    pub errors: Vec<String>,
    pub provider_observations_before: u64,
    pub provider_observations_after: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GapSummary {
    pub source_id: String,
    pub start: String,
    pub end: String,
    pub reason: String,
}

impl App {
    /// Opens `data_dir` for writing: takes the lock, creates and migrates the ledger.
    pub fn open_writer(data_dir: &Path, clock: Box<dyn Clock>) -> Result<Self> {
        fs::create_dir_all(data_dir).with_context(|| format!("create {}", data_dir.display()))?;
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(data_dir.join(LOCK_FILE))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                bail!(
                    "another tracr process holds {}",
                    data_dir.join(LOCK_FILE).display()
                )
            }
            Err(TryLockError::Error(e)) => return Err(e.into()),
        }
        let mut conn = open_ledger(&data_dir.join(LEDGER_FILE))?;
        let stamp = clock.now().to_rfc3339().replace(':', "-");
        migrate_ledger(&mut conn, &data_dir.join("backups"), &stamp)?;
        pricing::bundled().record(&conn, clock.now())?;
        let fid = file_identity();
        let machine = {
            let env = IngestEnv {
                clock: &*clock,
                file_identity: &*fid,
            };
            ledger::ensure_machine(&conn, &new_uuid_v7(&env), None, clock.now())?
        };
        Ok(Self {
            data_dir: data_dir.to_path_buf(),
            conn,
            machine,
            clock,
            fid,
            registry: registry(),
            client: CLIENT,
            _lock: lock,
        })
    }

    /// Names the host in user actions and runs (`tracr-cli`, `tracr-desktop`).
    pub fn with_client(mut self, client: &'static str) -> Self {
        self.client = client;
        self
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn machine(&self) -> &MachineId {
        &self.machine
    }

    /// Records an approved provider or repository root in `setting_rev` (PS-02).
    pub fn add_root(&mut self, kind: RootKind, path: &Path) -> Result<RootsChange> {
        let canonical =
            fs::canonicalize(path).with_context(|| format!("resolve {}", path.display()))?;
        if !canonical.is_dir() {
            bail!("{} is not a directory", canonical.display());
        }
        let text = canonical
            .to_str()
            .ok_or_else(|| anyhow!("path is not valid UTF-8"))?
            .to_owned();
        let mut roots = read_roots(&self.conn, kind)?;
        if roots.contains(&text) {
            return Ok(RootsChange::AlreadyApproved(roots));
        }
        roots.push(text);
        roots.sort();
        self.write_roots(kind, "add", &roots)?;
        Ok(RootsChange::Added(roots))
    }

    /// Stops reading a root (PS-02: approval is the user's and so is its
    /// withdrawal). The ledger is append-only, so what was imported stays;
    /// only future discovery changes. The path may be gone from disk.
    pub fn remove_root(&mut self, kind: RootKind, path: &Path) -> Result<RootsChange> {
        let mut roots = read_roots(&self.conn, kind)?;
        let given = path.to_str().map(str::to_owned);
        let resolved = fs::canonicalize(path)
            .ok()
            .and_then(|c| c.to_str().map(str::to_owned));
        let Some(at) = roots
            .iter()
            .position(|r| Some(r) == given.as_ref() || Some(r) == resolved.as_ref())
        else {
            return Ok(RootsChange::NotApproved(roots));
        };
        roots.remove(at);
        self.write_roots(kind, "remove", &roots)?;
        Ok(RootsChange::Removed(roots))
    }

    /// Chooses the reporting time zone (AD-14: one fixed home zone), or `None`
    /// to follow the system zone. The derived store must be rebuilt afterwards,
    /// because day boundaries move. Refused once a day has been reviewed: a
    /// reviewed revision records the zone it was cut in.
    pub fn set_timezone(&mut self, name: Option<&str>) -> Result<()> {
        if let Some(n) = name {
            jiff::tz::TimeZone::get(n).with_context(|| format!("unknown time zone {n:?}"))?;
        }
        let current = ledger::current_setting(&self.conn, TIMEZONE_KEY)?;
        if current.as_ref().and_then(|v| v.as_str()) == name {
            return Ok(());
        }
        let reviewed = ledger::reviewed_day_count(&self.conn)?;
        if reviewed > 0 {
            bail!(
                "{}; it can only be changed while no day is reviewed",
                if reviewed == 1 {
                    "1 reviewed day keeps its time zone".to_owned()
                } else {
                    format!("{reviewed} reviewed days keep their time zone")
                }
            );
        }
        let env = IngestEnv {
            clock: &*self.clock,
            file_identity: &*self.fid,
        };
        let tx = self.conn.transaction()?;
        let seq = ledger::append_user_action(
            &tx,
            &NewUserAction {
                id: &new_uuid_v7(&env),
                kind: "setting.timezone.set",
                actor: "user",
                at: self.clock.now(),
                reason: None,
                client: self.client,
            },
        )?;
        ledger::append_setting(&tx, TIMEZONE_KEY, &serde_json::json!(name), seq)?;
        tx.commit()?;
        Ok(())
    }

    fn write_roots(&mut self, kind: RootKind, action: &str, roots: &[String]) -> Result<()> {
        self.write_settings(
            &format!("setting.{}.{action}", kind.key()),
            &[(kind.key(), serde_json::json!(roots))],
        )
    }

    /// Appends one user action and the settings it changes, in one transaction.
    fn write_settings(
        &mut self,
        action_kind: &str,
        entries: &[(&str, serde_json::Value)],
    ) -> Result<()> {
        let env = IngestEnv {
            clock: &*self.clock,
            file_identity: &*self.fid,
        };
        let tx = self.conn.transaction()?;
        let seq = ledger::append_user_action(
            &tx,
            &NewUserAction {
                id: &new_uuid_v7(&env),
                kind: action_kind,
                actor: "user",
                at: self.clock.now(),
                reason: None,
                client: self.client,
            },
        )?;
        for (key, value) in entries {
            ledger::append_setting(&tx, key, value, seq)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Creates, renames, deletes projects and maps repositories to them (S3.1).
    /// Returns the project id the operation concerns. A view only: no minutes
    /// move, so nothing needs rebuilding.
    pub fn project_op(&mut self, op: &projects::ProjectOp) -> Result<String> {
        let mut config = projects::read_config(&self.conn)?;
        let id = {
            let env = IngestEnv {
                clock: &*self.clock,
                file_identity: &*self.fid,
            };
            projects::apply(&mut config, op, || format!("prj_{}", new_uuid_v7(&env)))?
        };
        self.write_settings(
            &format!("setting.projects.{}", op.action()),
            &[
                (projects::PROJECTS_KEY, serde_json::json!(config.projects)),
                (
                    projects::REPOSITORY_PROJECTS_KEY,
                    serde_json::json!(config.repository_projects),
                ),
            ],
        )?;
        Ok(id)
    }

    /// Backfill (§4.5): discovery over approved provider roots, then one
    /// reconcile per source, oldest first. Idempotent.
    pub fn import(&mut self, progress: &mut dyn FnMut(u64, u64)) -> Result<ImportReport> {
        if read_roots(&self.conn, RootKind::Provider)?.is_empty() {
            bail!("no approved provider roots; run `tracr roots add --provider <path>` first");
        }
        let env = IngestEnv {
            clock: &*self.clock,
            file_identity: &*self.fid,
        };
        let mut run = RunContext::begin_import(&mut self.conn, &env, &self.machine, self.client)?;
        self.reconcile_all(&mut run, IngestTrigger::Backfill, progress)
    }

    /// Discovery over the approved provider roots, then one reconcile per
    /// source (oldest first) with `trigger`, recorded under `run`.
    pub fn reconcile_all(
        &mut self,
        run: &mut RunContext,
        trigger: IngestTrigger,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<ImportReport> {
        let roots: Vec<PathBuf> = read_roots(&self.conn, RootKind::Provider)?
            .into_iter()
            .map(PathBuf::from)
            .collect();
        if roots.is_empty() {
            bail!("no approved provider roots; run `tracr roots add --provider <path>` first");
        }
        let env = IngestEnv {
            clock: &*self.clock,
            file_identity: &*self.fid,
        };
        let mut report = ImportReport {
            provider_observations_before: ledger::count_observations(
                &self.conn,
                Some(Channel::Collector),
            )?,
            run_id: run.run_id.clone(),
            ..ImportReport::default()
        };
        let d: DiscoveryReport = discover(
            &mut self.conn,
            &env,
            &self.registry,
            run,
            &self.machine,
            &roots,
            trigger,
        )?;
        report.roots = d
            .roots
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        report.missing_roots = d
            .missing_roots
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        (
            report.files_seen,
            report.files_claimed,
            report.files_unclaimed,
            report.new_sources,
            report.unreadable_dirs,
        ) = (
            d.files_seen,
            d.files_claimed,
            d.files_unclaimed,
            d.new_sources,
            d.unreadable_dirs,
        );

        // A file discovery did not find ends its source (§4.3) only if this
        // discovery walked every approved root completely. A missing root
        // (e.g. an unmounted volume) or an unreadable directory leaves such
        // sources `missing`; ending them would be permanent.
        let discovery_confirmed = d.missing_roots.is_empty() && d.unreadable_dirs == 0;

        // Oldest first (§4.6): by the earliest known creation time of the source's files.
        let mut sources: Vec<(Option<i64>, tracr_store::ledger::SourceRow)> = Vec::new();
        for s in ledger::sources(&self.conn, None)? {
            if self.registry.for_kind(&s.kind).is_none() {
                continue;
            }
            let birth = ledger::locators(&self.conn, &s.id)?
                .iter()
                .filter_map(|l| l.birth_time.or(l.first_seen).map(|t| t.0))
                .min();
            sources.push((birth, s));
        }
        sources.sort_by(|a, b| (a.0, &a.1.stream_key).cmp(&(b.0, &b.1.stream_key)));
        let total = sources.len() as u64;
        for (i, (_, source)) in sources.iter().enumerate() {
            progress(i as u64 + 1, total);
            match reconcile(
                &mut self.conn,
                &env,
                &self.registry,
                run,
                source,
                trigger,
                &ReconcileOptions {
                    discovery_confirmed,
                    ..ReconcileOptions::default()
                },
            ) {
                Ok(r) => {
                    report.sources_reconciled += 1;
                    report.inserted += r.inserted;
                    report.duplicates += r.duplicates;
                    report.quarantined += r.quarantined;
                    report.key_conflicts += r.key_conflicts;
                    report.incomplete_trailing += u64::from(r.incomplete_trailing);
                    report.ended += u64::from(r.ended);
                    report.missing += u64::from(r.mode == tracr_ingest::ReadMode::Missing);
                    report.full_rereads += u64::from(r.mode == tracr_ingest::ReadMode::FullReread);
                    for (k, n) in r.inserted_by_kind {
                        *report.inserted_by_kind.entry(k).or_default() += n;
                    }
                    for (k, n) in r.ignored_by_type {
                        *report.ignored_by_type.entry(k).or_default() += n;
                    }
                    report.unvalidated_versions.extend(r.unvalidated_versions);
                    if let Some(GapInfo { start, end, reason }) = r.gap {
                        report.gaps.push(GapSummary {
                            source_id: source.id.to_string(),
                            start: start.to_rfc3339(),
                            end: end.to_rfc3339(),
                            reason,
                        });
                    }
                }
                Err(e) => {
                    // One unreadable source never aborts the import; the failure is evidence too.
                    report.errors.push(format!("{}: {e}", source.id));
                    record_failed_run(&mut self.conn, &env, run, source, trigger, &e)?;
                }
            }
        }
        report.provider_observations_after =
            ledger::count_observations(&self.conn, Some(Channel::Collector))?;
        Ok(report)
    }
}

impl App {
    /// Rebuilds `derived.sqlite` from the ledger (§3.1). Only the lock holder rebuilds.
    pub fn rebuild(&mut self) -> Result<derive::RebuildReport> {
        derive::rebuild(&self.conn, &self.data_dir, self.clock.now())
    }

    /// Brings `derived.sqlite` up to date: an in-place update of the dirty
    /// units, or a full rebuild if the store is missing or outdated.
    pub fn update_derived(&mut self) -> Result<derive::RebuildReport> {
        match derive::update(&self.conn, &self.data_dir, self.clock.now())? {
            Some(report) => Ok(report),
            None => self.rebuild(),
        }
    }
}

// ---------------------------------------------------------------------------
// Readers (no lock; never create or migrate)
// ---------------------------------------------------------------------------

pub struct Reader {
    conn: Connection,
    data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub machine: Option<String>,
    pub schema_version: i64,
    pub approved_provider_roots: Vec<String>,
    pub approved_repository_roots: Vec<String>,
    pub sources_by_kind: Vec<(String, i64)>,
    pub ended_sources: u64,
    pub observations_by_channel: Vec<(String, i64)>,
    pub transcript_kinds: Vec<(String, i64)>,
    pub parse_status: Vec<(String, i64)>,
    pub quarantined: Vec<(String, i64)>,
    pub producer_versions: Vec<(String, i64)>,
    pub ignored_lines: Vec<(String, i64)>,
    pub collector_records: Vec<(String, i64)>,
    pub coverage_gaps: Vec<GapSummary>,
    pub usage_records: UsageRecordStatsDto,
}

/// Counts of usage-bearing records, not token totals (election is S1.3).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct UsageRecordStatsDto {
    pub usage_bearing_observations: u64,
    pub distinct_identities: u64,
    pub without_identity: u64,
}

impl From<UsageRecordStats> for UsageRecordStatsDto {
    fn from(u: UsageRecordStats) -> Self {
        Self {
            usage_bearing_observations: u.usage_bearing_observations,
            distinct_identities: u.distinct_identities,
            without_identity: u.without_identity,
        }
    }
}

impl Reader {
    pub fn open(data_dir: &Path) -> Result<Self> {
        let conn = open_ledger_read_only(&data_dir.join(LEDGER_FILE))?;
        if migrate::user_version(&conn)? < 1 {
            bail!(
                "ledger at {} is not initialized; run `tracr import`",
                data_dir.display()
            );
        }
        Ok(Self {
            conn,
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// The reporting timezone (AD-14): the `timezone` setting, else the system zone.
    pub fn reporting_zone(&self) -> Result<day::ReportingZone> {
        let setting = ledger::current_setting(&self.conn, TIMEZONE_KEY)?;
        day::ReportingZone::resolve(setting.as_ref().and_then(|v| v.as_str()))
    }

    fn derived(&self) -> Result<Connection> {
        tracr_store::derived::open_read_only(&self.data_dir)?
            .ok_or_else(|| anyhow!("no derived store yet; run `tracr rebuild`"))
    }

    /// `tracr-export_<local date>_<HHMM>` for an everything export.
    pub fn export_folder_name(&self) -> Result<String> {
        let zone = self.reporting_zone()?;
        let now = jiff::Timestamp::now().to_zoned(zone.tz);
        Ok(format!("tracr-export_{}", now.strftime("%Y-%m-%d_%H%M")))
    }

    /// Writes every derived table as CSV into `dir` (export everything).
    pub fn export_everything(&self, dir: &Path) -> Result<Vec<(String, usize)>> {
        export::export_everything(&self.derived()?, &self.reporting_zone()?, dir)
    }

    /// Days with derived agent activity, newest first.
    pub fn days(&self) -> Result<day::DayList> {
        day::days(&self.derived()?, &self.reporting_zone()?)
    }

    /// Agent activity of one reporting day (`YYYY-MM-DD`), every session individually.
    /// Quarantined and key-conflict records observed within the day.
    fn ledger_counts(&self, d: &day::DayAgents) -> Result<report::LedgerCounts> {
        let n = |kind: &str| -> Result<i64> {
            Ok(self.conn.query_row(
                "SELECT COUNT(*) FROM observation o JOIN observation_facts f ON f.observation_id = o.id
                 WHERE f.kind = ?1 AND o.observed_at >= ?2 AND o.observed_at < ?3",
                rusqlite::params![kind, d.start, d.end],
                |r| r.get(0),
            )?)
        };
        Ok(report::LedgerCounts {
            quarantined: n("quarantined")?,
            key_conflicts: n("key_content_conflict")?,
        })
    }

    /// Sessions active between two reporting days (inclusive), newest first.
    pub fn sessions(&self, from: &str, to: &str) -> Result<day::SessionList> {
        let mut list = day::sessions(&self.derived()?, &self.reporting_zone()?, from, to)?;
        if !quota::load_prefs(&self.data_dir).show_automated {
            // Automated sessions (e.g. Codex's auto-review) are not work the user does: out of the list and its total.
            list.sessions.retain(|s| !s.automated);
            let mut cost = day::CostView::default();
            for s in &list.sessions {
                cost.absorb(&s.cost);
            }
            list.cost = cost;
        }
        Ok(list)
    }

    /// One session laid out in time: actors, prompts, tool calls, request sizes.
    pub fn session_trace(&self, session_id: &str) -> Result<trace::SessionTrace> {
        trace::session_trace(&self.derived()?, &self.reporting_zone()?, session_id)
    }

    /// Limit figures the providers reported (percent of a window), newest known.
    pub fn limits(&self, settings: &Path) -> Result<limits::Limits> {
        let roots: Vec<PathBuf> = self
            .provider_roots()?
            .into_iter()
            .map(PathBuf::from)
            .collect();
        Ok(limits::read(&self.data_dir, &roots, settings))
    }

    /// Sessions of a range grouped by project (S3.1), with repositories, branches and cost.
    pub fn project_overview(&self, from: &str, to: &str) -> Result<projects::ProjectOverview> {
        let list = self.sessions(from, to)?;
        Ok(projects::overview(
            &list,
            &projects::read_config(&self.conn)?,
        ))
    }

    /// The folder name of a session by Claude's own session id (for hook notifications).
    pub fn session_label(&self, native_session_id: &str) -> Option<String> {
        self.derived()
            .ok()?
            .query_row(
                "SELECT label FROM agent_session WHERE native_session_id = ?1 AND label IS NOT NULL LIMIT 1",
                [native_session_id],
                |r| r.get(0),
            )
            .ok()
    }

    pub fn day_agents(&self, day: &str) -> Result<day::DayAgents> {
        let corrections = ledger::corrections(&self.conn)?;
        let manual = ledger::manual_entries(&self.conn)?;
        let mut v = day::day_agents(
            &self.derived()?,
            &self.reporting_zone()?,
            day,
            &corrections,
            &manual,
        )?;
        review::attach(&self.conn, &mut v)?;
        v.ledger_counts = self.ledger_counts(&v)?;
        // Grouped by project from the numbers the day shows: the reviewed
        // snapshot for a reviewed day, the live ones otherwise.
        let shown = v.review.as_ref().map_or(&v.human.counted_by_target, |r| {
            &r.snapshot.counted_by_target
        });
        v.counted_by_project = projects::project_totals(shown, &projects::read_config(&self.conn)?);
        Ok(v)
    }

    /// Targets time can be assigned to (S2): repositories seen in sessions,
    /// the fixed categories, and "Other…" labels used before.
    pub fn targets(&self) -> Result<Vec<day::TargetOption>> {
        let mut out = Vec::new();
        if let Some(d) = tracr_store::derived::open_read_only(&self.data_dir)? {
            let mut stmt = d.prepare(
                "SELECT DISTINCT repository_key FROM agent_session WHERE repository_key IS NOT NULL ORDER BY 1",
            )?;
            let repos: Vec<String> = stmt
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            for k in repos {
                let key = tracr_core::counting::TargetRef::Repository(k).key();
                out.push(day::TargetOption {
                    label: day::target_label(&key),
                    key,
                    group: "repository".into(),
                });
            }
        }
        for c in tracr_core::counting::CATEGORIES {
            let key = tracr_core::counting::TargetRef::Category((*c).to_owned()).key();
            out.push(day::TargetOption {
                label: day::target_label(&key),
                key,
                group: "category".into(),
            });
        }
        let mut others: Vec<String> = ledger::corrections(&self.conn)?
            .iter()
            .filter_map(correct::from_row)
            .filter_map(|c| match c.op {
                tracr_core::counting::CorrectionOp::SetTarget(
                    t @ tracr_core::counting::TargetRef::Other(_),
                ) => Some(t.key()),
                _ => None,
            })
            .collect();
        others.sort();
        others.dedup();
        for key in others {
            out.push(day::TargetOption {
                label: day::target_label(&key),
                key,
                group: "other".into(),
            });
        }
        Ok(out)
    }

    /// The running timer, if any (cheap enough to poll).
    pub fn running_timer(&self) -> Result<Option<day::ManualEntryInfo>> {
        Ok(ledger::manual_entries(&self.conn)?
            .iter()
            .find(|m| m.kind == "timer" && m.end_at.is_none() && !m.deleted)
            .map(day::ManualEntryInfo::from))
    }

    /// Approved provider roots (a single settings read; cheap enough to poll).
    pub fn provider_roots(&self) -> Result<Vec<String>> {
        read_roots(&self.conn, RootKind::Provider)
    }

    /// Days that carry a reviewed revision. They fix the reporting zone (AD-14).
    pub fn reviewed_day_count(&self) -> Result<i64> {
        Ok(ledger::reviewed_day_count(&self.conn)?)
    }

    /// The user's projects and repository mappings (S3.1).
    pub fn project_config(&self) -> Result<projects::ProjectConfig> {
        projects::read_config(&self.conn)
    }

    /// Repositories seen in agent sessions, for mapping them to projects.
    pub fn known_repositories(&self) -> Result<Vec<projects::KnownRepository>> {
        let Some(d) = tracr_store::derived::open_read_only(&self.data_dir)? else {
            return Ok(Vec::new());
        };
        let mut stmt = d.prepare(
            "SELECT DISTINCT repository_key FROM agent_session WHERE repository_key IS NOT NULL ORDER BY 1",
        )?;
        let keys: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(keys
            .into_iter()
            .map(|key| projects::KnownRepository {
                label: tracr_git::repository_label(&key),
                key,
            })
            .collect())
    }

    /// Approved repository roots, under which git is probed read-only.
    pub fn repository_roots(&self) -> Result<Vec<String>> {
        read_roots(&self.conn, RootKind::Repository)
    }

    pub fn sources(&self, kind: Option<&str>) -> Result<Vec<SourceSummary>> {
        Ok(ledger::source_summaries(&self.conn, kind)?)
    }

    pub fn stats(&self) -> Result<Stats> {
        let c = &self.conn;
        let ended: i64 = c.query_row(
            "SELECT COUNT(*) FROM source_cursor WHERE ended_at IS NOT NULL",
            [],
            |r| r.get(0),
        )?;
        Ok(Stats {
            machine: ledger::machine_id(c)?.map(|m| m.to_string()),
            schema_version: migrate::user_version(c)?,
            approved_provider_roots: read_roots(c, RootKind::Provider)?,
            approved_repository_roots: read_roots(c, RootKind::Repository)?,
            sources_by_kind: ledger::source_counts_by_kind(c)?,
            ended_sources: ended.max(0) as u64,
            observations_by_channel: ledger::observation_counts_by_channel(c)?,
            transcript_kinds: ledger::fact_counts(c, Channel::Transcript)?,
            parse_status: ledger::parse_status_counts(c)?,
            quarantined: ledger::quarantine_counts(c)?,
            producer_versions: ledger::producer_version_counts(c)?,
            ignored_lines: ledger::ignored_line_counts(c)?,
            collector_records: ledger::collector_record_counts(c)?,
            coverage_gaps: ledger::coverage_gaps(c)?
                .into_iter()
                .map(|g| GapSummary {
                    source_id: g.source_id,
                    start: g.start.to_rfc3339(),
                    end: g.end.to_rfc3339(),
                    reason: g.reason,
                })
                .collect(),
            usage_records: ledger::usage_record_stats(c)?.into(),
        })
    }
}
