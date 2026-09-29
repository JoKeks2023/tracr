//! The human-prompt rule, decided from retained markers only (§13.2).
//!
//! The extractor records the structural markers first and then classifies
//! from those markers, so the normalizer can re-decide from retained facts
//! with exactly the same function (walkthrough E-1): no content is needed.

use tracr_core::evidence::{Facts, Marker};
use tracr_core::manifest::CapabilityManifest;

use crate::manifest::{HUMAN_CONTENT_TAGS, HUMAN_PROMPT_SOURCES};

/// The markers the human-prompt rule reads, as retained on a `user` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSignals {
    pub subagent_stream: bool,
    pub sidechain: bool,
    pub agent_present: bool,
    /// `None` when absent or not identifier-like (never interactive).
    pub entrypoint: Option<String>,
    pub meta: bool,
    pub compact_summary: bool,
    pub transcript_only: bool,
    pub scheduled: bool,
    /// Any of `origin.kind`, `turnOrigin`, `promptSource` marks the record automated.
    pub automated_origin: bool,
    pub tag: Option<String>,
    pub content_kind: Option<String>,
}

fn flag(f: &Facts, key: &str) -> bool {
    matches!(f.markers.get(key), Some(Marker::Bool(true)))
}

fn text<'a>(f: &'a Facts, key: &str) -> Option<&'a str> {
    match f.markers.get(key) {
        Some(Marker::Ident(v)) => Some(v),
        _ => None,
    }
}

/// An origin marker is automated if its value is not the human value, or if
/// a value was present but not identifier-like (`<key>_non_ident`).
fn automated(f: &Facts, key: &str, human: &[&str]) -> bool {
    text(f, key).is_some_and(|v| !human.contains(&v)) || flag(f, &format!("{key}_non_ident"))
}

pub fn prompt_signals(f: &Facts) -> PromptSignals {
    PromptSignals {
        subagent_stream: text(f, "stream_role") == Some("subagent"),
        sidechain: flag(f, "is_sidechain"),
        agent_present: flag(f, "agent_id_present"),
        entrypoint: text(f, "entrypoint").map(str::to_owned),
        meta: flag(f, "is_meta"),
        compact_summary: flag(f, "is_compact_summary"),
        transcript_only: flag(f, "is_visible_in_transcript_only"),
        scheduled: flag(f, "scheduled_present"),
        automated_origin: automated(f, "origin_kind", &["human"])
            || automated(f, "turn_origin", &["human"])
            || automated(f, "prompt_source", HUMAN_PROMPT_SOURCES),
        tag: text(f, "content_tag").map(str::to_owned),
        content_kind: text(f, "content_kind").map(str::to_owned),
    }
}

/// Why a `user` record is not a human prompt, or `None` if it is one.
/// With `ignore_entrypoint`, the entrypoint check is skipped: the answer is
/// then "would this be a human prompt in an interactive session?".
pub fn non_human_reason(
    s: &PromptSignals,
    manifest: &CapabilityManifest,
    ignore_entrypoint: bool,
) -> Option<&'static str> {
    if s.subagent_stream {
        return Some("subagent_stream");
    }
    if s.sidechain || s.agent_present {
        return Some("sidechain");
    }
    if !ignore_entrypoint && !manifest.is_interactive(s.entrypoint.as_deref()) {
        return Some("non_interactive_entrypoint");
    }
    if s.meta {
        return Some("meta");
    }
    if s.compact_summary {
        return Some("compact_summary");
    }
    if s.transcript_only {
        return Some("transcript_only");
    }
    if s.scheduled {
        return Some("scheduled");
    }
    if s.automated_origin {
        return Some("automated_origin");
    }
    if s.tag
        .as_deref()
        .is_some_and(|t| !HUMAN_CONTENT_TAGS.contains(&t))
    {
        return Some("tool_or_system_output");
    }
    if matches!(s.content_kind.as_deref(), None | Some("absent")) {
        return Some("no_content");
    }
    None
}

/// Whether a non-human `user` record starts an agent run (a turn with
/// origin `automated` on the main chain, or `agent` in a subagent stream).
/// Tool and system output, meta records and summaries never do.
pub fn starts_agent_run(s: &PromptSignals, manifest: &CapabilityManifest) -> Option<&'static str> {
    if s.subagent_stream {
        let plain_input = !s.meta && s.tag.is_none() && s.content_kind.as_deref() != Some("absent");
        return plain_input.then_some("agent");
    }
    match non_human_reason(s, manifest, false) {
        Some("scheduled" | "automated_origin") => Some("automated"),
        // A prompt that is human-shaped except for its entrypoint (SDK, IDE
        // extensions not fixture-validated): an agent run, never an instant.
        Some("non_interactive_entrypoint") if non_human_reason(s, manifest, true).is_none() => {
            Some("automated")
        }
        _ => None,
    }
}
