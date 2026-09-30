//! Adapter traits (architecture §1.2).
//!
//! Adapters depend on this crate only. `tracr-app` registers concrete
//! adapters with the ingestion layer; ingestion never imports an adapter crate.
//! Observation adapters read files; probe adapters plan read-only commands
//! and parse their output (§1.2). The normalizer trait lives in `normalize`.

use serde::{Deserialize, Serialize};

use crate::events::FactKind;
use crate::evidence::{ExtractOutcome, Facts, SourceKind, StreamKey};
use crate::ids::MachineId;
use crate::manifest::CapabilityManifest;

/// Whether a stream is an agent's own conversation or a subagent's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamRole {
    Main,
    Subagent,
}

/// The provider-native stream a file belongs to (§4.2). Paths are locators,
/// not identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamRef {
    pub stream_key: StreamKey,
    pub role: StreamRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_agent: Option<String>,
}

/// What an extractor may know about the stream a line comes from.
#[derive(Debug, Clone, Copy)]
pub struct ExtractContext<'a> {
    pub stream: &'a StreamRef,
}

/// An adapter that turns lines of a provider's append-only files into
/// retained facts. Implementations must be pure: no IO, no clock.
pub trait ObservationAdapter: Send + Sync {
    /// Source kind of the streams this adapter owns.
    fn source_kind(&self) -> SourceKind;

    /// Producer name recorded on observations.
    fn producer(&self) -> &'static str;

    fn adapter_version(&self) -> &'static str;

    fn manifest(&self) -> &CapabilityManifest;

    /// Classifies a file by its path components relative to an approved
    /// provider root. Returns `None` for files this adapter does not own.
    fn classify_path(&self, machine: &MachineId, relative: &[&str]) -> Option<StreamRef>;

    /// Extracts retained facts from one complete line (terminator removed).
    /// Facts must be built from an allowlist; content is never copied (§11).
    fn extract(&self, line: &[u8], ctx: &ExtractContext<'_>) -> ExtractOutcome;
}

/// Exit status and standard output of one probe command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutput {
    pub success: bool,
    pub stdout: String,
}

/// An adapter that plans read-only commands against a working directory and
/// parses their output into retained facts (§1.2). Planning and parsing are
/// pure; `tracr-ingest` executes the plan, only under approved roots.
pub trait ProbeAdapter: Send + Sync {
    fn source_kind(&self) -> SourceKind;
    fn adapter_version(&self) -> &'static str;
    fn fact_kind(&self) -> FactKind;
    /// The program to run (resolved on `PATH`).
    fn program(&self) -> &'static str;
    /// Argument lists, one per command, in order.
    fn probes(&self) -> &'static [&'static [&'static str]];
    /// Environment the commands run with (e.g. disabling optional locks).
    fn env(&self) -> &'static [(&'static str, &'static str)];
    fn parse(&self, cwd: &str, machine: &str, outputs: &[ProbeOutput]) -> Facts;
}
