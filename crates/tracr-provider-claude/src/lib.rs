//! Claude Code transcript adapter (architecture §5.2, §13.2).
//!
//! Reads Claude Code's local JSONL transcripts into retained facts. The
//! extractor is an allowlist: it copies named non-content fields and
//! structural markers, never prompt, message or tool text (§11).

use tracr_core::adapter::{ExtractContext, ObservationAdapter, StreamRef};
use tracr_core::evidence::{ExtractOutcome, SourceKind};
use tracr_core::ids::MachineId;
use tracr_core::manifest::CapabilityManifest;

pub mod discovery;
pub mod extract;
pub mod manifest;

/// Source kind of Claude Code transcript streams (§3.2 `source.kind`).
pub const SOURCE_KIND: &str = "claude_transcript";
/// Producer recorded on observations.
pub const PRODUCER: &str = "claude_code";
pub const ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct ClaudeAdapter {
    manifest: CapabilityManifest,
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self {
            manifest: manifest::manifest(),
        }
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservationAdapter for ClaudeAdapter {
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
