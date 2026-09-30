//! Kinds recorded on retained facts (spec event model §2, §5).
//!
//! An extractor classifies each record into one [`FactKind`] and an optional
//! subtype, and keeps the structural markers the classification read, so a
//! later normalizer can re-decide from retained facts (architecture §13.2).
//! Normalized events are derived from these facts in a later step (S1.3).

use serde::{Deserialize, Serialize};

/// Subtype of [`FactKind::UserInteraction`] for a human-originated prompt.
pub const PROMPT_SUBMITTED: &str = "prompt_submitted";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    // Records from observation adapters.
    /// Human-originated interaction (subtype e.g. `prompt_submitted`).
    UserInteraction,
    /// The human interrupted a running turn.
    TurnInterrupted,
    /// A tool result was delivered to the agent.
    ToolFinished,
    /// An agent output record (may carry usage, model and tool starts).
    AgentMessage,
    /// A user-role record that did not come from the human (tool plumbing,
    /// meta records, subagent prompts, automated notifications).
    AgentInput,
    /// A provider system record (subtype = the provider's system subtype).
    SystemEvent,
    /// A cost figure as the provider reports it (never spend).
    ReportedCostSnapshot,
    PrObserved,
    WorktreeObserved,
    SessionRelocated,
    SessionContinued,
    QueueOperation,
    /// A record that could not be extracted (bounded facts only).
    Quarantined,

    // Records written by the collector itself.
    ImportStarted,
    DiscoveryCompleted,
    ReconcileRun,
    SweepCompleted,
    SourceEnded,
    SourceReappeared,
    CoverageGap,
    IncompleteTrailing,
    KeyContentConflict,
    LocatorConflict,
    /// A runtime began collecting (its ID is the run's `collector_run_id`).
    CoverageStarted,
    /// Every 60 s while a runtime collects: the state of each coverage key.
    CollectorHeartbeat,
    /// A runtime stopped cleanly.
    CoverageEnded,

    // Records from the OS sensor (§4.8).
    DeviceActive,
    DeviceIdle,
    DeviceLocked,
    DeviceUnlocked,
    DeviceSuspended,
    DeviceResumed,
}

impl FactKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserInteraction => "user_interaction",
            Self::TurnInterrupted => "turn_interrupted",
            Self::ToolFinished => "tool_finished",
            Self::AgentMessage => "agent_message",
            Self::AgentInput => "agent_input",
            Self::SystemEvent => "system_event",
            Self::ReportedCostSnapshot => "reported_cost_snapshot",
            Self::PrObserved => "pr_observed",
            Self::WorktreeObserved => "worktree_observed",
            Self::SessionRelocated => "session_relocated",
            Self::SessionContinued => "session_continued",
            Self::QueueOperation => "queue_operation",
            Self::Quarantined => "quarantined",
            Self::ImportStarted => "import_started",
            Self::DiscoveryCompleted => "discovery_completed",
            Self::ReconcileRun => "reconcile_run",
            Self::SweepCompleted => "sweep_completed",
            Self::SourceEnded => "source_ended",
            Self::SourceReappeared => "source_reappeared",
            Self::CoverageGap => "coverage_gap",
            Self::IncompleteTrailing => "incomplete_trailing",
            Self::KeyContentConflict => "key_content_conflict",
            Self::LocatorConflict => "locator_conflict",
            Self::CoverageStarted => "coverage_started",
            Self::CollectorHeartbeat => "collector_heartbeat",
            Self::CoverageEnded => "coverage_ended",
            Self::DeviceActive => "device_active",
            Self::DeviceIdle => "device_idle",
            Self::DeviceLocked => "device_locked",
            Self::DeviceUnlocked => "device_unlocked",
            Self::DeviceSuspended => "device_suspended",
            Self::DeviceResumed => "device_resumed",
        }
    }
}
