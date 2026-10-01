//! Derivation of `derived.sqlite` (architecture §3.1, §7.1, §8.1): a full
//! rebuild into a fresh file, or an in-place update of the dirty units.
//!
//! ledger observations + facts
//!   → within-stream record election (one observation per record key: the first)
//!   → provider normalizer per stream (events, within-stream usage election)
//!   → copy election across streams (core::copy)
//!   → agent timelines per session (core::agent)
//!   → derived.next.sqlite, installed by rename.
//!
//! Pure with respect to the ledger and `as_of`: the same ledger gives the
//! same rows (INV-6), whether rebuilt or updated.
//!
//! **Dirty units** (§7.1): sessions with transcript observations past the
//! watermark, closed over every session that shares a native record with
//! them (copy election decides between those streams together). Coverage,
//! the device lane and the human timeline are recomputed as a whole: they
//! are small and depend on `as_of`. Copied records stay as events with `copied_from` and
//! usage rows with `duplicate_of`; nothing is derived from them.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::Serialize;
use tracr_core::adapter::StreamRef;
use tracr_core::agent::{SegmentParams, SessionTimeline, TimedEvent, actor_id, derive_session};
use tracr_core::copy::{CopyElection, StreamOrder};
use tracr_core::events::FactKind;
use tracr_core::evidence::{Facts, TimeQuality};
use tracr_core::ids::{SourceId, derived_id};
use tracr_core::normalize::{
    ActorRef, EventKind, NormEvent, Normalizer, ObservationInput, StreamOutput,
};
use tracr_core::time::UtcMillis;
use tracr_provider_claude::normalize::ClaudeNormalizer;
use tracr_store::derived::{
    self, ActorRow, DerivedWriter, EventRow, InstantRow, RunRow, SegmentRow, SessionRow,
    ToolCallRow, TurnRow, UsageRow,
};
use tracr_store::ledger::{self, FactRow};

/// The normalizers tracr ships (S1.3: Claude Code transcripts).
pub fn normalizers() -> Vec<Arc<dyn Normalizer>> {
    vec![Arc::new(ClaudeNormalizer::new())]
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RebuildReport {
    pub as_of: String,
    pub obs_watermark: i64,
    pub observations_read: u64,
    /// Later observations of an already-seen record key (e.g. re-appended changed lines).
    pub record_versions_not_elected: u64,
    pub streams: u64,
    pub streams_skipped: u64,
    pub sessions: u64,
    pub actors: u64,
    pub turns: u64,
    pub tool_calls: u64,
    pub segments: u64,
    pub engagement_instants: u64,
    pub events: u64,
    pub copied_events: u64,
    pub copied_records: u64,
    pub usage_records: u64,
    pub usage_duplicates: u64,
    pub usage_unidentified: u64,
    pub diagnostics: BTreeMap<String, u64>,
    pub human: crate::derive_human::HumanReport,
}

struct Stream {
    session_native: String,
    machine: String,
    order: StreamOrder,
    inputs: Vec<ObservationInput>,
    output: StreamOutput,
    normalizer: usize,
}

fn fact_kind(text: &str) -> Option<FactKind> {
    serde_json::from_value(serde_json::Value::String(text.to_owned())).ok()
}

/// One observation per record key (the first in file order), each with its
/// latest fact row. Returns the inputs and the number of skipped versions.
fn elect_records(rows: &[&FactRow]) -> (Vec<ObservationInput>, u64) {
    let mut latest: Vec<&FactRow> = Vec::new();
    for r in rows {
        match latest.last_mut() {
            Some(prev) if prev.id == r.id => *prev = r, // rows are ordered by extractor version
            _ => latest.push(r),
        }
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut skipped = 0;
    let mut out = Vec::new();
    for r in latest {
        if !seen.insert(r.record_key.as_str()) {
            skipped += 1;
            continue;
        }
        let (Some(kind), Ok(facts)) = (
            fact_kind(&r.kind),
            serde_json::from_str::<Facts>(&r.facts_json),
        ) else {
            skipped += 1;
            continue;
        };
        out.push(ObservationInput {
            observation_id: r.id.clone(),
            seq: r.seq,
            record_key: r.record_key.clone(),
            occurred_at: r.occurred_at,
            observed_at: r.observed_at,
            producer_version: r.producer_version.clone(),
            manifest_validated: !r
                .manifest_range
                .as_deref()
                .is_some_and(|m| m.starts_with("unvalidated")),
            kind,
            subtype: r.subtype.clone(),
            time_quality: TimeQuality::parse(&r.time_quality).unwrap_or(TimeQuality::Receipt),
            facts,
        });
    }
    (out, skipped)
}

fn event_id(observation_id: &str, normalizer_version: &str, ordinal: u32) -> String {
    derived_id(
        "evt_",
        &[observation_id, normalizer_version, &ordinal.to_string()],
    )
}

fn opt_json<T: Serialize>(v: &T, empty: bool) -> Option<String> {
    (!empty).then(|| serde_json::to_string(v).expect("serializable"))
}

fn basename(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_owned()
}

/// Rebuilds `derived.sqlite` in `data_dir` from `ledger` into a fresh file.
/// The caller holds `tracr.lock`.
pub fn rebuild(ledger: &Connection, data_dir: &Path, as_of: UtcMillis) -> Result<RebuildReport> {
    let mut conn = derived::create_next(data_dir)?;
    let report = derive_into(ledger, &mut conn, as_of, None)?;
    drop(conn);
    derived::install_next(data_dir).context("install derived store")?;
    Ok(report)
}

/// Updates the installed derived store in place: only dirty sessions are
/// re-derived. `Ok(None)` when a full rebuild is needed instead (no store,
/// another schema version, or no watermark). The caller holds `tracr.lock`.
pub fn update(
    ledger: &Connection,
    data_dir: &Path,
    as_of: UtcMillis,
) -> Result<Option<RebuildReport>> {
    let Some(mut conn) = derived::open_writer(data_dir)? else {
        return Ok(None);
    };
    let Some(watermark) =
        derived::meta(&conn, "obs_watermark")?.and_then(|v| v.parse::<i64>().ok())
    else {
        return Ok(None);
    };
    // A new pricing catalog re-estimates every record: rebuild.
    if derived::meta(&conn, "pricing_revision")?.as_deref()
        != Some(crate::pricing::bundled().revision.as_str())
    {
        return Ok(None);
    }
    let scope = dirty_sources(ledger, watermark)?;
    Ok(Some(derive_into(ledger, &mut conn, as_of, Some(&scope))?))
}

/// Sources to re-derive: those with new transcript observations, expanded to
/// their whole sessions and closed over copy-election peers.
fn dirty_sources(ledger: &Connection, watermark: i64) -> Result<BTreeSet<String>> {
    let mut session_of: HashMap<String, String> = HashMap::new();
    let mut sources_of: HashMap<String, Vec<String>> = HashMap::new();
    for s in ledger::sources(ledger, None)? {
        let stream: Option<StreamRef> = s
            .stream_meta
            .as_deref()
            .and_then(|m| serde_json::from_str(m).ok());
        if let Some(native) = stream.and_then(|st| st.native_session) {
            let key = format!("{}|{}|{native}", s.kind, s.machine_id);
            session_of.insert(s.id.to_string(), key.clone());
            sources_of.entry(key).or_default().push(s.id.to_string());
        }
    }
    let mut scope: BTreeSet<String> = BTreeSet::new();
    let mut frontier: Vec<String> = ledger::sources_with_transcript_since(ledger, watermark)?;
    while !frontier.is_empty() {
        let mut added: Vec<String> = Vec::new();
        for src in frontier {
            let whole = session_of
                .get(&src)
                .and_then(|k| sources_of.get(k))
                .cloned()
                .unwrap_or_else(|| vec![src.clone()]);
            for s in whole {
                if scope.insert(s.clone()) {
                    added.push(s);
                }
            }
        }
        frontier = ledger::sources_sharing_records(ledger, &added)?
            .into_iter()
            .filter(|s| !scope.contains(s))
            .collect();
    }
    Ok(scope)
}

fn derive_into(
    ledger: &Connection,
    conn: &mut Connection,
    as_of: UtcMillis,
    scope: Option<&BTreeSet<String>>,
) -> Result<RebuildReport> {
    let normalizers = normalizers();
    let pricing = crate::pricing::bundled();
    let params = SegmentParams::default();
    let rule_ref = params.rule_ref();
    let mut report = RebuildReport {
        as_of: as_of.to_rfc3339(),
        obs_watermark: ledger::max_observation_seq(ledger)?,
        ..RebuildReport::default()
    };

    // 1. Read, elect records within each stream, normalize.
    let mut streams: Vec<Stream> = Vec::new();
    for (ni, n) in normalizers.iter().enumerate() {
        let kind = n.source_kind();
        let rows = match scope {
            None => ledger::transcript_fact_rows(ledger, kind.as_str())?,
            Some(scope) => {
                let mut rows = Vec::new();
                for src in scope {
                    rows.extend(ledger::transcript_fact_rows_of_source(ledger, src)?);
                }
                rows
            }
        };
        report.observations_read += rows.len() as u64;
        let mut by_source: BTreeMap<&str, Vec<&FactRow>> = BTreeMap::new();
        for r in &rows {
            by_source.entry(r.source_id.as_str()).or_default().push(r);
        }
        let sources: HashMap<String, ledger::SourceRow> =
            ledger::sources(ledger, Some(kind.as_str()))?
                .into_iter()
                .map(|s| (s.id.to_string(), s))
                .collect();
        for (source_id, rows) in by_source {
            let Some(source) = sources.get(source_id) else {
                report.streams_skipped += 1;
                continue;
            };
            let stream: Option<StreamRef> = source
                .stream_meta
                .as_deref()
                .and_then(|m| serde_json::from_str(m).ok());
            let Some(stream) = stream else {
                report.streams_skipped += 1;
                *report
                    .diagnostics
                    .entry("stream_meta_unreadable".into())
                    .or_default() += 1;
                continue;
            };
            let Some(session_native) = n.session_of(&stream) else {
                report.streams_skipped += 1;
                *report
                    .diagnostics
                    .entry("stream_without_session".into())
                    .or_default() += 1;
                continue;
            };
            let (inputs, skipped) = elect_records(&rows);
            report.record_versions_not_elected += skipped;
            let birth = ledger::locators(ledger, &SourceId::new(source_id))?
                .iter()
                .filter_map(|l| l.birth_time)
                .min();
            let output = n.normalize_stream(&stream, &inputs);
            streams.push(Stream {
                order: StreamOrder {
                    stream_key: source.stream_key.clone(),
                    birth,
                    min_seq: inputs.first().map(|i| i.seq).unwrap_or(i64::MAX),
                },
                session_native,
                machine: source.machine_id.to_string(),
                inputs,
                output,
                normalizer: ni,
            });
        }
    }
    report.streams = streams.len() as u64;

    // 2. Copy election over native record IDs (content-derived keys are not identities).
    let record_sets: Vec<BTreeSet<String>> = streams
        .iter()
        .map(|s| {
            s.inputs
                .iter()
                .filter(|i| !i.record_key.starts_with("h:"))
                .map(|i| i.record_key.clone())
                .collect()
        })
        .collect();
    let election = CopyElection::new(
        streams.iter().map(|s| s.order.clone()).collect(),
        &record_sets,
    );
    let obs_by_key: Vec<HashMap<&str, &str>> = streams
        .iter()
        .map(|s| {
            s.inputs
                .iter()
                .map(|i| (i.record_key.as_str(), i.observation_id.as_str()))
                .collect()
        })
        .collect();
    // Canonical stream of an observation, if it is a copy.
    let copy_of = |si: usize, key: &str| -> Option<usize> {
        if key.starts_with("h:") {
            return None;
        }
        election.canonical_record_stream(key).filter(|&c| c != si)
    };

    // Usage identities across streams (per provider).
    let mut identity_streams: BTreeMap<(usize, String), Vec<usize>> = BTreeMap::new();
    for (si, s) in streams.iter().enumerate() {
        for u in &s.output.usage {
            if let Some(id) = &u.identity {
                identity_streams
                    .entry((s.normalizer, id.clone()))
                    .or_default()
                    .push(si);
            }
        }
    }

    // 3. Sessions.
    let mut sessions: BTreeMap<(usize, String, String), Vec<usize>> = BTreeMap::new();
    for (si, s) in streams.iter().enumerate() {
        sessions
            .entry((s.normalizer, s.machine.clone(), s.session_native.clone()))
            .or_default()
            .push(si);
    }

    let zone = crate::day::ReportingZone::resolve(
        ledger::current_setting(ledger, crate::TIMEZONE_KEY)?
            .as_ref()
            .and_then(|v| v.as_str()),
    )?;
    {
        let w = DerivedWriter::begin(conn)?;
        if scope.is_some() {
            for (ni, machine, native) in sessions.keys() {
                w.delete_session(&derived_id(
                    "ses_",
                    &[normalizers[*ni].provider(), machine, native],
                ))?;
            }
            w.clear_human()?;
        }
        for ((ni, machine, native), members) in &sessions {
            let n = &normalizers[*ni];
            let provider = n.provider();
            let session_id = derived_id("ses_", &[provider, machine, native]);
            let versions = serde_json::json!({
                "normalizer": n.normalizer_version(),
                "segment_rule": rule_ref,
                "derived_schema": derived::DERIVED_SCHEMA_VERSION,
            })
            .to_string();

            let mut fp = blake3::Hasher::new();
            let mut diagnostics: BTreeMap<String, u64> = BTreeMap::new();
            let mut timed: Vec<TimedEvent> = Vec::new();
            let mut all_events: Vec<(usize, &NormEvent, String, Option<String>)> = Vec::new();
            for &si in members {
                let s = &streams[si];
                for i in &s.inputs {
                    fp.update(i.observation_id.as_bytes());
                }
                for (k, v) in &s.output.diagnostics {
                    *diagnostics.entry(k.clone()).or_default() += v;
                }
                let mut copied_records: BTreeSet<usize> = BTreeSet::new();
                for e in &s.output.events {
                    let input = &s.inputs[e.observation];
                    let id = event_id(&input.observation_id, n.normalizer_version(), e.ordinal);
                    let copied_from = copy_of(si, &input.record_key).and_then(|c| {
                        obs_by_key[c]
                            .get(input.record_key.as_str())
                            .map(|o| event_id(o, n.normalizer_version(), e.ordinal))
                    });
                    if copied_from.is_some() {
                        copied_records.insert(e.observation);
                    } else if let Some(at) = e.at {
                        timed.push(TimedEvent {
                            id: id.clone(),
                            order: input.seq * 1000 + i64::from(e.ordinal),
                            kind: e.kind,
                            subtype: e.subtype.clone(),
                            actor: e.actor.clone(),
                            at,
                            native_id: e.native_id.clone(),
                            tool: e.tool.clone(),
                            context: e.context.clone(),
                        });
                    }
                    all_events.push((si, e, id, copied_from));
                }
                let copied = s
                    .inputs
                    .iter()
                    .filter(|i| copy_of(si, &i.record_key).is_some())
                    .count() as u64;
                if copied > 0 {
                    *diagnostics.entry("copied_records".into()).or_default() += copied;
                    report.copied_records += copied;
                }
            }
            let fingerprint = fp.finalize();
            let run_id = derived_id(
                "run_",
                &[
                    "session",
                    &session_id,
                    &fingerprint.to_hex(),
                    &versions,
                    &as_of.0.to_string(),
                ],
            );
            w.run(&RunRow {
                id: run_id.clone(),
                unit: "session".into(),
                unit_key: session_id.clone(),
                as_of: as_of.0,
                obs_watermark: report.obs_watermark,
                versions: versions.clone(),
                input_fingerprint: fingerprint.as_bytes().to_vec(),
            })?;

            let tl: SessionTimeline = derive_session(&session_id, &timed, &params);

            // Session row; context from the earliest canonical event that states a cwd.
            let first_ctx = timed
                .iter()
                .filter(|e| e.context.as_ref().is_some_and(|c| c.cwd.is_some()))
                .min_by_key(|e| (e.at, e.order))
                .and_then(|e| e.context.clone());
            let cwd = first_ctx.as_ref().and_then(|c| c.cwd.clone());
            w.session(&SessionRow {
                id: session_id.clone(),
                provider: provider.to_owned(),
                machine_id: machine.clone(),
                native_session_id: native.clone(),
                first_evidence_at: tl.first_evidence.map(|t| t.0),
                last_evidence_at: tl.last_evidence.map(|t| t.0),
                // No session_ended in S1: a session ends by absence of evidence (§13.2).
                end_basis: tl.last_evidence.map(|_| "last_evidence".to_owned()),
                diagnostics: opt_json(&diagnostics, diagnostics.is_empty()),
                label: cwd.as_deref().map(basename),
                cwd,
                git_branch: first_ctx.and_then(|c| c.git_branch),
                run: run_id.clone(),
            })?;
            report.sessions += 1;

            for a in &tl.actors {
                w.actor(&ActorRow {
                    id: actor_id(&session_id, a).expect("agent actor"),
                    session_id: session_id.clone(),
                    actor_kind: a.actor_kind().expect("agent actor").to_owned(),
                    native_agent_id: match a {
                        ActorRef::Known(id) => Some(id.clone()),
                        _ => None,
                    },
                    run: run_id.clone(),
                })?;
                report.actors += 1;
            }
            for t in &tl.turns {
                w.turn(&TurnRow {
                    id: t.id.clone(),
                    session_id: session_id.clone(),
                    actor_id: actor_id(&session_id, &t.actor),
                    native_turn_id: t.native_turn_id.clone(),
                    ordinal: i64::from(t.ordinal),
                    origin: t.origin.clone(),
                    started_at: t.started_at.0,
                    ended_at: t.ended_at.0,
                    end_kind: t.end_kind.map(str::to_owned),
                    end_basis: t.end_basis.to_owned(),
                    run: run_id.clone(),
                })?;
                report.turns += 1;
            }
            for c in &tl.tool_calls {
                w.tool_call(&ToolCallRow {
                    id: c.id.clone(),
                    session_id: session_id.clone(),
                    actor_id: actor_id(&session_id, &c.actor),
                    turn_id: c.turn_id.clone(),
                    native_id: c.native_id.clone(),
                    tool_name: c.name.clone(),
                    tool_kind: c.kind.as_str().to_owned(),
                    started_at: c.started_at.0,
                    finished_at: c.finished_at.map(|t| t.0),
                    run: run_id.clone(),
                })?;
                report.tool_calls += 1;
            }
            for g in &tl.segments {
                w.segment(&SegmentRow {
                    id: g.id.clone(),
                    session_id: session_id.clone(),
                    actor_id: actor_id(&session_id, &g.actor).expect("agent actor"),
                    turn_id: g.turn_id.clone(),
                    start_at: g.start.0,
                    end_at: g.end.0,
                    start_basis: g.start_basis.to_owned(),
                    end_basis: g.end_basis.to_owned(),
                    kind: "execution".into(),
                    rule_ref: rule_ref.clone(),
                    run: run_id.clone(),
                })?;
                report.segments += 1;
            }
            for i in &tl.instants {
                w.instant(&InstantRow {
                    id: i.id.clone(),
                    at: i.at.0,
                    session_id: session_id.clone(),
                    event_id: i.event_id.clone(),
                    subtype: Some(tracr_core::events::PROMPT_SUBMITTED.into()),
                    context: i
                        .context
                        .as_ref()
                        .map(|c| serde_json::to_string(c).expect("json")),
                    run: run_id.clone(),
                })?;
                report.engagement_instants += 1;
            }
            for (si, e, id, copied_from) in &all_events {
                let input = &streams[*si].inputs[e.observation];
                w.event(&EventRow {
                    id: id.clone(),
                    kind: e.kind.as_str().to_owned(),
                    subtype: e.subtype.clone(),
                    provider: provider.to_owned(),
                    session_id: Some(session_id.clone()),
                    actor_id: actor_id(&session_id, &e.actor),
                    turn_id: tl.event_turns.get(id).cloned(),
                    occurred_at: e.at.map(|t| t.0),
                    time_quality: e.time_quality.as_str().to_owned(),
                    observed_at: input.observed_at.0,
                    native_id: e.native_id.clone(),
                    context: e
                        .context
                        .as_ref()
                        .map(|c| serde_json::to_string(c).expect("json")),
                    model: e.model.as_ref().map(|m| m.0.clone()),
                    model_source: e.model.as_ref().map(|m| m.1.as_str().to_owned()),
                    normalizer_version: n.normalizer_version().to_owned(),
                    copied_from: copied_from.clone(),
                    quality: (!input.manifest_validated)
                        .then(|| r#"["producer_version_not_fixture_validated"]"#.to_owned()),
                    observation_id: input.observation_id.clone(),
                    run: run_id.clone(),
                })?;
                report.events += 1;
                report.copied_events += u64::from(copied_from.is_some());
            }

            // Usage rows: one per occurrence in a stream (§6.1).
            for &si in members {
                let s = &streams[si];
                let model_event: HashMap<usize, u32> = s
                    .output
                    .events
                    .iter()
                    .filter(|e| e.kind == EventKind::ModelObserved)
                    .map(|e| (e.observation, e.ordinal))
                    .collect();
                for u in &s.output.usage {
                    let input = &s.inputs[u.observation];
                    let occurrence = u
                        .identity
                        .clone()
                        .unwrap_or_else(|| input.record_key.clone());
                    let id = derived_id("use_", &[provider, &s.order.stream_key, &occurrence]);
                    let canonical_stream = match &u.identity {
                        Some(identity) => {
                            let group = &identity_streams[&(s.normalizer, identity.clone())];
                            let mut distinct = group.clone();
                            distinct.sort_unstable();
                            distinct.dedup();
                            (distinct.len() > 1)
                                .then(|| election.canonical(&distinct))
                                .filter(|&c| c != si)
                        }
                        None => copy_of(si, &input.record_key),
                    };
                    let duplicate_of = canonical_stream.map(|c| {
                        derived_id(
                            "use_",
                            &[provider, &streams[c].order.stream_key, &occurrence],
                        )
                    });
                    let turn_id = model_event
                        .get(&u.observation)
                        .map(|&o| event_id(&input.observation_id, n.normalizer_version(), o))
                        .and_then(|eid| tl.event_turns.get(&eid).cloned());
                    let f = &u.usage;
                    let (start, end) = match f.extent {
                        tracr_core::evidence::TemporalExtent::Instant { at } => {
                            (Some(at.0), Some(at.0))
                        }
                        tracr_core::evidence::TemporalExtent::Interval { from, to } => {
                            (Some(from.0), Some(to.0))
                        }
                        tracr_core::evidence::TemporalExtent::Unknown => (None, None),
                    };
                    let tok = |v: Option<u64>| v.map(|x| i64::try_from(x).unwrap_or(i64::MAX));
                    report.usage_records += 1;
                    report.usage_duplicates += u64::from(duplicate_of.is_some());
                    report.usage_unidentified += u64::from(u.identity.is_none());
                    let row = UsageRow {
                        id,
                        provider: provider.to_owned(),
                        stream_key: s.order.stream_key.clone(),
                        dedup_identity: u.identity.clone(),
                        identified: u.identity.is_some(),
                        duplicate_of,
                        session_id: Some(session_id.clone()),
                        actor_id: actor_id(&session_id, &u.actor),
                        turn_id,
                        model: u.model.as_ref().map(|m| m.0.clone()),
                        model_source: Some(
                            u.model
                                .as_ref()
                                .map(|m| m.1.as_str())
                                .unwrap_or("unknown")
                                .to_owned(),
                        ),
                        service_tier: f.service_tier.clone(),
                        // The elected record states the request's usage: one delta per request.
                        semantics: "delta".into(),
                        scope: f.scope.as_str().to_owned(),
                        input_fresh: tok(f.input_fresh),
                        input_total: tok(f.input_total),
                        cache_read: tok(f.cache_read),
                        cache_creation: tok(f.cache_creation),
                        cache_creation_tiers: opt_json(
                            &f.cache_creation_tiers,
                            f.cache_creation_tiers.is_empty(),
                        ),
                        output: tok(f.output),
                        reasoning_output: tok(f.reasoning_output),
                        tool_charges: opt_json(&f.tool_charges, f.tool_charges.is_empty()),
                        extent_start: start,
                        extent_end: end,
                        election_rule: u.election_rule.to_owned(),
                        quality: (!input.manifest_validated)
                            .then(|| r#"["producer_version_not_fixture_validated"]"#.to_owned()),
                        sources: u
                            .sources
                            .iter()
                            .map(|&i| s.inputs[i].observation_id.clone())
                            .collect(),
                        run: run_id.clone(),
                    };
                    w.usage(&row)?;
                    let e = tracr_core::pricing::estimate(
                        &pricing.catalog,
                        &tracr_core::pricing::PricedUsage {
                            provider,
                            model: row.model.as_deref(),
                            service_tier: row.service_tier.as_deref(),
                            // Not retained by the extractor yet: stated as assumptions.
                            speed: None,
                            inference_geo: None,
                            at: start,
                            input_fresh: f.input_fresh,
                            input_total: f.input_total,
                            cache_read: f.cache_read,
                            cache_creation: f.cache_creation,
                            cache_creation_tiers: Some(&f.cache_creation_tiers),
                            output: f.output,
                            tool_charges: Some(&f.tool_charges),
                        },
                    );
                    let list = |v: &Vec<String>| opt_json(v, v.is_empty());
                    w.cost(&derived::CostRow {
                        usage_id: row.id.clone(),
                        session_id: row.session_id.clone(),
                        pricing_revision: pricing.revision.clone(),
                        currency: pricing.catalog.currency.clone(),
                        status: e.status.as_str().to_owned(),
                        amount_nano: e
                            .amount
                            .as_ref()
                            .map(|a| i64::try_from(a.0.nano).unwrap_or(i64::MAX)),
                        priced_tokens: tok(Some(e.priced_tokens)).unwrap_or(0),
                        unpriced_tokens: tok(Some(e.unpriced_tokens)).unwrap_or(0),
                        reasons: list(&e.reasons),
                        assumptions: list(&e.assumptions),
                        price_basis: e.price_basis.clone(),
                        rule_id: e.rule_id.clone(),
                    })?;
                }
            }
            // Provider-reported cost: the latest figure per field, never summed.
            let mut latest: BTreeMap<
                String,
                (i64, i64, String, &tracr_core::evidence::ReportedCostFacts),
            > = BTreeMap::new();
            for &si in members {
                for input in &streams[si].inputs {
                    if let Some(c) = &input.facts.reported_cost {
                        let key = (input.observed_at.0, input.seq);
                        let newer = latest
                            .get(&c.source_field)
                            .is_none_or(|(t, q, _, _)| key > (*t, *q));
                        if newer {
                            latest.insert(
                                c.source_field.clone(),
                                (key.0, key.1, input.observation_id.clone(), c),
                            );
                        }
                    }
                }
            }
            for (field, (observed_at, _, observation_id, c)) in latest {
                w.reported(&derived::ReportedRow {
                    session_id: session_id.clone(),
                    source_field: field,
                    value_as_stated: c.value_as_stated.clone(),
                    value_nano: c.value_nano,
                    currency: c.unit_as_stated.clone(),
                    semantics: c.semantics.as_str().to_owned(),
                    scope: c.scope.as_str().to_owned(),
                    observation_id,
                    observed_at,
                })?;
            }
            for (k, v) in diagnostics {
                *report.diagnostics.entry(k).or_default() += v;
            }
        }
        let agent_kinds: Vec<String> = normalizers
            .iter()
            .map(|n| n.source_kind().as_str().to_owned())
            .collect();
        // The human layer reads every session's instants and segments from the
        // store, so an update sees the sessions it did not re-derive too.
        let instant_rows = w.all_instants()?;
        let agents = crate::derive_human::AgentContext {
            instants: instant_rows
                .iter()
                .map(|(session, at, _)| tracr_core::human::Instant {
                    at: UtcMillis(*at),
                    session: session.clone(),
                })
                .collect(),
            instant_cwds: instant_rows
                .into_iter()
                .map(|(session, at, ctx)| {
                    let cwd = ctx
                        .and_then(|c| {
                            serde_json::from_str::<tracr_core::evidence::ContextFacts>(&c).ok()
                        })
                        .and_then(|c| c.cwd);
                    (session, at, cwd)
                })
                .collect(),
            segments: w.all_segments()?,
            sessions: w.all_session_cwds()?,
        };
        report.human = crate::derive_human::derive(
            ledger,
            &w,
            &zone,
            as_of,
            report.obs_watermark,
            &agent_kinds,
            &agents,
        )?;
        w.meta(
            "schema_version",
            &derived::DERIVED_SCHEMA_VERSION.to_string(),
        )?;
        w.meta("as_of", &as_of.0.to_string())?;
        w.meta("pricing_revision", &pricing.revision)?;
        w.meta("obs_watermark", &report.obs_watermark.to_string())?;
        w.commit()?;
    }
    Ok(report)
}
