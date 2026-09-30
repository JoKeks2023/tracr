//! Observation envelope vocabulary and the retained fact body
//! (spec event model §2–§3, architecture §3.2, §4.4, §11).
//!
//! Retained facts are built from an allowlist of named, non-content fields.
//! The validators in this module are the gate: a value that is not a short
//! identifier, a native ID or bounded context metadata is dropped, never
//! copied. Nothing here can hold prompt, message or tool text.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::events::FactKind;
use crate::time::UtcMillis;

macro_rules! str_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }

            pub fn parse(text: &str) -> Option<Self> {
                match text { $($text => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

str_enum!(
    /// Channel through which an observation arrived (§3.2 `observation.channel`).
    Channel {
        Transcript => "transcript",
        OsSensor => "os_sensor",
        Git => "git",
        Manual => "manual",
        Collector => "collector",
        Hook => "hook",
    }
);

str_enum!(
    ParseStatus {
        Ok => "ok",
        Partial => "partial",
        UnknownFormat => "unknown_format",
        Failed => "failed",
    }
);

str_enum!(
    /// Payload state. tracr 0.9 stores no payload (EV-08): always `never_stored`.
    PayloadState {
        Present => "present",
        Redacted => "redacted",
        Deleted => "deleted",
        NeverStored => "never_stored",
    }
);

str_enum!(
    /// Whether a time was stated by the source, is only a receipt time, or derived.
    TimeQuality {
        Source => "source",
        Receipt => "receipt",
        Derived => "derived",
    }
);

str_enum!(
    /// What caused a read (§4.5). Only `hint` and `dir_hint` are live.
    IngestTrigger {
        Hint => "hint",
        DirHint => "dir_hint",
        Startup => "startup",
        Sweep => "sweep",
        Overflow => "overflow",
        Wake => "wake",
        Discovery => "discovery",
        Backfill => "backfill",
        Sample => "sample",
    }
);

impl IngestTrigger {
    /// Live triggers (§4.7): records first ingested by any other trigger are `recovered`.
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Hint | Self::DirHint)
    }
}

str_enum!(
    RetentionClass {
        EvidenceMetadata => "evidence_metadata",
    }
);

/// Source kind, e.g. a transcript kind declared by an adapter or `collector`.
/// Kept as an opaque string so the core names no provider.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceKind(String);

impl SourceKind {
    pub const COLLECTOR: &'static str = "collector";

    pub fn new(kind: impl Into<String>) -> Self {
        Self(kind.into())
    }

    pub fn collector() -> Self {
        Self(Self::COLLECTOR.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Provider-native stream identity (§4.2). Never a path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StreamKey(String);

impl StreamKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Native identities stated by the source (§3.2). Agent and parent session
/// only where the source provides them; nothing is inferred.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeIds {
    pub session: Option<String>,
    pub parent_session: Option<String>,
    pub agent: Option<String>,
    pub turn: Option<String>,
    pub item: Option<String>,
    pub request: Option<String>,
    pub message: Option<String>,
    pub tool_call: Option<String>,
}

// ---------------------------------------------------------------------------
// Allowlist validators
// ---------------------------------------------------------------------------

fn all_chars(value: &str, max_len: usize, allowed: impl Fn(char) -> bool) -> bool {
    !value.is_empty() && value.len() <= max_len && value.chars().all(allowed)
}

/// Enum-like marker value: at most 64 characters of `[A-Za-z0-9_.:-]`.
pub fn ident(value: &str) -> Option<&str> {
    all_chars(value, 64, |c| {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-')
    })
    .then_some(value)
}

/// Native identifier (UUIDs, request, message and tool-call IDs): at most
/// 128 characters of `[A-Za-z0-9_.:-]`.
pub fn native_id(value: &str) -> Option<&str> {
    all_chars(value, 128, |c| {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-')
    })
    .then_some(value)
}

/// Model identifier: at most 128 characters of `[A-Za-z0-9_.:/@<>[]-]`.
pub fn model_id(value: &str) -> Option<&str> {
    all_chars(value, 128, |c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '_' | '.' | ':' | '/' | '@' | '<' | '>' | '[' | ']' | '-')
    })
    .then_some(value)
}

/// Tool name: at most 128 characters of `[A-Za-z0-9_.:-]`.
pub fn tool_name(value: &str) -> Option<&str> {
    native_id(value)
}

/// Context metadata kept by §11 (paths, branch names): single line, no
/// control characters, bounded length.
pub fn context_text(value: &str, max_len: usize) -> Option<&str> {
    all_chars(value, max_len, |c| !c.is_control()).then_some(value)
}

/// Top-level key names kept for a quarantined record (§4.4): at most 64 keys
/// matching `^[A-Za-z_][A-Za-z0-9_-]{0,63}$`. Values and nested keys are never kept.
pub fn bounded_quarantine_keys<'a>(keys: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = keys
        .into_iter()
        .filter(|k| {
            let mut chars = k.chars();
            matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
                && k.len() <= 64
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out.truncate(64);
    out
}

// ---------------------------------------------------------------------------
// Retained fact body
// ---------------------------------------------------------------------------

/// A structural marker: a boolean, a number or an enum-like identifier.
/// Never text (§11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Marker {
    Bool(bool),
    Int(i64),
    Ident(String),
    Idents(Vec<String>),
}

impl Marker {
    /// An identifier marker, or `None` if the value is not identifier-like.
    pub fn ident(value: &str) -> Option<Self> {
        ident(value).map(|v| Self::Ident(v.to_owned()))
    }
}

/// Retained fact body of one observation (EV-10). Every field is optional;
/// absent means unknown, never zero.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Facts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_type: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub markers: BTreeMap<String, Marker>,
    /// Further native IDs (parent record, prompt, source tool call, …).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub links: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelFacts>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools_started: Vec<ToolStart>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_results: Vec<ToolResultRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_cost: Option<ReportedCostFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<PullRequestFacts>,
    /// A duration the source states (e.g. an agent turn duration). Agent time, never human time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stated_duration_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine: Option<QuarantineFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collector: Option<CollectorFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<crate::device::DeviceFacts>,
}

impl Facts {
    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.markers.insert(key.to_owned(), Marker::Bool(value));
    }

    pub fn set_int(&mut self, key: &str, value: i64) {
        self.markers.insert(key.to_owned(), Marker::Int(value));
    }

    /// Sets an identifier marker if the value is identifier-like; otherwise
    /// records only that a non-identifier value was present.
    pub fn set_ident(&mut self, key: &str, value: &str) {
        match Marker::ident(value) {
            Some(m) => {
                self.markers.insert(key.to_owned(), m);
            }
            None => {
                self.markers
                    .insert(format!("{key}_non_ident"), Marker::Bool(true));
            }
        }
    }

    pub fn set_link(&mut self, key: &str, id: &str) {
        if let Some(id) = native_id(id) {
            self.links.insert(key.to_owned(), id.to_owned());
        }
    }
}

/// Context evidence captured at event time (EV-07).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeFacts>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeFacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_head_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_branch: Option<String>,
}

str_enum!(
    /// What a usage number means (spec event model §6).
    UsageSemantics {
        Delta => "delta",
        Cumulative => "cumulative",
        Snapshot => "snapshot",
    }
);

str_enum!(
    UsageScope {
        Request => "request",
        Turn => "turn",
        Session => "session",
        ContextWindow => "context_window",
    }
);

/// Temporal extent of a usage record (EV-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TemporalExtent {
    Instant { at: UtcMillis },
    Interval { from: UtcMillis, to: UtcMillis },
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageIdentity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
}

/// Usage exactly as the source reports it, with explicit semantics.
/// Each category is `None` when the source does not state it: unknown, not zero (EV-05).
/// These are per-record facts, not totals: election and deduplication happen later (§8.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageFacts {
    pub semantics: UsageSemantics,
    pub scope: UsageScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_fresh: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cache_creation_tiers: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<u64>,
    /// Component of `output` unless the provider states otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_output: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tool_charges: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    pub identity: UsageIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub includes_copied_context: Option<bool>,
    pub extent: TemporalExtent,
}

str_enum!(
    ModelSource {
        Reported => "reported",
        Fallback => "fallback",
        Inferred => "inferred",
        Unknown => "unknown",
    }
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub source: ModelSource,
}

str_enum!(
    /// Whether a tool is built into the agent or provided by an extension server.
    ToolKind {
        Builtin => "builtin",
        Extension => "extension",
        Unknown => "unknown",
    }
);

/// A tool invocation's identity and name. Never its arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStart {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub kind: ToolKind,
}

/// A tool result's link to its invocation. Never its output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResultRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

/// A cost figure as a provider states it (spec cost model §1). Never spend,
/// never added to estimates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportedCostFacts {
    /// The number exactly as written in the source.
    pub value_as_stated: String,
    /// Exact decimal conversion to nano-units, when the stated number is a plain decimal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_nano: Option<i64>,
    pub unit_as_stated: String,
    pub source_field: String,
    pub semantics: UsageSemantics,
    pub scope: UsageScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestFacts {
    pub repository: String,
    pub number: u64,
}

/// Bounded facts of a quarantined record (§4.4): native type, byte length,
/// key names. Content hash and timestamp live in the envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantineFacts {
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_type: Option<String>,
    pub byte_len: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_level_keys: Vec<String>,
}

/// Facts of records the collector writes about its own work (§4.4–§4.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum CollectorFacts {
    /// A CLI import run began. Supplies `collector_run_id`; asserts no coverage.
    ImportStarted {
        client: String,
    },
    DiscoveryCompleted {
        roots: Vec<String>,
        missing_roots: Vec<String>,
        files_seen: u64,
        files_claimed: u64,
        files_unclaimed: u64,
        new_sources: u64,
        /// Directories under the roots that could not be read (skipped, not fatal).
        unreadable_dirs: u64,
    },
    ReconcileRun {
        source_id: String,
        trigger: IngestTrigger,
        mode: String,
        from_offset: u64,
        to_offset: u64,
        reached_eof: bool,
        continuity: String,
        inserted: u64,
        duplicates: u64,
        quarantined: u64,
        key_conflicts: u64,
        ignored_by_type: BTreeMap<String, u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    SweepCompleted {
        stat_time: UtcMillis,
        sources_checked: u64,
        changed_sources: Vec<String>,
    },
    SourceEnded {
        source_id: String,
        path: String,
        unread_bytes_lost: bool,
    },
    SourceReappeared {
        source_id: String,
        path: String,
    },
    CoverageGap {
        source_id: String,
        start: UtcMillis,
        end: UtcMillis,
        reason: String,
    },
    IncompleteTrailing {
        source_id: String,
        offset: u64,
        byte_len: u64,
        hash: String,
    },
    KeyContentConflict {
        source_id: String,
        record_key: String,
        previous_hash: String,
        new_hash: String,
    },
    LocatorConflict {
        source_id: String,
        kept_path: String,
        other_path: String,
    },
    /// A runtime began collecting these coverage keys (§4.7).
    CoverageStarted {
        client: String,
        keys: Vec<String>,
        heartbeat_interval_ms: i64,
        sample_interval_ms: i64,
    },
    /// Liveness and per-key state (`collecting`, `unavailable`, …).
    CollectorHeartbeat {
        keys: BTreeMap<String, String>,
    },
    /// A clean stop (Quit, Ctrl-C).
    CoverageEnded {
        reason: String,
    },
}

impl CollectorFacts {
    pub fn kind(&self) -> FactKind {
        match self {
            Self::ImportStarted { .. } => FactKind::ImportStarted,
            Self::DiscoveryCompleted { .. } => FactKind::DiscoveryCompleted,
            Self::ReconcileRun { .. } => FactKind::ReconcileRun,
            Self::SweepCompleted { .. } => FactKind::SweepCompleted,
            Self::SourceEnded { .. } => FactKind::SourceEnded,
            Self::SourceReappeared { .. } => FactKind::SourceReappeared,
            Self::CoverageGap { .. } => FactKind::CoverageGap,
            Self::IncompleteTrailing { .. } => FactKind::IncompleteTrailing,
            Self::KeyContentConflict { .. } => FactKind::KeyContentConflict,
            Self::LocatorConflict { .. } => FactKind::LocatorConflict,
            Self::CoverageStarted { .. } => FactKind::CoverageStarted,
            Self::CollectorHeartbeat { .. } => FactKind::CollectorHeartbeat,
            Self::CoverageEnded { .. } => FactKind::CoverageEnded,
        }
    }
}

// ---------------------------------------------------------------------------
// Extraction outcomes
// ---------------------------------------------------------------------------

/// A record an adapter extracted from one line.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedRecord {
    /// The source's own record ID, if it provides one (validated).
    pub native_record_id: Option<String>,
    pub native: NativeIds,
    /// Time stated by the source; `None` if it states none.
    pub occurred_at: Option<UtcMillis>,
    pub time_quality: TimeQuality,
    pub producer_version: Option<String>,
    pub kind: FactKind,
    pub subtype: Option<String>,
    pub facts: Facts,
}

/// A line the adapter could not extract: kept with bounded facts only.
#[derive(Debug, Clone, PartialEq)]
pub struct QuarantinedRecord {
    pub parse_status: ParseStatus,
    pub native_record_id: Option<String>,
    pub native: NativeIds,
    pub occurred_at: Option<UtcMillis>,
    pub producer_version: Option<String>,
    pub facts: QuarantineFacts,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtractOutcome {
    Record(Box<ExtractedRecord>),
    /// A record type the manifest lists as irrelevant. No observation is written.
    Ignore {
        record_type: String,
    },
    Quarantine(Box<QuarantinedRecord>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validators_reject_text() {
        assert_eq!(ident("typed"), Some("typed"));
        assert_eq!(ident("hello world"), None);
        assert_eq!(ident(""), None);
        assert_eq!(native_id("toolu_01AbC-9.x:y"), Some("toolu_01AbC-9.x:y"));
        assert_eq!(native_id("id with space"), None);
        assert_eq!(model_id("<synthetic>"), Some("<synthetic>"));
        assert_eq!(
            context_text("/Users/x/My Project", 100),
            Some("/Users/x/My Project")
        );
        assert_eq!(context_text("line1\nline2", 100), None);
        assert_eq!(context_text(&"a".repeat(101), 100), None);
    }

    #[test]
    fn quarantine_keys_are_bounded() {
        let keys = [
            "type",
            "sessionId",
            "1bad",
            "has space",
            "a/b",
            "ok-key",
            "_x",
        ];
        assert_eq!(
            bounded_quarantine_keys(keys),
            vec!["_x", "ok-key", "sessionId", "type"]
        );
        let many: Vec<String> = (0..100).map(|i| format!("k{i}")).collect();
        assert_eq!(
            bounded_quarantine_keys(many.iter().map(String::as_str)).len(),
            64
        );
    }

    #[test]
    fn non_ident_marker_records_presence_only() {
        let mut f = Facts::default();
        f.set_ident("prompt_source", "free text here");
        assert_eq!(f.markers.get("prompt_source"), None);
        assert_eq!(
            f.markers.get("prompt_source_non_ident"),
            Some(&Marker::Bool(true))
        );
    }

    #[test]
    fn only_hint_triggers_are_live() {
        assert!(IngestTrigger::Hint.is_live());
        assert!(IngestTrigger::DirHint.is_live());
        for t in [
            IngestTrigger::Startup,
            IngestTrigger::Sweep,
            IngestTrigger::Overflow,
            IngestTrigger::Wake,
            IngestTrigger::Discovery,
            IngestTrigger::Backfill,
            IngestTrigger::Sample,
        ] {
            assert!(!t.is_live());
        }
    }
}
