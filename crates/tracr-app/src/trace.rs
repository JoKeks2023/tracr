//! One session laid out in time (T-0015 step 2): who ran when, which tools
//! were called, how large each request's context was. Names, times and token
//! counts only; no message or tool content is read.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use rusqlite::Connection;
use serde::Serialize;
use tracr_store::derived;

#[derive(Debug, Clone, Serialize)]
pub struct TraceActor {
    pub actor_id: String,
    /// `main`, `known` (a subagent) or `unknown`.
    pub actor_kind: String,
    pub native_agent_id: Option<String>,
    /// Execution segments, unclipped: agent time, never human time.
    pub segments: Vec<(i64, i64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceTool {
    pub actor_id: Option<String>,
    pub name: String,
    /// `builtin` or `extension` (an MCP tool), as the provider records it.
    pub kind: String,
    pub at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolSummary {
    pub name: String,
    pub kind: String,
    pub calls: i64,
}

/// What one request carried. `context` is the whole input the model saw:
/// the provider's input total, else fresh + cache read + cache write when all
/// three are stated; `None` otherwise (unknown, never zero).
#[derive(Debug, Clone, Serialize)]
pub struct TracePoint {
    pub at: i64,
    pub actor_id: Option<String>,
    pub model: Option<String>,
    pub context: Option<i64>,
    pub cache_read: Option<i64>,
    pub output: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionTrace {
    pub session_id: String,
    pub timezone: String,
    pub start: i64,
    pub end: i64,
    pub actors: Vec<TraceActor>,
    pub prompts: Vec<i64>,
    pub tools: Vec<TraceTool>,
    pub tool_summary: Vec<ToolSummary>,
    pub points: Vec<TracePoint>,
}

fn context_of(p: &derived::UsagePointRow) -> Option<i64> {
    p.input_total
        .or_else(|| Some(p.input_fresh? + p.cache_read? + p.cache_creation?))
}

pub fn session_trace(
    conn: &Connection,
    zone: &crate::day::ReportingZone,
    session_id: &str,
) -> Result<SessionTrace> {
    let (first, last): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT first_evidence_at, last_evidence_at FROM agent_session WHERE id = ?1",
            [session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| anyhow!("unknown session"))?;
    let segments = derived::segments_in(conn, session_id, i64::MIN, i64::MAX)?;
    let actors = derived::actors_of(conn, session_id)?
        .into_iter()
        .map(|a| TraceActor {
            segments: segments
                .iter()
                .filter(|g| g.actor_id == a.id)
                .map(|g| (g.start_at, g.end_at))
                .collect(),
            actor_id: a.id,
            actor_kind: a.actor_kind,
            native_agent_id: a.native_agent_id,
        })
        .filter(|a| !a.segments.is_empty() || a.actor_kind == "main")
        .collect::<Vec<_>>();
    let tools: Vec<TraceTool> = derived::tool_call_times_of(conn, session_id)?
        .into_iter()
        .filter_map(|t| {
            Some(TraceTool {
                actor_id: t.actor_id,
                name: t.tool_name.unwrap_or_else(|| "unknown".to_owned()),
                kind: t.tool_kind.unwrap_or_else(|| "unknown".to_owned()),
                at: t.started_at?,
                finished_at: t.finished_at,
            })
        })
        .collect();
    let mut counts: BTreeMap<(String, String), i64> = BTreeMap::new();
    for t in &tools {
        *counts.entry((t.name.clone(), t.kind.clone())).or_default() += 1;
    }
    let mut tool_summary: Vec<ToolSummary> = counts
        .into_iter()
        .map(|((name, kind), calls)| ToolSummary { name, kind, calls })
        .collect();
    tool_summary.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.name.cmp(&b.name)));
    let points: Vec<TracePoint> = derived::usage_points_of(conn, session_id)?
        .into_iter()
        .map(|p| TracePoint {
            context: context_of(&p),
            at: p.at,
            actor_id: p.actor_id,
            model: p.model,
            cache_read: p.cache_read,
            output: p.output,
        })
        .collect();
    let prompts = derived::all_instants_of(conn, session_id)?;
    let times = segments
        .iter()
        .flat_map(|g| [g.start_at, g.end_at])
        .chain(tools.iter().map(|t| t.at))
        .chain(points.iter().map(|p| p.at))
        .chain(prompts.iter().copied())
        .chain(first)
        .chain(last);
    let (start, end) = times.fold((i64::MAX, i64::MIN), |(lo, hi), t| (lo.min(t), hi.max(t)));
    if start > end {
        return Err(anyhow!("session has no timed evidence"));
    }
    Ok(SessionTrace {
        session_id: session_id.to_owned(),
        timezone: zone.name.clone(),
        start,
        end,
        actors,
        prompts,
        tools,
        tool_summary,
        points,
    })
}
