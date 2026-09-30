//! Full rebuild of `derived.sqlite` (architecture §3.1, §7.1, §8.1).
//!
//! ledger observations + facts
//!   → within-stream record election (one observation per record key: the first)
//!   → provider normalizer per stream (events, within-stream usage election)
//!   → copy election across streams (core::copy)
//!   → agent timelines per session (core::agent)
//!   → derived.next.sqlite, installed by rename.
//!
//! Pure with respect to the ledger and `as_of`: the same ledger gives the
//! same rows (INV-6). Copied records stay as events with `copied_from` and
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

/// Rebuilds `derived.sqlite` in `data_dir` from `ledger`. The caller holds `tracr.lock`.
pub fn rebuild(ledger: &Connection, data_dir: &Path, as_of: UtcMillis) -> Result<RebuildReport> {
    let normalizers = normalizers();
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
        let rows = ledger::transcript_fact_rows(ledger, kind.as_str())?;
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
    let mut agents = crate::derive_human::AgentContext {
        instants: Vec::new(),
        segments: Vec::new(),
    };
    let mut conn = derived::create_next(data_dir)?;
    {
        let w = DerivedWriter::begin(&mut conn)?;
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
            agents
                .instants
                .extend(tl.instants.iter().map(|i| tracr_core::human::Instant {
                    at: i.at,
                    session: session_id.clone(),
                }));
            agents.segments.extend(
                tl.segments
                    .iter()
                    .map(|g| (session_id.clone(), g.start.0, g.end.0)),
            );

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
                    w.usage(&UsageRow {
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
                    })?;
                }
            }
            for (k, v) in diagnostics {
                *report.diagnostics.entry(k).or_default() += v;
            }
        }
        let agent_kinds: Vec<String> = normalizers
            .iter()
            .map(|n| n.source_kind().as_str().to_owned())
            .collect();
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
        w.meta("obs_watermark", &report.obs_watermark.to_string())?;
        w.commit()?;
    }
    drop(conn);
    derived::install_next(data_dir).context("install derived store")?;
    Ok(report)
}
