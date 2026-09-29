//! Source registry, collector runs and collector records (§4.2, §4.4).

use std::sync::Arc;

use rusqlite::Connection;
use tracr_core::adapter::{ObservationAdapter, StreamRef};
use tracr_core::evidence::{
    Channel, CollectorFacts, Facts, IngestTrigger, NativeIds, ParseStatus, RetentionClass,
    SourceKind, TimeQuality,
};
use tracr_core::ids::{ContentHash, MachineId, SourceId};
use tracr_store::ledger::{self, InsertOutcome, NewObservation, NewSource, SourceRow};

use crate::{IngestEnv, IngestError, Result};

/// Adapters registered by the application (§1.2). Ingestion reaches provider
/// formats only through the core adapter trait.
#[derive(Clone, Default)]
pub struct Registry {
    adapters: Vec<Arc<dyn ObservationAdapter>>,
}

impl Registry {
    pub fn new(adapters: Vec<Arc<dyn ObservationAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn adapters(&self) -> &[Arc<dyn ObservationAdapter>] {
        &self.adapters
    }

    pub fn for_kind(&self, kind: &str) -> Option<&Arc<dyn ObservationAdapter>> {
        self.adapters
            .iter()
            .find(|a| a.source_kind().as_str() == kind)
    }
}

/// Opaque UUIDv7 whose timestamp comes from the injected clock.
pub fn new_uuid_v7(env: &IngestEnv<'_>) -> String {
    let ms = u64::try_from(env.clock.now().0).unwrap_or(0);
    let ts =
        uuid::Timestamp::from_unix(uuid::NoContext, ms / 1000, ((ms % 1000) * 1_000_000) as u32);
    uuid::Uuid::new_v7(ts).to_string()
}

/// The stream a source was discovered as (stored in `source.stream_meta`).
pub fn stream_ref(source: &SourceRow) -> Result<StreamRef> {
    let meta = source
        .stream_meta
        .as_deref()
        .ok_or_else(|| IngestError::StreamMeta(source.id.to_string()))?;
    serde_json::from_str(meta).map_err(|_| IngestError::StreamMeta(source.id.to_string()))
}

/// One collector run. CLI imports begin with an `import_started` record whose
/// ID every observation of the run carries as `collector_run_id` (§4.4). It
/// asserts no coverage: no sensor or watcher runs during an import.
pub struct RunContext {
    pub run_id: String,
    pub machine_id: MachineId,
    pub collector_source: SourceId,
    receipt_seq: i64,
}

impl RunContext {
    pub fn begin_import(
        conn: &mut Connection,
        env: &IngestEnv<'_>,
        machine: &MachineId,
        client: &str,
    ) -> Result<Self> {
        let collector = ensure_collector_source(conn, env, machine)?;
        let run_id = new_uuid_v7(env);
        let mut run = Self {
            run_id: run_id.clone(),
            machine_id: machine.clone(),
            collector_source: collector.id,
            receipt_seq: 0,
        };
        let tx = conn.transaction()?;
        insert_collector(
            &tx,
            env,
            &mut run,
            CollectorFacts::ImportStarted {
                client: client.to_owned(),
            },
            IngestTrigger::Backfill,
            Some(run_id.clone()),
            Some(run_id),
        )?;
        tx.commit()?;
        Ok(run)
    }

    /// Monotonic receipt order within this run.
    pub fn next_receipt(&mut self) -> i64 {
        self.receipt_seq += 1;
        self.receipt_seq
    }
}

pub fn ensure_collector_source(
    conn: &Connection,
    env: &IngestEnv<'_>,
    machine: &MachineId,
) -> Result<SourceRow> {
    let (row, _) = ledger::ensure_source(
        conn,
        &NewSource {
            id: &new_uuid_v7(env),
            kind: SourceKind::COLLECTOR,
            machine_id: machine,
            stream_key: &format!("collector:{machine}"),
            adapter: "tracr-ingest",
            adapter_version: env!("CARGO_PKG_VERSION"),
            created_at: env.clock.now(),
            stream_meta: None,
        },
    )?;
    Ok(row)
}

/// Writes a record about the collector's own work. `record_key` makes
/// idempotent diagnostics (the same partial line or key conflict is recorded
/// once); by default each record gets a fresh key.
pub fn write_collector_record(
    conn: &Connection,
    env: &IngestEnv<'_>,
    run: &mut RunContext,
    facts: CollectorFacts,
    trigger: IngestTrigger,
    record_key: Option<String>,
) -> Result<InsertOutcome> {
    insert_collector(conn, env, run, facts, trigger, record_key, None)
}

fn insert_collector(
    conn: &Connection,
    env: &IngestEnv<'_>,
    run: &mut RunContext,
    facts: CollectorFacts,
    trigger: IngestTrigger,
    record_key: Option<String>,
    observation_id: Option<String>,
) -> Result<InsertOutcome> {
    let kind = facts.kind();
    let body = Facts {
        collector: Some(facts),
        ..Facts::default()
    };
    let facts_json = serde_json::to_string(&body)?;
    let hash = ContentHash::of(facts_json.as_bytes());
    let id = observation_id.unwrap_or_else(|| new_uuid_v7(env));
    let key = record_key.unwrap_or_else(|| id.clone());
    let now = env.clock.now();
    let native = NativeIds::default();
    let receipt = run.next_receipt();
    Ok(ledger::insert_observation(
        conn,
        &NewObservation {
            id: &id,
            source_id: &run.collector_source,
            record_key: &key,
            content_hash: &hash,
            channel: Channel::Collector,
            locator_path: None,
            cursor_offset: None,
            producer: Some("tracr"),
            producer_version: Some(env!("CARGO_PKG_VERSION")),
            adapter_version: env!("CARGO_PKG_VERSION"),
            manifest_range: None,
            native: &native,
            occurred_at: Some(now),
            observed_at: now,
            collector_run_id: &run.run_id,
            receipt_seq: receipt,
            ingest_trigger: trigger,
            parse_status: ParseStatus::Ok,
            retention_class: RetentionClass::EvidenceMetadata,
            extractor_version: concat!("tracr-ingest/", env!("CARGO_PKG_VERSION")),
            kind: kind.as_str(),
            subtype: None,
            time_quality: TimeQuality::Source,
            facts_json: &facts_json,
        },
    )?)
}
