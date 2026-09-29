//! The Claude Code capability manifest (AD-19, AD-21).
//!
//! Validated range: producer versions 2.1.231–2.1.281. The committed
//! fixtures under `fixtures/<version>/` (2.1.231, 2.1.236, 2.1.267, 2.1.281)
//! were reduced from real local transcripts and pass `tests/extract.rs`.

use tracr_core::manifest::{CapabilityManifest, GatedCapability, VersionRange};

pub const EXTRACTOR_VERSION: &str = "claude-extract/2";

/// Record types the extractor reads.
pub const KNOWN_RECORD_TYPES: &[&str] = &[
    "user",
    "assistant",
    "system",
    "cost-state",
    "pr-link",
    "continued-in",
    "relocated",
    "worktree-state",
    "queue-operation",
];

/// Record types that carry content or nothing tracr uses. They produce no
/// observation (§4.4, §11): `ai-title`, `last-prompt`, `summary` and
/// `attachment` are content; the others are UI or account state.
pub const IGNORED_RECORD_TYPES: &[&str] = &[
    "attachment",
    "last-prompt",
    "ai-title",
    "summary",
    "mode",
    "permission-mode",
    "atis-latch",
    "agent-name",
    "agent-setting",
    "file-history-snapshot",
    "file-history-delta",
    "bridge-session",
];

/// `entrypoint` values that may yield human engagement (AD-19): only values
/// the fixture corpus covers. SDK and any unlisted entrypoint (including
/// `claude-vscode`, seen locally but not in any fixture yet) never do.
pub const INTERACTIVE_ENTRYPOINTS: &[&str] = &["cli"];

/// `promptSource` values of human-typed prompts. `queued` is a prompt the
/// human typed while the agent was busy; `system` is automated.
pub const HUMAN_PROMPT_SOURCES: &[&str] = &["typed", "queued", "suggestion_accepted"];

/// Leading envelope tags of string content the extractor recognizes. Only the
/// tag name is kept as a marker, never the text.
pub const CONTENT_TAGS: &[&str] = &[
    "command-name",
    "command-message",
    "command-args",
    "local-command-caveat",
    "local-command-stdout",
    "local-command-stderr",
    "bash-input",
    "bash-stdout",
    "bash-stderr",
    "task-notification",
    "system-reminder",
];

/// Tags of records the human typed. A slash command or skill invocation is
/// written as `<command-name>…` or, in current producers, led by
/// `<command-message>…` (its expanded prompt follows as an `isMeta` record);
/// `<bash-input>` is a shell command typed in bash mode. Every other
/// recognized tag is tool or system output.
pub const HUMAN_CONTENT_TAGS: &[&str] = &["command-name", "command-message", "bash-input"];

pub fn manifest() -> CapabilityManifest {
    CapabilityManifest {
        adapter: "claude_code_transcript",
        extractor_version: EXTRACTOR_VERSION,
        validated_ranges: vec![VersionRange::new("2.1.231", "2.1.281")],
        interactive_values: INTERACTIVE_ENTRYPOINTS.to_vec(),
        ignored_record_types: IGNORED_RECORD_TYPES.to_vec(),
        known_record_types: KNOWN_RECORD_TYPES.to_vec(),
        fixture_gated: vec![GatedCapability {
            name: "compaction_mapping",
            fallback: "no context_compacted events (OQ-I-07)",
        }],
        // AD-21: transcript structure alone does not document the main agent.
        main_actor_documented: false,
    }
}
