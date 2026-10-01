//! The Codex capability manifest (AD-19, AD-21).
//!
//! Validated range: Codex CLI/Desktop 0.145–0.158 (numeric core; alpha
//! builds such as `0.149.0-alpha.4.3` match by their `0.149.0` core). The
//! structure was profiled from all local rollouts (37 threads, 2026-07 to
//! 2026-10) and is reproduced, without content, by the fixtures in
//! `tracr-testkit::codex`.

use tracr_core::manifest::{CapabilityManifest, GatedCapability, VersionRange};

pub const EXTRACTOR_VERSION: &str = "codex-extract/1";

pub const KNOWN_RECORD_TYPES: &[&str] = &[
    "session_meta",
    "turn_context",
    "event_msg",
    "response_item",
    "token_usage_record",
];

/// Record types that carry content or nothing tracr uses (§4.4, §11):
/// `compacted` holds replacement history (content); `world_state` and
/// `inter_agent_communication_metadata` are UI or routing state.
pub const IGNORED_RECORD_TYPES: &[&str] = &[
    "compacted",
    "world_state",
    "inter_agent_communication_metadata",
];

/// `session_meta.source` values that may yield human engagement (AD-19).
/// Only `vscode` (the Codex Desktop and IDE surface) is seen locally; `cli`
/// and `exec` are not in the corpus, so they never do yet.
pub const INTERACTIVE_SOURCES: &[&str] = &["vscode"];

pub const VALIDATED_MIN: &str = "0.145.0";
pub const VALIDATED_MAX: &str = "0.158.0";

/// The numeric core of a Codex version: `0.149.0-alpha.4.3` → `0.149.0`.
pub fn version_core(v: &str) -> &str {
    v.split_once('-').map_or(v, |(core, _)| core)
}

pub fn manifest() -> CapabilityManifest {
    CapabilityManifest {
        adapter: "codex_rollout",
        extractor_version: EXTRACTOR_VERSION,
        validated_ranges: vec![VersionRange::new(VALIDATED_MIN, VALIDATED_MAX)],
        interactive_values: INTERACTIVE_SOURCES.to_vec(),
        ignored_record_types: IGNORED_RECORD_TYPES.to_vec(),
        known_record_types: KNOWN_RECORD_TYPES.to_vec(),
        fixture_gated: vec![GatedCapability {
            name: "subagent_linkage",
            fallback: "child thread stays its own session, actor unknown (OQ-I-06)",
        }],
        // AD-21: rollouts do not document a main-agent identity.
        main_actor_documented: false,
    }
}
