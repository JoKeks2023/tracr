//! Claude Code normalizer (architecture §7.1, §8.1 step 2, §13.2).
//!
//! Reads retained facts only: re-running it after a normalizer change needs
//! no transcript content (walkthrough E-1). Per stream it emits:
//!
//! * `user_interaction.prompt_submitted` + `turn_started(human)` for human
//!   prompts, re-decided from markers with the extractor's rule;
//! * `turn_started(automated | agent)` for inputs that start an agent run
//!   without a human (task notifications, scheduled or non-interactive
//!   prompts, a subagent's task prompt). These never yield engagement instants;
//! * `turn_interrupted`, `turn_failed` (API error records), `tool_started`,
//!   `tool_finished`, `model_observed` (each agent output record) and
//!   `cwd_changed`;
//! * one usage candidate per `(requestId, message.id)`: the **last**
//!   usage-bearing record in file order, which undoes streaming re-emission
//!   (walkthrough D: 910, not 1,510). Records without that identity stay
//!   unidentified and are never collapsed (EV-06).

use std::collections::BTreeMap;

use tracr_core::adapter::{StreamRef, StreamRole};
use tracr_core::events::{FactKind, PROMPT_SUBMITTED};
use tracr_core::evidence::{Marker, SourceKind};
use tracr_core::manifest::CapabilityManifest;
use tracr_core::normalize::{
    ActorRef, EventKind, NormEvent, Normalizer, ObservationInput, StreamOutput, TurnOrigin,
    UsageCandidate,
};

use crate::rules::{non_human_reason, prompt_signals, starts_agent_run};
use crate::{PRODUCER, SOURCE_KIND, manifest};

pub const NORMALIZER_VERSION: &str = "claude-normalize/1";
pub const USAGE_ELECTION_RULE: &str = "claude.last_record_per_request_message@1";

#[derive(Debug, Clone)]
pub struct ClaudeNormalizer {
    manifest: CapabilityManifest,
}

impl ClaudeNormalizer {
    pub fn new() -> Self {
        Self {
            manifest: manifest::manifest(),
        }
    }
}

impl Default for ClaudeNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Main-chain records are `unknown` unless the manifest documents the main
/// agent (AD-21); a subagent stream is its agent, `known` by native ID.
fn stream_actor(stream: &StreamRef, manifest: &CapabilityManifest) -> ActorRef {
    match (stream.role, &stream.native_agent) {
        (StreamRole::Subagent, Some(agent)) => ActorRef::Known(agent.clone()),
        (StreamRole::Subagent, None) => ActorRef::Unknown,
        (StreamRole::Main, _) if manifest.main_actor_documented => ActorRef::Main,
        (StreamRole::Main, _) => ActorRef::Unknown,
    }
}

/// Claude usage dedup identity (§6.2): `(requestId, message.id)`, else
/// `message.id`, else unidentified.
fn usage_identity(request: Option<&str>, message: Option<&str>) -> Option<String> {
    match (request, message) {
        (Some(r), Some(m)) => Some(format!("{r}/{m}")),
        (None, Some(m)) => Some(format!("msg/{m}")),
        _ => None,
    }
}

impl Normalizer for ClaudeNormalizer {
    fn source_kind(&self) -> SourceKind {
        SourceKind::new(SOURCE_KIND)
    }

    fn provider(&self) -> &'static str {
        PRODUCER
    }

    fn normalizer_version(&self) -> &'static str {
        NORMALIZER_VERSION
    }

    fn normalize_stream(&self, stream: &StreamRef, records: &[ObservationInput]) -> StreamOutput {
        let actor = stream_actor(stream, &self.manifest);
        let mut out = StreamOutput::default();
        let mut diag = |key: &str| *out.diagnostics.entry(key.to_owned()).or_default() += 1;
        let mut events: Vec<NormEvent> = Vec::new();
        let mut prev_cwd: Option<String> = None;
        // identity → (elected record index, every record index with it)
        let mut identified: BTreeMap<String, (usize, Vec<usize>)> = BTreeMap::new();
        let mut identity_order: Vec<String> = Vec::new();
        let mut usage: Vec<UsageCandidate> = Vec::new();

        for (i, r) in records.iter().enumerate() {
            if !r.manifest_validated {
                diag("producer_version_not_fixture_validated");
            }
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

            match r.kind {
                FactKind::UserInteraction | FactKind::AgentInput => {
                    let signals = prompt_signals(&r.facts);
                    let record = Some(r.record_key.clone());
                    if non_human_reason(&signals, &self.manifest, false).is_none() {
                        push(
                            &mut events,
                            EventKind::UserInteraction,
                            Some(PROMPT_SUBMITTED),
                            ActorRef::Human,
                            record.clone(),
                        );
                        push(
                            &mut events,
                            EventKind::TurnStarted,
                            Some(TurnOrigin::Human.as_str()),
                            actor.clone(),
                            record,
                        );
                    } else if let Some(origin) = starts_agent_run(&signals, &self.manifest) {
                        if non_human_reason(&signals, &self.manifest, true).is_none() {
                            // Human-shaped prompt from a non-interactive entrypoint.
                            diag("prompt_from_non_interactive_entrypoint");
                        }
                        push(
                            &mut events,
                            EventKind::TurnStarted,
                            Some(origin),
                            actor.clone(),
                            record,
                        );
                    }
                }
                FactKind::TurnInterrupted => {
                    push(
                        &mut events,
                        EventKind::TurnInterrupted,
                        None,
                        actor.clone(),
                        Some(r.record_key.clone()),
                    );
                }
                FactKind::ToolFinished => {
                    let ids: Vec<Option<String>> = if r.facts.tool_results.is_empty() {
                        vec![None]
                    } else {
                        r.facts
                            .tool_results
                            .iter()
                            .map(|t| t.tool_use_id.clone())
                            .collect()
                    };
                    for id in ids {
                        push(
                            &mut events,
                            EventKind::ToolFinished,
                            None,
                            actor.clone(),
                            id,
                        );
                    }
                }
                FactKind::AgentMessage => {
                    let e = push(
                        &mut events,
                        EventKind::ModelObserved,
                        None,
                        actor.clone(),
                        Some(r.record_key.clone()),
                    );
                    events[e].model = r
                        .facts
                        .model
                        .as_ref()
                        .map(|m| (m.id.clone().unwrap_or_default(), m.source))
                        .filter(|(id, _)| !id.is_empty());
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
                    if matches!(
                        r.facts.markers.get("is_api_error_message"),
                        Some(Marker::Bool(true))
                    ) {
                        push(
                            &mut events,
                            EventKind::TurnFailed,
                            None,
                            actor.clone(),
                            Some(r.record_key.clone()),
                        );
                    }
                    if let Some(u) = &r.facts.usage {
                        let identity = usage_identity(
                            u.identity.request_id.as_deref(),
                            u.identity.message_id.as_deref(),
                        );
                        match identity {
                            Some(id) => {
                                let entry = identified.entry(id.clone()).or_insert_with(|| {
                                    identity_order.push(id);
                                    (i, Vec::new())
                                });
                                entry.0 = i; // the last record in file order wins
                                entry.1.push(i);
                            }
                            None => usage.push(UsageCandidate {
                                observation: i,
                                sources: vec![i],
                                identity: None,
                                actor: actor.clone(),
                                model: None,
                                usage: u.clone(),
                                election_rule: USAGE_ELECTION_RULE,
                            }),
                        }
                    }
                }
                _ => {}
            }
        }

        for id in identity_order {
            let (elected, sources) = identified.remove(&id).expect("recorded identity");
            let usage_facts = records[elected]
                .facts
                .usage
                .clone()
                .expect("elected record carries usage");
            usage.push(UsageCandidate {
                observation: elected,
                sources,
                identity: Some(id),
                actor: actor.clone(),
                model: None,
                usage: usage_facts,
                election_rule: USAGE_ELECTION_RULE,
            });
        }
        for u in &mut usage {
            u.model = records[u.observation]
                .facts
                .model
                .as_ref()
                .and_then(|m| m.id.clone().map(|id| (id, m.source)));
        }
        usage.sort_by_key(|u| u.observation);
        out.events = events;
        out.usage = usage;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_identity_follows_the_natural_key() {
        assert_eq!(usage_identity(Some("r"), Some("m")).as_deref(), Some("r/m"));
        assert_eq!(usage_identity(None, Some("m")).as_deref(), Some("msg/m"));
        assert_eq!(usage_identity(Some("r"), None), None);
        assert_eq!(usage_identity(None, None), None);
    }
}
