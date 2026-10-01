//! Codex rollout adapter (architecture §4.2, §8.1, S1.6).
//!
//! Reads Codex's local rollout files (`~/.codex/sessions/YYYY/MM/DD/` and
//! `~/.codex/archived_sessions/`) into retained facts. Never `session_index`
//! or `transcription-history`. The extractor is an allowlist: it keeps named
//! non-content fields and structural markers, never prompt, message,
//! reasoning or tool text (§11).

use tracr_core::adapter::{ExtractContext, ObservationAdapter, StreamRef};
use tracr_core::evidence::{ExtractOutcome, SourceKind};
use tracr_core::ids::MachineId;
use tracr_core::manifest::CapabilityManifest;

pub mod discovery;
pub mod extract;
pub mod manifest;
pub mod normalize;

/// Source kind of Codex rollout streams (§3.2 `source.kind`).
pub const SOURCE_KIND: &str = "codex_rollout";
/// Producer recorded on observations.
pub const PRODUCER: &str = "codex";
pub const ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct CodexAdapter {
    manifest: CapabilityManifest,
}

impl CodexAdapter {
    pub fn new() -> Self {
        Self {
            manifest: manifest::manifest(),
        }
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservationAdapter for CodexAdapter {
    fn source_kind(&self) -> SourceKind {
        SourceKind::new(SOURCE_KIND)
    }

    fn producer(&self) -> &'static str {
        PRODUCER
    }

    fn adapter_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn manifest(&self) -> &CapabilityManifest {
        &self.manifest
    }

    fn classify_path(&self, machine: &MachineId, relative: &[&str]) -> Option<StreamRef> {
        discovery::classify(machine, relative)
    }

    fn extract(&self, line: &[u8], ctx: &ExtractContext<'_>) -> ExtractOutcome {
        extract::extract(line, ctx, &self.manifest)
    }
}
