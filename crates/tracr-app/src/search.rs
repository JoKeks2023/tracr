//! Session search over metadata (TRACR-21): find sessions by period,
//! provider, project, branch, model, tool, subagents, binding and estimated
//! cost. Never by content: tracr does not store prompts or messages
//! (payload `never_stored`, mvp M3), so there is no full-text search.
//!
//! The result also lists the values found in the period (facets), so a UI
//! offers them as dropdowns instead of free text.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::day::{CostView, SessionDay, SessionList};

/// Filters; every one that is set must hold. Text matches ignore case.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionQuery {
    /// First and last day, `YYYY-MM-DD`, inclusive.
    pub from: String,
    pub to: String,
    /// In the session's label, working folder, branch, repository or ID.
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    /// Project ID: the session is bound to it, or its repository is mapped to it.
    #[serde(default)]
    pub project: Option<String>,
    /// Exact branch name.
    #[serde(default)]
    pub branch: Option<String>,
    /// Exact model ID used by at least one usage record.
    #[serde(default)]
    pub model: Option<String>,
    /// Exact tool name called at least once in the period.
    #[serde(default)]
    pub tool: Option<String>,
    /// `true`: only sessions with subagents (known actors); `false`: only without.
    #[serde(default)]
    pub subagents: Option<bool>,
    /// A target the session is bound to (`other:TRACR-12`, `project:…`).
    #[serde(default)]
    pub bound_to: Option<String>,
    /// Estimated cost at least this many nano-units. Unpriced sessions never match.
    #[serde(default)]
    pub min_cost_nano: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Facets {
    pub providers: Vec<String>,
    pub branches: Vec<String>,
    pub models: Vec<String>,
    pub tools: Vec<String>,
    pub projects: Vec<FacetProject>,
    pub bound_to: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct FacetProject {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSearch {
    /// Matching sessions (newest first) and their summed estimate.
    pub list: SessionList,
    /// Sessions in the period before filtering.
    pub total: usize,
    pub facets: Facets,
}

/// Tool names called in a session between `start` and `end`.
fn tools_of(conn: &Connection, session: &str, start: i64, end: i64) -> Result<BTreeSet<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT tool_name FROM tool_call
         WHERE session_id = ?1 AND tool_name IS NOT NULL AND started_at >= ?2 AND started_at < ?3",
    )?;
    let rows = stmt.query_map(rusqlite::params![session, start, end], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn has(hay: &Option<String>, needle: &str) -> bool {
    hay.as_deref()
        .is_some_and(|h| h.to_lowercase().contains(needle))
}

/// Facts about one session the filters look at.
pub struct SessionFacts<'a> {
    pub session: &'a SessionDay,
    pub tools: BTreeSet<String>,
    /// Project IDs: bound to, or mapped from the repository.
    pub projects: BTreeSet<String>,
}

pub fn matches(q: &SessionQuery, f: &SessionFacts<'_>) -> bool {
    let s = f.session;
    if let Some(t) = q.text.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        let t = t.to_lowercase();
        let any = has(&s.label, &t)
            || has(&s.cwd, &t)
            || has(&s.git_branch, &t)
            || has(&s.repository_label, &t)
            || s.native_session_id.to_lowercase().contains(&t);
        if !any {
            return false;
        }
    }
    if q.provider.as_ref().is_some_and(|p| *p != s.provider) {
        return false;
    }
    if q.branch
        .as_ref()
        .is_some_and(|b| s.git_branch.as_ref() != Some(b))
    {
        return false;
    }
    if let Some(m) = &q.model
        && !s.usage.iter().any(|u| u.model.as_ref() == Some(m))
    {
        return false;
    }
    if q.tool.as_ref().is_some_and(|t| !f.tools.contains(t)) {
        return false;
    }
    if let Some(want) = q.subagents {
        let any = s.actors.iter().any(|a| a.actor_kind == "known");
        if any != want {
            return false;
        }
    }
    if q.project.as_ref().is_some_and(|p| !f.projects.contains(p)) {
        return false;
    }
    if q.bound_to.as_ref().is_some_and(|t| !s.bound_to.contains(t)) {
        return false;
    }
    if let Some(min) = q.min_cost_nano
        && s.cost.amount_nano.is_none_or(|a| a < min)
    {
        return false;
    }
    true
}

/// Filters `list` by `q`, keeping the period's facets.
pub fn search(
    conn: &Connection,
    q: &SessionQuery,
    list: SessionList,
    (start, end): (i64, i64),
    projects: &crate::projects::ProjectConfig,
) -> Result<SessionSearch> {
    let names: BTreeMap<&str, &str> = projects
        .projects
        .iter()
        .map(|p| (p.id.as_str(), p.name.as_str()))
        .collect();
    let mut facets = Facets::default();
    let mut provider_set = BTreeSet::new();
    let mut branch_set = BTreeSet::new();
    let mut model_set = BTreeSet::new();
    let mut tool_set = BTreeSet::new();
    let mut project_set = BTreeSet::new();
    let mut bound_set = BTreeSet::new();
    let total = list.sessions.len();
    let mut keep: Vec<SessionDay> = Vec::new();
    let mut cost = CostView::default();
    for s in &list.sessions {
        let tools = tools_of(conn, &s.session_id, start, end)?;
        let mut ids: BTreeSet<String> = s
            .bound_to
            .iter()
            .filter_map(|t| t.strip_prefix("project:").map(str::to_owned))
            .collect();
        if let Some(p) = s
            .repository
            .as_ref()
            .and_then(|r| projects.repository_projects.get(r))
        {
            ids.insert(p.clone());
        }
        provider_set.insert(s.provider.clone());
        branch_set.extend(s.git_branch.clone());
        model_set.extend(s.usage.iter().filter_map(|u| u.model.clone()));
        tool_set.extend(tools.iter().cloned());
        bound_set.extend(s.bound_to.iter().cloned());
        for id in &ids {
            if let Some(name) = names.get(id.as_str()) {
                project_set.insert(FacetProject {
                    id: id.clone(),
                    name: (*name).to_owned(),
                });
            }
        }
        let facts = SessionFacts {
            session: s,
            tools,
            projects: ids,
        };
        if matches(q, &facts) {
            cost.absorb(&s.cost);
            keep.push(s.clone());
        }
    }
    facets.providers = provider_set.into_iter().collect();
    facets.branches = branch_set.into_iter().collect();
    facets.models = model_set.into_iter().collect();
    facets.tools = tool_set.into_iter().collect();
    facets.projects = project_set.into_iter().collect();
    facets.bound_to = bound_set.into_iter().collect();
    Ok(SessionSearch {
        list: SessionList {
            sessions: keep,
            cost,
            ..list
        },
        total,
        facets,
    })
}
