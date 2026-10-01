//! Codex normalizer (architecture §7.1, §8.1 step 2, S1.6).
//!
//! Reads retained facts only. Per thread it emits:
//!
//! * `user_interaction.prompt_submitted` for a `UserMessage` item, only in a
//!   thread whose `session_meta.source` is interactive (AD-19); a user
//!   message elsewhere starts no engagement;
//! * `turn_started` per `task_started` (origin `human` when the turn carries
//!   a human prompt, `agent` in a subagent thread, else `automated`),
//!   `turn_interrupted` per `turn_aborted`, `turn_failed` per failed
//!   `task_complete`;
//! * `tool_started` / `tool_finished` by call ID, `model_observed` per agent
//!   message and per usage record, `cwd_changed`;
//! * usage: one candidate per `token_usage_record` `response_id` (the last
//!   record wins). A thread without any `token_usage_record` (older
//!   producers) turns its cumulative `token_count` totals into deltas, with
//!   unchanged totals suppressed and a decreasing total treated as a reset
//!   (walkthrough D: 0.60 M, not 1.55 M).
//!
//! **Subagent linkage (gated, OQ-I-06).** A thread whose `session_meta`
//! names a parent thread joins the parent's session as actor
//! `known(child thread)` when its producer version is fixture-validated.
//! Otherwise it stays its own session with actor `unknown` and the
//! `unlinked_subagent` diagnostic. Its usage is never moved to the parent's
//! main chain.

use std::collections::{BTreeMap, BTreeSet};

use tracr_core::adapter::StreamRef;
use tracr_core::events::{FactKind, PROMPT_SUBMITTED};
use tracr_core::evidence::{
    Marker, ModelSource, SourceKind, TemporalExtent, UsageFacts, UsageScope, UsageSemantics,
};
use tracr_core::manifest::CapabilityManifest;
use tracr_core::normalize::{
    ActorRef, EventKind, NormEvent, Normalizer, ObservationInput, StreamOutput, TurnOrigin,
    UsageCandidate,
};
use tracr_core::time::UtcMillis;

use crate::manifest::{self, version_core};
use crate::{PRODUCER, SOURCE_KIND};

pub const NORMALIZER_VERSION: &str = "codex-normalize/1";
pub const RECORD_ELECTION_RULE: &str = "codex.last_record_per_response@1";
pub const DELTA_ELECTION_RULE: &str = "codex.cumulative_delta@1";

#[derive(Debug, Clone)]
pub struct CodexNormalizer {
    manifest: CapabilityManifest,
}

impl CodexNormalizer {
    pub fn new() -> Self {
        Self {
            manifest: manifest::manifest(),
        }
    }
}

impl Default for CodexNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

fn marker<'a>(r: &'a ObservationInput, key: &str) -> Option<&'a str> {
    match r.facts.markers.get(key) {
        Some(Marker::Ident(v)) => Some(v.as_str()),
        _ => None,
    }
}

/// What a thread's `session_meta` says about it.
#[derive(Debug, Clone, Default)]
struct ThreadMeta {
    source: Option<String>,
    parent: Option<String>,
    version: Option<String>,
    at: Option<UtcMillis>,
}

fn thread_meta(records: &[ObservationInput]) -> ThreadMeta {
    records
        .iter()
        .find(|r| r.kind == FactKind::SystemEvent && r.subtype.as_deref() == Some("session_meta"))
        .map(|r| ThreadMeta {
            source: marker(r, "source").map(str::to_owned),
            // The parent reference is a retained native ID of the observation.
            parent: r.facts.links.get("parent_thread").cloned(),
            version: r.producer_version.clone(),
            at: r.occurred_at,
        })
        .unwrap_or_default()
}

impl CodexNormalizer {
    fn version_validated(&self, version: Option<&str>) -> bool {
        self.manifest
            .select(version.map(version_core))
            .is_validated()
    }

    /// The parent thread, if this thread links to it (gate on).
    fn linked_parent(&self, meta: &ThreadMeta) -> Option<String> {
        meta.parent
            .clone()
            .filter(|_| self.version_validated(meta.version.as_deref()))
    }
}

struct Totals {
    at: Option<UtcMillis>,
    input: u64,
    cached: u64,
    output: u64,
    reasoning: u64,
}

fn totals(r: &ObservationInput) -> Option<Totals> {
    let u = r.facts.usage.as_ref()?;
    Some(Totals {
        at: r.occurred_at,
        input: u.input_total?,
        cached: u.cache_read.unwrap_or(0),
        output: u.output?,
        reasoning: u.reasoning_output.unwrap_or(0),
    })
}

impl Normalizer for CodexNormalizer {
    fn source_kind(&self) -> SourceKind {
        SourceKind::new(SOURCE_KIND)
    }

    fn provider(&self) -> &'static str {
        PRODUCER
    }

    fn normalizer_version(&self) -> &'static str {
        NORMALIZER_VERSION
    }

    fn session_of(&self, stream: &StreamRef, records: &[ObservationInput]) -> Option<String> {
        self.linked_parent(&thread_meta(records))
            .or_else(|| stream.native_session.clone())
    }

    fn normalize_stream(&self, stream: &StreamRef, records: &[ObservationInput]) -> StreamOutput {
        let meta = thread_meta(records);
        let linked = self.linked_parent(&meta).is_some();
        let is_subagent = meta.source.as_deref() == Some("subagent") || meta.parent.is_some();
        let interactive = !is_subagent && self.manifest.is_interactive(meta.source.as_deref());
        let actor = match (&stream.native_session, linked) {
            (Some(thread), true) => ActorRef::Known(thread.clone()),
            _ if self.manifest.main_actor_documented && !is_subagent => ActorRef::Main,
            _ => ActorRef::Unknown,
        };

        let mut out = StreamOutput::default();
        let mut diag = |key: &str, n: u64| {
            if n > 0 {
                *out.diagnostics.entry(key.to_owned()).or_default() += n;
            }
        };
        if !self.version_validated(meta.version.as_deref()) {
            diag(
                "producer_version_not_fixture_validated",
                records.len() as u64,
            );
        }
        if meta.parent.is_some() && !linked {
            diag("unlinked_subagent", 1);
        }

        // Pass 1: which turns carry a user message; each turn's model.
        let user_turns: BTreeSet<&str> = records
            .iter()
            .filter(|r| r.kind == FactKind::UserInteraction)
            .filter_map(|r| r.facts.links.get("turn").map(String::as_str))
            .collect();
        let model_of_turn: BTreeMap<&str, &str> = records
            .iter()
            .filter(|r| r.subtype.as_deref() == Some("turn_context"))
            .filter_map(|r| {
                let turn = r.facts.links.get("turn")?;
                let model = r.facts.model.as_ref()?.id.as_deref()?;
                Some((turn.as_str(), model))
            })
            .collect();
        let has_usage_records = records
            .iter()
            .any(|r| r.subtype.as_deref() == Some("token_usage_record"));

        let mut events: Vec<NormEvent> = Vec::new();
        let mut usage: Vec<UsageCandidate> = Vec::new();
        let mut by_response: BTreeMap<String, (usize, Vec<usize>)> = BTreeMap::new();
        let mut response_order: Vec<String> = Vec::new();
        let mut prev_cwd: Option<String> = None;
        let mut model: Option<String> = None;
        let mut tier: Option<String> = None;
        let mut last_total: Option<Totals> = None;
        let mut non_interactive_prompts = 0u64;
        let mut suppressed = 0u64;
        let mut resets = 0u64;

        for (i, r) in records.iter().enumerate() {
            let mut ordinal = 0u32;
            let mut push = |events: &mut Vec<NormEvent>,
                            kind: EventKind,
                            subtype: Option<&str>,
                            who: ActorRef,
                            native: Option<String>| {
                events.push(NormEvent {
                    observation: i,
                    ordinal,
                    kind,
                    subtype: subtype.map(str::to_owned),
                    actor: who,
                    at: r.occurred_at,
                    time_quality: r.time_quality,
                    native_id: native,
                    model: None,
                    tool: None,
                    context: r.facts.context.clone(),
                });
                ordinal += 1;
                events.len() - 1
            };
            if let Some(cwd) = r.facts.context.as_ref().and_then(|c| c.cwd.clone()) {
                if prev_cwd.as_ref().is_some_and(|p| *p != cwd) {
                    push(
                        &mut events,
                        EventKind::CwdChanged,
                        None,
                        actor.clone(),
                        None,
                    );
                }
                prev_cwd = Some(cwd);
            }
            let turn = r.facts.links.get("turn").cloned();
            let turn_model = |turn: Option<&String>, current: &Option<String>| {
                turn.and_then(|t| model_of_turn.get(t.as_str()))
                    .map(|m| ((*m).to_owned(), ModelSource::Reported))
                    .or_else(|| current.clone().map(|m| (m, ModelSource::Inferred)))
            };

            match (r.kind, r.subtype.as_deref()) {
                (FactKind::SystemEvent, Some("turn_context" | "thread_settings")) => {
                    if let Some(m) = r.facts.model.as_ref().and_then(|m| m.id.clone()) {
                        model = Some(m);
                    }
                    if let Some(t) = marker(r, "service_tier") {
                        tier = Some(t.to_owned());
                    }
                }
                (FactKind::SystemEvent, Some("task_started")) => {
                    let origin = if is_subagent {
                        TurnOrigin::Agent
                    } else if interactive && turn.as_deref().is_some_and(|t| user_turns.contains(t))
                    {
                        TurnOrigin::Human
                    } else {
                        TurnOrigin::Automated
                    };
                    push(
                        &mut events,
                        EventKind::TurnStarted,
                        Some(origin.as_str()),
                        actor.clone(),
                        turn.clone(),
                    );
                }
                (FactKind::SystemEvent, Some("task_complete")) => {
                    if matches!(r.facts.markers.get("has_error"), Some(Marker::Bool(true))) {
                        push(
                            &mut events,
                            EventKind::TurnFailed,
                            None,
                            actor.clone(),
                            turn.clone(),
                        );
                    }
                }
                (FactKind::TurnInterrupted, _) => {
                    push(
                        &mut events,
                        EventKind::TurnInterrupted,
                        None,
                        actor.clone(),
                        turn.clone(),
                    );
                }
                (FactKind::UserInteraction, _) => {
                    if interactive {
                        push(
                            &mut events,
                            EventKind::UserInteraction,
                            Some(PROMPT_SUBMITTED),
                            ActorRef::Human,
                            Some(r.record_key.clone()),
                        );
                    } else {
                        non_interactive_prompts += 1;
                    }
                }
                (FactKind::AgentMessage, Some("AgentMessage")) => {
                    let e = push(
                        &mut events,
                        EventKind::ModelObserved,
                        None,
                        actor.clone(),
                        Some(r.record_key.clone()),
                    );
                    events[e].model = turn_model(turn.as_ref(), &model);
                }
                (FactKind::AgentMessage, Some("tool_call")) => {
                    for t in &r.facts.tools_started {
                        let e = push(
                            &mut events,
                            EventKind::ToolStarted,
                            None,
                            actor.clone(),
                            t.id.clone(),
                        );
                        events[e].tool = Some((t.name.clone(), t.kind));
                    }
                }
                (FactKind::ToolFinished, _) => {
                    for t in &r.facts.tool_results {
                        push(
                            &mut events,
                            EventKind::ToolFinished,
                            None,
                            actor.clone(),
                            t.tool_use_id.clone(),
                        );
                    }
                }
                (FactKind::AgentMessage, Some("token_usage_record")) => {
                    let e = push(
                        &mut events,
                        EventKind::ModelObserved,
                        None,
                        actor.clone(),
                        Some(r.record_key.clone()),
                    );
                    events[e].model = turn_model(turn.as_ref(), &model);
                    let response = r
                        .facts
                        .usage
                        .as_ref()
                        .and_then(|u| u.identity.request_id.clone());
                    match response {
                        Some(id) => {
                            let entry = by_response.entry(id.clone()).or_insert_with(|| {
                                response_order.push(id);
                                (i, Vec::new())
                            });
                            entry.0 = i;
                            entry.1.push(i);
                        }
                        None => usage.push(UsageCandidate {
                            observation: i,
                            sources: vec![i],
                            identity: None,
                            actor: actor.clone(),
                            model: events[e].model.clone(),
                            usage: with_tier(r.facts.usage.clone().expect("usage"), &tier),
                            election_rule: RECORD_ELECTION_RULE,
                        }),
                    }
                }
                (FactKind::AgentMessage, Some("token_count")) if !has_usage_records => {
                    let Some(cur) = totals(r) else { continue };
                    let delta = match &last_total {
                        Some(p)
                            if p.input == cur.input
                                && p.cached == cur.cached
                                && p.output == cur.output
                                && p.reasoning == cur.reasoning =>
                        {
                            suppressed += 1;
                            continue;
                        }
                        Some(p)
                            if cur.input >= p.input
                                && cur.cached >= p.cached
                                && cur.output >= p.output
                                && cur.reasoning >= p.reasoning =>
                        {
                            (
                                cur.input - p.input,
                                cur.cached - p.cached,
                                cur.output - p.output,
                                cur.reasoning - p.reasoning,
                            )
                        }
                        Some(_) => {
                            resets += 1;
                            (cur.input, cur.cached, cur.output, cur.reasoning)
                        }
                        None => (cur.input, cur.cached, cur.output, cur.reasoning),
                    };
                    let from = last_total.as_ref().and_then(|p| p.at).or(meta.at);
                    let extent = match (from, cur.at) {
                        (Some(from), Some(to)) if from < to => {
                            TemporalExtent::Interval { from, to }
                        }
                        (_, Some(at)) => TemporalExtent::Instant { at },
                        _ => TemporalExtent::Unknown,
                    };
                    let e = push(
                        &mut events,
                        EventKind::ModelObserved,
                        None,
                        actor.clone(),
                        Some(r.record_key.clone()),
                    );
                    events[e].model = model.clone().map(|m| (m, ModelSource::Inferred));
                    let (input, cached, output, reasoning) = delta;
                    usage.push(UsageCandidate {
                        observation: i,
                        sources: vec![i],
                        identity: Some(format!("count/{}", r.record_key)),
                        actor: actor.clone(),
                        model: events[e].model.clone(),
                        usage: with_tier(
                            UsageFacts {
                                semantics: UsageSemantics::Delta,
                                scope: UsageScope::Session,
                                input_fresh: input.checked_sub(cached),
                                input_total: Some(input),
                                cache_read: Some(cached),
                                cache_creation: None,
                                cache_creation_tiers: Default::default(),
                                output: Some(output),
                                reasoning_output: Some(reasoning),
                                tool_charges: Default::default(),
                                service_tier: None,
                                identity: Default::default(),
                                includes_copied_context: None,
                                extent,
                            },
                            &tier,
                        ),
                        election_rule: DELTA_ELECTION_RULE,
                    });
                    last_total = Some(cur);
                }
                _ => {}
            }
        }

        for id in response_order {
            let (elected, sources) = by_response.remove(&id).expect("recorded response");
            let r = &records[elected];
            let turn = r.facts.links.get("turn");
            let model = turn
                .and_then(|t| model_of_turn.get(t.as_str()))
                .map(|m| ((*m).to_owned(), ModelSource::Reported));
            usage.push(UsageCandidate {
                observation: elected,
                sources,
                identity: Some(format!("resp/{id}")),
                actor: actor.clone(),
                model,
                usage: with_tier(r.facts.usage.clone().expect("usage"), &tier),
                election_rule: RECORD_ELECTION_RULE,
            });
        }
        usage.sort_by_key(|u| u.observation);
        diag(
            "prompt_from_non_interactive_source",
            non_interactive_prompts,
        );
        diag("unchanged_total_suppressed", suppressed);
        diag("cumulative_counter_reset", resets);
        out.events = events;
        out.usage = usage;
        out
    }
}

fn with_tier(mut u: UsageFacts, tier: &Option<String>) -> UsageFacts {
    if u.service_tier.is_none() {
        u.service_tier = tier.clone();
    }
    u
}
