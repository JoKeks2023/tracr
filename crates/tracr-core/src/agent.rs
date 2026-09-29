//! Agent timelines per session (architecture §5, §7.1 "Agent timelines").
//!
//! Input: the canonical (non-copied) normalized events of one session, from
//! all of its streams. Output: actors, turns, tool calls, execution segments
//! and engagement instants. Pure and deterministic: the same events give the
//! same rows and IDs.
//!
//! Rules (`agent-segment@2`, parameter `quiet_gap_max`):
//!
//! * Each actor's events are processed in time order. A `turn_started`
//!   opens a turn and an execution segment at its instant (`observed`).
//! * Every later event of that actor extends the segment; the segment and
//!   the turn end at the actor's last event before the next turn start
//!   (`bounded`: the agent may have run a little longer than its last record).
//! * A quiet gap longer than `quiet_gap_max` with no record of the actor
//!   ends the segment at its last record and opens a new one at the next
//!   record (both `bounded`); the turn continues. In real transcripts such
//!   gaps are the agent waiting for the human (a question or a permission
//!   prompt answered hours later), which is not execution.
//! * `turn_interrupted` ends both at the interruption instant (`observed`).
//! * Activity with no preceding turn start (a stream that begins mid-run)
//!   opens a segment at its first event with a `bounded` start and no turn.
//! * Zero-length segments are dropped: an instant has no duration.
//! * Segments of different actors and sessions overlap freely and are never
//!   merged. Nothing here produces human time. Engagement instants come only
//!   from `user_interaction.prompt_submitted` events.

use std::collections::{BTreeMap, BTreeSet};

use crate::events::PROMPT_SUBMITTED;
use crate::evidence::{ContextFacts, ToolKind};
use crate::ids::derived_id;
use crate::normalize::{ActorRef, EventKind};
use crate::time::UtcMillis;

pub const SEGMENT_RULE: &str = "agent-segment@2";

/// Rule parameters (AD-13: versioned configuration; provisional value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentParams {
    pub quiet_gap_max_ms: i64,
}

impl Default for SegmentParams {
    /// 10 minutes: longer than a single foreground tool call may run in the
    /// producers seen so far, shorter than the human waits seen in real data.
    fn default() -> Self {
        Self {
            quiet_gap_max_ms: 10 * 60_000,
        }
    }
}

impl SegmentParams {
    /// `rule@version#params`, recorded on every segment.
    pub fn rule_ref(&self) -> String {
        format!("{SEGMENT_RULE}#quiet_gap_max_ms={}", self.quiet_gap_max_ms)
    }
}

pub const BASIS_OBSERVED: &str = "observed";
pub const BASIS_BOUNDED: &str = "bounded";

/// A canonical normalized event with a time, ready for timeline derivation.
#[derive(Debug, Clone)]
pub struct TimedEvent {
    pub id: String,
    /// Tie-break for equal times: file order (e.g. seq × 1000 + ordinal).
    pub order: i64,
    pub kind: EventKind,
    pub subtype: Option<String>,
    pub actor: ActorRef,
    pub at: UtcMillis,
    pub native_id: Option<String>,
    pub tool: Option<(Option<String>, ToolKind)>,
    pub context: Option<ContextFacts>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    pub id: String,
    pub actor: ActorRef,
    pub native_turn_id: String,
    pub ordinal: u32,
    pub origin: Option<String>,
    pub started_at: UtcMillis,
    pub ended_at: UtcMillis,
    /// `interrupted`, `failed`, or `None` when no end was observed.
    pub end_kind: Option<&'static str>,
    pub end_basis: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub actor: ActorRef,
    pub turn_id: Option<String>,
    pub native_id: Option<String>,
    pub name: Option<String>,
    pub kind: ToolKind,
    pub started_at: UtcMillis,
    pub finished_at: Option<UtcMillis>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub id: String,
    pub actor: ActorRef,
    pub turn_id: Option<String>,
    pub start: UtcMillis,
    pub end: UtcMillis,
    pub start_basis: &'static str,
    pub end_basis: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngagementInstant {
    pub id: String,
    pub at: UtcMillis,
    pub event_id: String,
    pub context: Option<ContextFacts>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionTimeline {
    pub first_evidence: Option<UtcMillis>,
    pub last_evidence: Option<UtcMillis>,
    /// Agent actors that produced at least one event (never the human).
    pub actors: Vec<ActorRef>,
    pub turns: Vec<Turn>,
    pub tool_calls: Vec<ToolCall>,
    pub segments: Vec<Segment>,
    pub instants: Vec<EngagementInstant>,
    /// Event ID → turn ID for events inside a turn.
    pub event_turns: BTreeMap<String, String>,
}

pub fn actor_id(session_key: &str, actor: &ActorRef) -> Option<String> {
    actor.key().map(|k| derived_id("act_", &[session_key, k]))
}

struct OpenRun {
    turn: Option<usize>,
    seg_start: UtcMillis,
    seg_start_basis: &'static str,
    /// Latest instant of the open segment (its start until a record extends it).
    last: UtcMillis,
}

pub fn derive_session(
    session_key: &str,
    events: &[TimedEvent],
    params: &SegmentParams,
) -> SessionTimeline {
    let rule_ref = params.rule_ref();
    let mut sorted: Vec<&TimedEvent> = events.iter().collect();
    sorted.sort_by(|a, b| (a.at, a.order, &a.id).cmp(&(b.at, b.order, &b.id)));

    let mut out = SessionTimeline {
        first_evidence: sorted.first().map(|e| e.at),
        last_evidence: sorted.last().map(|e| e.at),
        actors: Vec::new(),
        turns: Vec::new(),
        tool_calls: Vec::new(),
        segments: Vec::new(),
        instants: Vec::new(),
        event_turns: BTreeMap::new(),
    };

    // Engagement instants: human prompts only.
    for e in &sorted {
        if e.kind == EventKind::UserInteraction && e.subtype.as_deref() == Some(PROMPT_SUBMITTED) {
            let key = e.native_id.as_deref().unwrap_or(&e.id);
            out.instants.push(EngagementInstant {
                id: derived_id("eng_", &[session_key, key]),
                at: e.at,
                event_id: e.id.clone(),
                context: e.context.clone(),
            });
        }
    }

    // Per actor: turns and execution segments.
    let mut by_actor: BTreeMap<ActorRef, Vec<&TimedEvent>> = BTreeMap::new();
    for e in &sorted {
        if e.actor != ActorRef::Human {
            by_actor.entry(e.actor.clone()).or_default().push(e);
        }
    }
    struct TurnDraft {
        actor: ActorRef,
        native: String,
        origin: Option<String>,
        start: UtcMillis,
        end: UtcMillis,
        end_kind: Option<&'static str>,
        end_basis: &'static str,
        events: Vec<String>,
    }
    let mut drafts: Vec<TurnDraft> = Vec::new();
    // Segments refer to turns by draft index until turn IDs exist.
    let mut seg_drafts = Vec::new();

    type SegDraft = (
        ActorRef,
        Option<usize>,
        UtcMillis,
        UtcMillis,
        &'static str,
        &'static str,
    );
    for (actor, evs) in &by_actor {
        out.actors.push(actor.clone());
        let mut run: Option<OpenRun> = None;
        let emit = |r: &OpenRun, end_basis: &'static str, segs: &mut Vec<SegDraft>| {
            if r.last > r.seg_start {
                segs.push((
                    actor.clone(),
                    r.turn,
                    r.seg_start,
                    r.last,
                    r.seg_start_basis,
                    end_basis,
                ));
            }
        };
        for e in evs {
            if e.kind == EventKind::TurnStarted {
                if let Some(r) = run.take() {
                    emit(&r, BASIS_BOUNDED, &mut seg_drafts);
                }
                drafts.push(TurnDraft {
                    actor: actor.clone(),
                    native: e.native_id.clone().unwrap_or_else(|| e.id.clone()),
                    origin: e.subtype.clone(),
                    start: e.at,
                    end: e.at,
                    end_kind: None,
                    end_basis: BASIS_BOUNDED,
                    events: vec![e.id.clone()],
                });
                run = Some(OpenRun {
                    turn: Some(drafts.len() - 1),
                    seg_start: e.at,
                    seg_start_basis: BASIS_OBSERVED,
                    last: e.at,
                });
                continue;
            }
            let r = match run.as_mut() {
                None => run.insert(OpenRun {
                    turn: None,
                    seg_start: e.at,
                    seg_start_basis: BASIS_BOUNDED,
                    last: e.at,
                }),
                Some(r) if e.at.0 - r.last.0 > params.quiet_gap_max_ms => {
                    // A long quiet gap: the agent was not observably executing.
                    emit(r, BASIS_BOUNDED, &mut seg_drafts);
                    r.seg_start = e.at;
                    r.seg_start_basis = BASIS_BOUNDED;
                    r.last = e.at;
                    r
                }
                Some(r) => {
                    r.last = e.at;
                    r
                }
            };
            if let Some(t) = r.turn {
                drafts[t].events.push(e.id.clone());
                drafts[t].end = e.at;
                if e.kind == EventKind::TurnFailed {
                    drafts[t].end_kind = Some("failed");
                }
            }
            if e.kind == EventKind::TurnInterrupted {
                if let Some(t) = r.turn {
                    drafts[t].end_kind = Some("interrupted");
                    drafts[t].end_basis = BASIS_OBSERVED;
                }
                let r = run.take().expect("open run");
                emit(&r, BASIS_OBSERVED, &mut seg_drafts);
            }
        }
        if let Some(r) = run.take() {
            emit(&r, BASIS_BOUNDED, &mut seg_drafts);
        }
    }

    // Turn IDs and ordinals (session-wide, by start time).
    let mut order: Vec<usize> = (0..drafts.len()).collect();
    order.sort_by(|&a, &b| {
        (drafts[a].start, &drafts[a].native).cmp(&(drafts[b].start, &drafts[b].native))
    });
    let mut turn_ids: Vec<String> = vec![String::new(); drafts.len()];
    for (ordinal, &i) in order.iter().enumerate() {
        let d = &drafts[i];
        let id = derived_id("trn_", &[session_key, &d.native]);
        turn_ids[i] = id.clone();
        for ev in &d.events {
            out.event_turns.insert(ev.clone(), id.clone());
        }
        out.turns.push(Turn {
            id,
            actor: d.actor.clone(),
            native_turn_id: d.native.clone(),
            ordinal: ordinal as u32,
            origin: d.origin.clone(),
            started_at: d.start,
            ended_at: d.end,
            end_kind: d.end_kind,
            end_basis: d.end_basis,
        });
    }

    for (actor, turn, start, end, sb, eb) in seg_drafts {
        let actor_key = actor.key().unwrap_or("unknown").to_owned();
        out.segments.push(Segment {
            id: derived_id(
                "seg_",
                &[
                    session_key,
                    &actor_key,
                    &start.0.to_string(),
                    &end.0.to_string(),
                    &rule_ref,
                ],
            ),
            actor,
            turn_id: turn.map(|t| turn_ids[t].clone()),
            start,
            end,
            start_basis: sb,
            end_basis: eb,
        });
    }
    out.segments
        .sort_by(|a, b| (a.start, &a.id).cmp(&(b.start, &b.id)));

    // Tool calls: a start and, where observed, its finish (by native call ID).
    let mut finished: BTreeMap<&str, UtcMillis> = BTreeMap::new();
    for e in &sorted {
        if e.kind == EventKind::ToolFinished
            && let Some(id) = e.native_id.as_deref()
        {
            finished.entry(id).or_insert(e.at);
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for e in &sorted {
        if e.kind != EventKind::ToolStarted {
            continue;
        }
        let key = e.native_id.clone().unwrap_or_else(|| e.id.clone());
        if !seen.insert(key.clone()) {
            continue;
        }
        let (name, kind) = e.tool.clone().unwrap_or((None, ToolKind::Unknown));
        out.tool_calls.push(ToolCall {
            id: derived_id("tol_", &[session_key, &key]),
            actor: e.actor.clone(),
            turn_id: out.event_turns.get(&e.id).cloned(),
            native_id: e.native_id.clone(),
            name,
            kind,
            started_at: e.at,
            finished_at: e
                .native_id
                .as_deref()
                .and_then(|id| finished.get(id).copied())
                .filter(|f| *f >= e.at),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str, at: i64, kind: EventKind, actor: ActorRef) -> TimedEvent {
        TimedEvent {
            id: id.into(),
            order: at,
            kind,
            subtype: None,
            actor,
            at: UtcMillis(at),
            native_id: Some(id.into()),
            tool: None,
            context: None,
        }
    }

    fn prompt(id: &str, at: i64) -> [TimedEvent; 2] {
        let mut p = ev(id, at, EventKind::UserInteraction, ActorRef::Human);
        p.subtype = Some(PROMPT_SUBMITTED.into());
        let mut t = ev(id, at, EventKind::TurnStarted, ActorRef::Unknown);
        t.subtype = Some("human".into());
        t.id = format!("{id}-t");
        [p, t]
    }

    const MIN: i64 = 60_000;

    #[test]
    fn a_turn_runs_from_its_prompt_to_its_last_agent_record() {
        let mut e: Vec<TimedEvent> = prompt("p1", 0).into();
        e.push(ev("m1", MIN, EventKind::ModelObserved, ActorRef::Unknown));
        e.push(ev(
            "m2",
            5 * MIN,
            EventKind::ModelObserved,
            ActorRef::Unknown,
        ));
        e.extend(prompt("p2", 20 * MIN));
        e.push(ev(
            "m3",
            22 * MIN,
            EventKind::ModelObserved,
            ActorRef::Unknown,
        ));
        let t = derive_session("s", &e, &SegmentParams::default());
        assert_eq!(t.instants.len(), 2);
        assert_eq!(t.turns.len(), 2);
        assert_eq!(t.segments.len(), 2);
        assert_eq!((t.segments[0].start.0, t.segments[0].end.0), (0, 5 * MIN));
        assert_eq!(t.segments[0].start_basis, BASIS_OBSERVED);
        assert_eq!(t.segments[0].end_basis, BASIS_BOUNDED);
        assert_eq!(t.turns[1].ended_at.0, 22 * MIN);
        assert_eq!(t.actors, vec![ActorRef::Unknown]);
    }

    #[test]
    fn crashed_session_ends_at_last_evidence_bounded() {
        let mut e: Vec<TimedEvent> = prompt("p1", 0).into();
        e.push(ev(
            "m1",
            3 * MIN,
            EventKind::ModelObserved,
            ActorRef::Unknown,
        ));
        let t = derive_session("s", &e, &SegmentParams::default());
        assert_eq!(t.last_evidence, Some(UtcMillis(3 * MIN)));
        assert_eq!(t.turns[0].end_kind, None);
        assert_eq!(t.turns[0].end_basis, BASIS_BOUNDED);
        assert_eq!(t.segments[0].end.0, 3 * MIN);
    }

    #[test]
    fn a_long_quiet_gap_splits_the_segment_but_not_the_turn() {
        // The agent asks the human a question at 1 min; the answer arrives
        // 9 hours later and the agent finishes at 9 h 2 min.
        let mut e: Vec<TimedEvent> = prompt("p1", 0).into();
        e.push(ev("m1", MIN, EventKind::ModelObserved, ActorRef::Unknown));
        e.push(ev(
            "u1",
            540 * MIN,
            EventKind::ToolFinished,
            ActorRef::Unknown,
        ));
        e.push(ev(
            "m2",
            542 * MIN,
            EventKind::ModelObserved,
            ActorRef::Unknown,
        ));
        let t = derive_session("s", &e, &SegmentParams::default());
        assert_eq!(t.turns.len(), 1);
        assert_eq!(t.turns[0].ended_at.0, 542 * MIN, "the turn spans the wait");
        let spans: Vec<(i64, i64)> = t.segments.iter().map(|g| (g.start.0, g.end.0)).collect();
        assert_eq!(spans, vec![(0, MIN), (540 * MIN, 542 * MIN)]);
        assert_eq!(t.segments[1].start_basis, BASIS_BOUNDED);
        assert!(
            t.segments
                .iter()
                .all(|g| g.turn_id.as_deref() == Some(t.turns[0].id.as_str()))
        );
        // A gap at the threshold is not split.
        let mut e: Vec<TimedEvent> = prompt("p1", 0).into();
        e.push(ev(
            "m1",
            10 * MIN,
            EventKind::ModelObserved,
            ActorRef::Unknown,
        ));
        let t = derive_session("s", &e, &SegmentParams::default());
        assert_eq!(t.segments.len(), 1);
    }

    #[test]
    fn interruption_ends_turn_and_segment_observed() {
        let mut e: Vec<TimedEvent> = prompt("p1", 0).into();
        e.push(ev("m1", MIN, EventKind::ModelObserved, ActorRef::Unknown));
        e.push(ev(
            "i1",
            2 * MIN,
            EventKind::TurnInterrupted,
            ActorRef::Unknown,
        ));
        let t = derive_session("s", &e, &SegmentParams::default());
        assert_eq!(t.turns[0].end_kind, Some("interrupted"));
        assert_eq!(t.segments[0].end.0, 2 * MIN);
        assert_eq!(t.segments[0].end_basis, BASIS_OBSERVED);
    }

    #[test]
    fn a_prompt_without_agent_activity_yields_no_segment() {
        let e: Vec<TimedEvent> = prompt("p1", 0).into();
        let t = derive_session("s", &e, &SegmentParams::default());
        assert_eq!(t.turns.len(), 1);
        assert!(t.segments.is_empty());
        assert_eq!(t.instants.len(), 1);
    }

    #[test]
    fn subagent_overlaps_main_and_neither_is_merged() {
        let sub = ActorRef::Known("a1".into());
        let mut e: Vec<TimedEvent> = prompt("p1", 0).into();
        e.push(ev(
            "m1",
            10 * MIN,
            EventKind::ModelObserved,
            ActorRef::Unknown,
        ));
        let mut st = ev("s0", 2 * MIN, EventKind::TurnStarted, sub.clone());
        st.subtype = Some("agent".into());
        e.push(st);
        e.push(ev("s1", 6 * MIN, EventKind::ModelObserved, sub.clone()));
        let t = derive_session("s", &e, &SegmentParams::default());
        assert_eq!(t.segments.len(), 2);
        assert_eq!(t.actors.len(), 2);
        // The subagent's turn start is not a human instant.
        assert_eq!(t.instants.len(), 1);
    }

    #[test]
    fn activity_without_a_turn_start_has_a_bounded_start() {
        let e = vec![
            ev("m1", MIN, EventKind::ModelObserved, ActorRef::Unknown),
            ev("m2", 4 * MIN, EventKind::ModelObserved, ActorRef::Unknown),
        ];
        let t = derive_session("s", &e, &SegmentParams::default());
        assert!(t.turns.is_empty());
        assert_eq!(t.segments[0].start_basis, BASIS_BOUNDED);
        assert_eq!(t.segments[0].turn_id, None);
    }

    #[test]
    fn tool_calls_pair_by_native_id_and_belong_to_their_turn() {
        let mut e: Vec<TimedEvent> = prompt("p1", 0).into();
        let mut ts = ev("m1", MIN, EventKind::ToolStarted, ActorRef::Unknown);
        ts.native_id = Some("toolu_1".into());
        ts.tool = Some((Some("Bash".into()), ToolKind::Builtin));
        let mut tf = ev("u1", 2 * MIN, EventKind::ToolFinished, ActorRef::Unknown);
        tf.native_id = Some("toolu_1".into());
        e.push(ts);
        e.push(tf);
        let t = derive_session("s", &e, &SegmentParams::default());
        assert_eq!(t.tool_calls.len(), 1);
        let c = &t.tool_calls[0];
        assert_eq!(c.finished_at, Some(UtcMillis(2 * MIN)));
        assert_eq!(c.turn_id.as_deref(), Some(t.turns[0].id.as_str()));
        assert_eq!(c.name.as_deref(), Some("Bash"));
    }

    #[test]
    fn derivation_is_deterministic_regardless_of_input_order() {
        let mut e: Vec<TimedEvent> = prompt("p1", 0).into();
        e.push(ev("m1", MIN, EventKind::ModelObserved, ActorRef::Unknown));
        e.extend(prompt("p2", 5 * MIN));
        e.push(ev(
            "m2",
            6 * MIN,
            EventKind::ModelObserved,
            ActorRef::Unknown,
        ));
        let a = derive_session("s", &e, &SegmentParams::default());
        e.reverse();
        let b = derive_session("s", &e, &SegmentParams::default());
        assert_eq!(a, b);
    }
}
