//! Projects and repository-to-project mappings (S3.1, attribution §2).
//!
//! A **project** is a name the user chooses; a **mapping** says which
//! repository belongs to which project. Both are user-defined and confirmed by
//! definition. They are a *view* over counted time: the minutes stay on their
//! repositories in the ledger, and the day groups them by project when it is
//! read. Changing a mapping therefore never edits history, and a reviewed day
//! keeps the targets it was reviewed with.
//!
//! Stored as two settings (`projects`, `repository_projects`), each change a
//! user action, so the ledger keeps the full history of who mapped what when.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracr_store::ledger;

pub const PROJECTS_KEY: &str = "projects";
pub const REPOSITORY_PROJECTS_KEY: &str = "repository_projects";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
}

/// The projects and which repository (by key) belongs to which project (by id).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProjectConfig {
    pub projects: Vec<Project>,
    pub repository_projects: BTreeMap<String, String>,
}

/// A repository seen in agent sessions, offered for mapping.
#[derive(Debug, Clone, Serialize)]
pub struct KnownRepository {
    pub key: String,
    pub label: String,
}

/// What the user can do with projects.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ProjectOp {
    Create {
        name: String,
    },
    Rename {
        id: String,
        name: String,
    },
    /// Removes the project and the mappings that point to it.
    Delete {
        id: String,
    },
    /// `project: None` takes the repository out of every project.
    Map {
        repository: String,
        project: Option<String>,
    },
}

impl ProjectOp {
    pub fn action(&self) -> &'static str {
        match self {
            Self::Create { .. } => "create",
            Self::Rename { .. } => "rename",
            Self::Delete { .. } => "delete",
            Self::Map { .. } => "map",
        }
    }
}

/// One line of "counted by project".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectTotal {
    /// `project` (a mapped project), or `other` (a repository without a
    /// project, a category, `unattributed` and the like: kept as it was).
    pub kind: &'static str,
    pub project_id: Option<String>,
    pub label: String,
    pub ms: i64,
    /// The repositories inside a project, largest first.
    pub parts: Vec<PartTotal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PartTotal {
    pub label: String,
    pub ms: i64,
}

pub fn read_config(conn: &Connection) -> Result<ProjectConfig> {
    let projects = match ledger::current_setting(conn, PROJECTS_KEY)? {
        Some(v) => serde_json::from_value(v).map_err(|e| anyhow!("setting projects: {e}"))?,
        None => Vec::new(),
    };
    let repository_projects = match ledger::current_setting(conn, REPOSITORY_PROJECTS_KEY)? {
        Some(v) => {
            serde_json::from_value(v).map_err(|e| anyhow!("setting repository_projects: {e}"))?
        }
        None => BTreeMap::new(),
    };
    Ok(ProjectConfig {
        projects,
        repository_projects,
    })
}

/// A project name as stored: trimmed, 1 to 80 characters, no control characters.
pub fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("a project needs a name");
    }
    if name.chars().count() > 80 || name.chars().any(char::is_control) {
        bail!("a project name has at most 80 characters and no control characters");
    }
    Ok(name.to_owned())
}

/// Applies `op` to `config`. Returns the project id the operation concerns
/// (the new id for `Create`, empty for a `Map` without a project).
pub fn apply(
    config: &mut ProjectConfig,
    op: &ProjectOp,
    new_id: impl FnOnce() -> String,
) -> Result<String> {
    let taken = |config: &ProjectConfig, name: &str, except: Option<&str>| {
        config
            .projects
            .iter()
            .any(|p| Some(p.id.as_str()) != except && p.name.to_lowercase() == name.to_lowercase())
    };
    match op {
        ProjectOp::Create { name } => {
            let name = clean_name(name)?;
            if taken(config, &name, None) {
                bail!("there is already a project called {name:?}");
            }
            let id = new_id();
            config.projects.push(Project {
                id: id.clone(),
                name,
            });
            Ok(id)
        }
        ProjectOp::Rename { id, name } => {
            let name = clean_name(name)?;
            if taken(config, &name, Some(id)) {
                bail!("there is already a project called {name:?}");
            }
            let project = config
                .projects
                .iter_mut()
                .find(|p| &p.id == id)
                .ok_or_else(|| anyhow!("unknown project"))?;
            project.name = name;
            Ok(id.clone())
        }
        ProjectOp::Delete { id } => {
            let before = config.projects.len();
            config.projects.retain(|p| &p.id != id);
            if config.projects.len() == before {
                bail!("unknown project");
            }
            config.repository_projects.retain(|_, p| p != id);
            Ok(id.clone())
        }
        ProjectOp::Map {
            repository,
            project,
        } => {
            if repository.trim().is_empty() {
                bail!("a repository is needed");
            }
            match project {
                Some(id) => {
                    if !config.projects.iter().any(|p| &p.id == id) {
                        bail!("unknown project");
                    }
                    config
                        .repository_projects
                        .insert(repository.clone(), id.clone());
                    Ok(id.clone())
                }
                None => {
                    config.repository_projects.remove(repository);
                    Ok(String::new())
                }
            }
        }
    }
}

/// Groups counted minutes (by target label, as the day reports them) by
/// project. Empty while no project exists, so the day keeps its repository
/// line. A repository label shared by two repositories that sit in different
/// projects cannot be told apart by label and stays unmapped.
pub fn project_totals(
    counted_by_target: &BTreeMap<String, i64>,
    config: &ProjectConfig,
) -> Vec<ProjectTotal> {
    if config.projects.is_empty() {
        return Vec::new();
    }
    // label -> Some(project) when every repository with this label agrees.
    let mut by_label: BTreeMap<String, Option<&str>> = BTreeMap::new();
    for (repo, project) in &config.repository_projects {
        let label = tracr_git::repository_label(repo);
        match by_label.get(&label) {
            Some(Some(p)) if *p != project.as_str() => {
                by_label.insert(label, None);
            }
            Some(None) => {}
            _ => {
                by_label.insert(label, Some(project.as_str()));
            }
        }
    }
    let mut groups: BTreeMap<&str, ProjectTotal> = BTreeMap::new();
    let mut others: Vec<ProjectTotal> = Vec::new();
    for (label, &ms) in counted_by_target {
        let project = by_label
            .get(label)
            .copied()
            .flatten()
            .and_then(|id| config.projects.iter().find(|p| p.id == id));
        match project {
            Some(p) => {
                let g = groups.entry(p.id.as_str()).or_insert_with(|| ProjectTotal {
                    kind: "project",
                    project_id: Some(p.id.clone()),
                    label: p.name.clone(),
                    ms: 0,
                    parts: Vec::new(),
                });
                g.ms += ms;
                g.parts.push(PartTotal {
                    label: label.clone(),
                    ms,
                });
            }
            None => others.push(ProjectTotal {
                kind: "other",
                project_id: None,
                label: label.clone(),
                ms,
                parts: Vec::new(),
            }),
        }
    }
    let mut out: Vec<ProjectTotal> = groups.into_values().collect();
    for g in &mut out {
        g.parts
            .sort_by(|a, b| b.ms.cmp(&a.ms).then(a.label.cmp(&b.label)));
    }
    out.sort_by(|a, b| b.ms.cmp(&a.ms).then(a.label.cmp(&b.label)));
    others.sort_by(|a, b| b.ms.cmp(&a.ms).then(a.label.cmp(&b.label)));
    out.extend(others);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ProjectConfig {
        let mut c = ProjectConfig::default();
        let mut n = 0;
        for name in ["Acme", "Side"] {
            apply(&mut c, &ProjectOp::Create { name: name.into() }, || {
                n += 1;
                format!("prj_{n}")
            })
            .unwrap();
        }
        c
    }

    #[test]
    fn names_are_trimmed_unique_ignoring_case_and_bounded() {
        let mut c = config();
        assert_eq!(c.projects[0].name, "Acme");
        let dup = apply(
            &mut c,
            &ProjectOp::Create {
                name: "  acme ".into(),
            },
            || "x".into(),
        );
        assert!(dup.unwrap_err().to_string().contains("already"));
        assert!(clean_name("   ").is_err());
        assert!(clean_name(&"x".repeat(81)).is_err());
        assert!(clean_name("a\u{0}b").is_err());
        // Renaming to its own name in another case is fine; to another project's is not.
        apply(
            &mut c,
            &ProjectOp::Rename {
                id: "prj_1".into(),
                name: "ACME".into(),
            },
            || "x".into(),
        )
        .unwrap();
        assert!(
            apply(
                &mut c,
                &ProjectOp::Rename {
                    id: "prj_1".into(),
                    name: "side".into()
                },
                || "x".into()
            )
            .is_err()
        );
    }

    #[test]
    fn deleting_a_project_drops_its_mappings() {
        let mut c = config();
        for (repo, p) in [
            ("github.com/acme/web", "prj_1"),
            ("github.com/acme/api", "prj_1"),
            ("github.com/me/notes", "prj_2"),
        ] {
            apply(
                &mut c,
                &ProjectOp::Map {
                    repository: repo.into(),
                    project: Some(p.into()),
                },
                || "x".into(),
            )
            .unwrap();
        }
        apply(&mut c, &ProjectOp::Delete { id: "prj_1".into() }, || {
            "x".into()
        })
        .unwrap();
        assert_eq!(c.repository_projects.len(), 1);
        assert_eq!(c.repository_projects["github.com/me/notes"], "prj_2");
        assert!(
            apply(
                &mut c,
                &ProjectOp::Map {
                    repository: "r".into(),
                    project: Some("prj_1".into())
                },
                || "x".into()
            )
            .is_err()
        );
        apply(
            &mut c,
            &ProjectOp::Map {
                repository: "github.com/me/notes".into(),
                project: None,
            },
            || "x".into(),
        )
        .unwrap();
        assert!(c.repository_projects.is_empty());
    }

    #[test]
    fn minutes_group_by_project_and_the_rest_stays_as_it_was() {
        let mut c = config();
        for (repo, p) in [
            ("github.com/acme/web", "prj_1"),
            ("github.com/acme/api", "prj_1"),
            ("github.com/me/notes", "prj_2"),
        ] {
            apply(
                &mut c,
                &ProjectOp::Map {
                    repository: repo.into(),
                    project: Some(p.into()),
                },
                || "x".into(),
            )
            .unwrap();
        }
        // Day labels are `owner/name`, as repository_label gives them.
        let day: BTreeMap<String, i64> = [
            ("acme/web", 200),
            ("acme/api", 60),
            ("me/notes", 90),
            ("me/unmapped", 30),
            ("Meeting", 15),
            ("unattributed", 10),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let t = project_totals(&day, &c);
        let flat: Vec<(&str, &str, i64)> =
            t.iter().map(|x| (x.kind, x.label.as_str(), x.ms)).collect();
        assert_eq!(
            flat,
            vec![
                ("project", "Acme", 260),
                ("project", "Side", 90),
                ("other", "me/unmapped", 30),
                ("other", "Meeting", 15),
                ("other", "unattributed", 10),
            ]
        );
        assert_eq!(
            t[0].parts,
            vec![
                PartTotal {
                    label: "acme/web".into(),
                    ms: 200
                },
                PartTotal {
                    label: "acme/api".into(),
                    ms: 60
                }
            ]
        );
        // The same minutes are in both views: nothing is added or lost.
        assert_eq!(
            t.iter().map(|x| x.ms).sum::<i64>(),
            day.values().sum::<i64>()
        );
        // No project yet: the day keeps its repository line.
        assert!(project_totals(&day, &ProjectConfig::default()).is_empty());
    }

    #[test]
    fn a_label_shared_by_repositories_of_different_projects_stays_unmapped() {
        let mut c = config();
        for (repo, p) in [("github.com/a/web", "prj_1"), ("gitlab.com/a/web", "prj_2")] {
            apply(
                &mut c,
                &ProjectOp::Map {
                    repository: repo.into(),
                    project: Some(p.into()),
                },
                || "x".into(),
            )
            .unwrap();
        }
        let day: BTreeMap<String, i64> = [("a/web".to_owned(), 40)].into_iter().collect();
        let t = project_totals(&day, &c);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].kind, "other");
    }
}

/// A repository inside a project entry.
#[derive(Debug, Clone, Serialize)]
pub struct RepoPart {
    pub key: String,
    pub label: String,
    pub sessions: i64,
    pub agent_ms: i64,
}

/// One row of the projects page: a project, a repository without a project,
/// or the sessions that ran outside any repository.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectEntry {
    /// Stable per entry: `project:<id>`, `repo:<key>`, `dir:<cwd>` or `none`.
    pub key: String,
    /// `project`, `repository`, `folder` (a working folder outside any repository) or `none`.
    pub kind: &'static str,
    pub project_id: Option<String>,
    pub label: String,
    pub repositories: Vec<RepoPart>,
    pub session_ids: Vec<String>,
    pub sessions: i64,
    /// Agent execution, never human time.
    pub agent_ms: i64,
    /// Estimated API cost of the identified usage in the range.
    pub cost: crate::day::CostView,
    pub first_at: Option<i64>,
    pub last_at: Option<i64>,
    pub branches: Vec<String>,
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectOverview {
    pub from: String,
    pub to: String,
    pub timezone: String,
    pub entries: Vec<ProjectEntry>,
    pub pricing: crate::pricing::PricingInfo,
}

/// Groups the sessions of a range by project. Defined projects always appear
/// (without activity they are empty); a repository that belongs to no project
/// appears on its own; sessions without a repository are one `none` entry.
pub fn overview(list: &crate::day::SessionList, config: &ProjectConfig) -> ProjectOverview {
    let mut by: BTreeMap<String, ProjectEntry> = BTreeMap::new();
    for p in &config.projects {
        by.insert(
            format!("project:{}", p.id),
            blank_entry(
                format!("project:{}", p.id),
                "project",
                Some(p.id.clone()),
                p.name.clone(),
            ),
        );
    }
    for s in &list.sessions {
        let (key, kind, pid, label) = match &s.repository {
            Some(repo) => match config.repository_projects.get(repo) {
                Some(id) if config.projects.iter().any(|p| &p.id == id) => {
                    let name = config
                        .projects
                        .iter()
                        .find(|p| &p.id == id)
                        .map(|p| p.name.clone());
                    (
                        format!("project:{id}"),
                        "project",
                        Some(id.clone()),
                        name.unwrap_or_default(),
                    )
                }
                _ => (
                    format!("repo:{repo}"),
                    "repository",
                    None,
                    s.repository_label.clone().unwrap_or_else(|| repo.clone()),
                ),
            },
            // No repository (none approved, or not a git folder): the working folder stands in.
            None => match &s.cwd {
                Some(cwd) => (
                    format!("dir:{cwd}"),
                    "folder",
                    None,
                    s.label
                        .clone()
                        .unwrap_or_else(|| cwd.rsplit('/').next().unwrap_or(cwd).to_owned()),
                ),
                None => ("none".to_owned(), "none", None, String::new()),
            },
        };
        let e = by
            .entry(key.clone())
            .or_insert_with(|| blank_entry(key, kind, pid, label));
        let agent_ms: i64 = s.actors.iter().map(|a| a.execution_ms).sum();
        e.sessions += 1;
        e.agent_ms += agent_ms;
        e.cost.absorb(&s.cost);
        e.session_ids.push(s.session_id.clone());
        e.first_at = min_opt(e.first_at, s.first_evidence_at);
        e.last_at = max_opt(e.last_at, s.last_evidence_at);
        if let Some(b) = &s.git_branch
            && !e.branches.contains(b)
        {
            e.branches.push(b.clone());
        }
        if !e.providers.contains(&s.provider) {
            e.providers.push(s.provider.clone());
        }
        if let Some(repo) = &s.repository {
            let label = s.repository_label.clone().unwrap_or_else(|| repo.clone());
            match e.repositories.iter_mut().find(|r| &r.key == repo) {
                Some(r) => {
                    r.sessions += 1;
                    r.agent_ms += agent_ms;
                }
                None => e.repositories.push(RepoPart {
                    key: repo.clone(),
                    label,
                    sessions: 1,
                    agent_ms,
                }),
            }
        }
    }
    let mut entries: Vec<ProjectEntry> = by.into_values().collect();
    for e in &mut entries {
        e.repositories
            .sort_by_key(|r| std::cmp::Reverse(r.agent_ms));
    }
    entries.sort_by(|a, b| {
        b.last_at
            .cmp(&a.last_at)
            .then_with(|| a.label.cmp(&b.label))
    });
    ProjectOverview {
        from: list.from.clone(),
        to: list.to.clone(),
        timezone: list.timezone.clone(),
        entries,
        pricing: list.pricing.clone(),
    }
}

fn blank_entry(
    key: String,
    kind: &'static str,
    project_id: Option<String>,
    label: String,
) -> ProjectEntry {
    ProjectEntry {
        key,
        kind,
        project_id,
        label,
        repositories: Vec::new(),
        session_ids: Vec::new(),
        sessions: 0,
        agent_ms: 0,
        cost: crate::day::CostView::default(),
        first_at: None,
        last_at: None,
        branches: Vec::new(),
        providers: Vec::new(),
    }
}

fn min_opt(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, None) | (None, x) => x,
    }
}

fn max_opt(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) | (None, x) => x,
    }
}
