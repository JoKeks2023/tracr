//! Normalized events and the normalizer trait (spec event model §4–§5,
//! architecture §7.1, §8.1, §13.2).
//!
//! A provider normalizer turns one stream's retained facts into normalized
//! events and usage candidates, electing usage *within* the stream. Election
//! *across* streams (copied context) and agent timelines are provider-free
//! and live in [`crate::copy`] and [`crate::agent`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::adapter::StreamRef;
use crate::events::FactKind;
use crate::evidence::{
    ContextFacts, Facts, ModelSource, SourceKind, TimeQuality, ToolKind, UsageFacts,
};
use crate::time::UtcMillis;

/// Normalized event kinds emitted in S1.3 (a subset of event model §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Human interaction; subtype `prompt_submitted`. Never carries an agent actor.
    UserInteraction,
    /// Subtype: the [`TurnOrigin`].
    TurnStarted,
    TurnInterrupted,
    TurnFailed,
    ToolStarted,
    ToolFinished,
    /// The agent produced output with this model (one per agent output record).
    ModelObserved,
    CwdChanged,
}

impl EventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserInteraction => "user_interaction",
            Self::TurnStarted => "turn_started",
            Self::TurnInterrupted => "turn_interrupted",
            Self::TurnFailed => "turn_failed",
            Self::ToolStarted => "tool_started",
            Self::ToolFinished => "tool_finished",
            Self::ModelObserved => "model_observed",
            Self::CwdChanged => "cwd_changed",
        }
    }
}

/// What started a turn. Only `Human` turns come with an engagement instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnOrigin {
    /// A human prompt (the same record also yields `user_interaction.prompt_submitted`).
    Human,
    /// A scheduled, notification or non-interactive input on the main chain.
    Automated,
    /// A subagent's own input (its task prompt or a continuation).
    Agent,
}

impl TurnOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Automated => "automated",
            Self::Agent => "agent",
        }
    }
}

/// Who produced an event, as far as the source says (AD-21, AT-09).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ActorRef {
    /// The human; not an agent actor.
    Human,
    /// Main-agent activity whose identity is not documented: the session's
    /// unknown bucket. Never promoted to `main` by guessing.
    Unknown,
    /// The main agent, only where the manifest cites documented semantics.
    Main,
    /// An agent with a native ID from the source.
    Known(String),
}

impl ActorRef {
    /// `known | main | unknown`, or `None` for the human.
    pub fn actor_kind(&self) -> Option<&'static str> {
        match self {
            Self::Human => None,
            Self::Unknown => Some("unknown"),
            Self::Main => Some("main"),
            Self::Known(_) => Some("known"),
        }
    }

    /// Natural key of the actor within its session.
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::Human => None,
            Self::Unknown => Some("unknown"),
            Self::Main => Some("main"),
            Self::Known(id) => Some(id),
        }
    }
}

/// One observation as a normalizer reads it: envelope fields and retained facts.
#[derive(Debug, Clone)]
pub struct ObservationInput {
    pub observation_id: String,
    pub seq: i64,
    pub record_key: String,
    pub occurred_at: Option<UtcMillis>,
    pub observed_at: UtcMillis,
    pub producer_version: Option<String>,
    /// False when the producer version is outside the manifest's validated ranges.
    pub manifest_validated: bool,
    pub kind: FactKind,
    pub subtype: Option<String>,
    pub time_quality: TimeQuality,
    pub facts: Facts,
}

/// A normalized event before copy election. `observation` indexes the
/// stream's input slice; `ordinal` numbers the events of one observation.
#[derive(Debug, Clone, PartialEq)]
pub struct NormEvent {
    pub observation: usize,
    pub ordinal: u32,
    pub kind: EventKind,
    pub subtype: Option<String>,
    pub actor: ActorRef,
    pub at: Option<UtcMillis>,
    pub time_quality: TimeQuality,
    /// Native ID the event is about: the record for a turn start, the call for a tool event.
    pub native_id: Option<String>,
    pub model: Option<(String, ModelSource)>,
    /// Tool name and kind for tool events (never arguments).
    pub tool: Option<(Option<String>, ToolKind)>,
    pub context: Option<ContextFacts>,
}

/// One usage occurrence in a stream after within-stream election (§8.1 step 2).
#[derive(Debug, Clone, PartialEq)]
pub struct UsageCandidate {
    /// Index of the elected observation in the stream's input slice.
    pub observation: usize,
    /// Every observation in this stream that reported this identity (provenance).
    pub sources: Vec<usize>,
    /// Provider dedup identity; `None` means unidentified (never collapsed, EV-06).
    pub identity: Option<String>,
    pub actor: ActorRef,
    pub model: Option<(String, ModelSource)>,
    pub usage: UsageFacts,
    /// Name and version of the rule that elected this occurrence.
    pub election_rule: &'static str,
}

/// Everything a normalizer produces for one stream.
#[derive(Debug, Clone, Default)]
pub struct StreamOutput {
    pub events: Vec<NormEvent>,
    pub usage: Vec<UsageCandidate>,
    /// Counted diagnostics, e.g. records from producer versions that are not fixture-validated.
    pub diagnostics: BTreeMap<String, u64>,
}

/// A provider normalizer (§1.2). Pure: no IO, no clock.
pub trait Normalizer: Send + Sync {
    fn source_kind(&self) -> SourceKind;

    /// Provider name used in session natural keys.
    fn provider(&self) -> &'static str;

    fn normalizer_version(&self) -> &'static str;

    /// The native session a stream belongs to (its session unit).
    fn session_of(&self, stream: &StreamRef) -> Option<String> {
        stream.native_session.clone()
    }

    /// Normalizes one stream. `records` are the stream's elected
    /// observations in file order (one per record key).
    fn normalize_stream(&self, stream: &StreamRef, records: &[ObservationInput]) -> StreamOutput;
}
