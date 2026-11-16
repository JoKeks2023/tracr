//! Bindings and issue suggestions (S3.2, mvp M6/M7, attribution §4–§6).
//!
//! A **binding** is a standing statement by the user: "this session (or
//! this repository) is for that target", valid from a time, until it is
//! ended. It acts on the **attribution axis only** (attribution §5, level 3:
//! below corrections and manual entries, above inference). It never creates
//! or removes human time (TW-09): a binding on a session whose time is
//! unknown leaves that time unknown.
//!
//! A **suggestion** is a candidate target read from evidence (the session's
//! branch name). It never counts until the user accepts it, which creates a
//! binding (attribution §4: a branch key is a candidate, not ownership). A
//! rejected (subject, target) pair is remembered and not suggested again
//! (AT-05), whatever the evidence or rule version.
//!
//! Decisions taken here (T-0023):
//! - A binding applies to a human-timeline piece when it is valid at the
//!   piece's start. Pieces are short (cut at prompts, switches and day
//!   boundaries); a binding that starts mid-piece applies from the next one.
//! - Bindings on the same piece that name different targets are
//!   `conflicting` (OQ-P-20: no silent last-write rule); none counts for a
//!   target until the user resolves it.
//! - Issue keys are suggested as free labels (`other:TRACR-12`). A scoped
//!   Huly or GitHub reference needs its workspace or repository (attribution
//!   §2) and comes with the integrations (S3.3/S3.4).
//!
//! Stored as two settings (`bindings`, `rejected_suggestions`); every change
//! is one user action `setting.bindings.<op>`.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, bail};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracr_core::counting::TargetRef;
use tracr_store::ledger;

pub const BINDINGS_KEY: &str = "bindings";
pub const REJECTED_KEY: &str = "rejected_suggestions";

/// What a binding or a rejection is about.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Subject {
    /// A provider session, by its native session ID.
    Session(String),
    /// A repository, by its normalized key (as in `repository:<key>` targets).
    Repository(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub id: String,
    pub subject: Subject,
    /// A target key (`project:…`, `category:…`, `other:…`).
    pub target: String,
    pub valid_from: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<i64>,
}

impl Binding {
    pub fn valid_at(&self, at: i64) -> bool {
        self.valid_from <= at && self.valid_to.is_none_or(|to| at < to)
    }

    pub fn overlaps(&self, start: i64, end: i64) -> bool {
        self.valid_from < end && self.valid_to.is_none_or(|to| start < to)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejection {
    pub subject: Subject,
    pub target: String,
    pub at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BindingConfig {
    pub bindings: Vec<Binding>,
    pub rejected: Vec<Rejection>,
}

/// What the user can do with bindings and suggestions.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BindingOp {
    /// Binds `subject` to `target` from `valid_from` (default: now).
    Bind {
        subject: Subject,
        target: String,
        valid_from: Option<i64>,
    },
    /// Ends a binding at `at` (default: now). History before stays bound.
    End { id: String, at: Option<i64> },
    /// Rejects a suggested target for a subject: never suggested again.
    Reject { subject: Subject, target: String },
}

impl BindingOp {
    pub fn action(&self) -> &'static str {
        match self {
            Self::Bind { .. } => "bind",
            Self::End { .. } => "end",
            Self::Reject { .. } => "reject",
        }
    }
}

pub fn read_config(conn: &Connection) -> Result<BindingConfig> {
    let read = |key: &str| -> Result<Option<serde_json::Value>> {
        Ok(ledger::current_setting(conn, key)?)
    };
    Ok(BindingConfig {
        bindings: match read(BINDINGS_KEY)? {
            Some(v) => serde_json::from_value(v).map_err(|e| anyhow!("setting bindings: {e}"))?,
            None => Vec::new(),
        },
        rejected: match read(REJECTED_KEY)? {
            Some(v) => serde_json::from_value(v)
                .map_err(|e| anyhow!("setting rejected_suggestions: {e}"))?,
            None => Vec::new(),
        },
    })
}

/// A binding names a project, a category or a label. A repository is
/// observed, not bound to.
fn check_target(target: &str) -> Result<()> {
    match TargetRef::parse(target) {
        Some(TargetRef::Repository(_)) => {
            bail!("a binding names a project, a category or a label, not a repository")
        }
        Some(_) => Ok(()),
        None => bail!("not a valid target: {target}"),
    }
}

fn check_subject(subject: &Subject) -> Result<()> {
    let id = match subject {
        Subject::Session(id) | Subject::Repository(id) => id,
    };
    if id.trim().is_empty() || id.chars().count() > 300 || id.chars().any(char::is_control) {
        bail!("not a valid subject");
    }
    Ok(())
}

/// Applies `op` to `config` at `now`. Returns the binding id the operation
/// concerns (the new id for `Bind`, empty for `Reject`).
pub fn apply(
    config: &mut BindingConfig,
    op: &BindingOp,
    now: i64,
    new_id: impl FnOnce() -> String,
) -> Result<String> {
    match op {
        BindingOp::Bind {
            subject,
            target,
            valid_from,
        } => {
            check_subject(subject)?;
            check_target(target)?;
            let from = valid_from.unwrap_or(now);
            if from > now {
                bail!("a binding cannot start in the future");
            }
            if config.bindings.iter().any(|b| {
                b.subject == *subject
                    && b.target == *target
                    && b.valid_to.is_none_or(|to| to > from)
            }) {
                bail!("this binding already exists");
            }
            let id = new_id();
            config.bindings.push(Binding {
                id: id.clone(),
                subject: subject.clone(),
                target: target.clone(),
                valid_from: from,
                valid_to: None,
            });
            Ok(id)
        }
        BindingOp::End { id, at } => {
            let b = config
                .bindings
                .iter_mut()
                .find(|b| b.id == *id)
                .ok_or_else(|| anyhow!("no binding {id}"))?;
            if b.valid_to.is_some() {
                bail!("this binding has already ended");
            }
            let at = at.unwrap_or(now);
            if at < b.valid_from {
                bail!("a binding cannot end before it starts");
            }
            if at > now {
                bail!("a binding cannot end in the future");
            }
            b.valid_to = Some(at);
            Ok(id.clone())
        }
        BindingOp::Reject { subject, target } => {
            check_subject(subject)?;
            check_target(target)?;
            if !config
                .rejected
                .iter()
                .any(|r| r.subject == *subject && r.target == *target)
            {
                config.rejected.push(Rejection {
                    subject: subject.clone(),
                    target: target.clone(),
                    at: now,
                });
            }
            Ok(String::new())
        }
    }
}

impl BindingConfig {
    /// The bindings valid at `at` for any of `subjects`.
    pub fn active_at<'a>(&'a self, subjects: &[Subject], at: i64) -> Vec<&'a Binding> {
        self.bindings
            .iter()
            .filter(|b| b.valid_at(at) && subjects.contains(&b.subject))
            .collect()
    }

    pub fn is_rejected(&self, subject: &Subject, target: &str) -> bool {
        self.rejected
            .iter()
            .any(|r| r.subject == *subject && r.target == target)
    }
}

/// Words that look like issue keys in branch names but are not.
const NOT_KEYS: &[&str] = &["UTF", "SHA", "ISO", "RFC", "MD", "X", "V", "PR"];

/// Issue keys (`ABC-123`) in a branch name, in order, uppercased.
/// `tracr-46-review-speed` → `TRACR-46`; `feature/SERV-7_fix` → `SERV-7`.
pub fn issue_keys(branch: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let chars: Vec<char> = branch.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // A key starts at a word boundary with a letter.
        let boundary = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
        if !(boundary && chars[i].is_ascii_alphabetic()) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let prefix: String = chars[start..i].iter().collect::<String>().to_uppercase();
        if i + 1 < chars.len() && chars[i] == '-' && chars[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            let ends_word = j == chars.len() || !chars[j].is_ascii_alphanumeric();
            // `2026-11`, `1-2-3`: a date or version, not a key.
            let dated = j + 1 < chars.len() && chars[j] == '-' && chars[j + 1].is_ascii_digit();
            let len = prefix.chars().count();
            if ends_word
                && !dated
                && (2..=10).contains(&len)
                && prefix
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic())
                && !NOT_KEYS.contains(&prefix.as_str())
                && j - (i + 1) <= 7
            {
                let key = format!("{prefix}-{}", chars[i + 1..j].iter().collect::<String>());
                if !out.contains(&key) {
                    out.push(key);
                }
            }
            i = j;
        }
    }
    out
}

/// A target suggested for a session, with the evidence it comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    pub target: String,
    pub label: String,
    /// `branch:<name>`.
    pub evidence: String,
}

/// Suggestions for a session: issue keys in its branch, minus rejected
/// pairs and targets the session is already bound to.
pub fn suggestions(
    config: &BindingConfig,
    native_session: &str,
    branch: Option<&str>,
    bound: &BTreeSet<String>,
) -> Vec<Suggestion> {
    let Some(branch) = branch else {
        return Vec::new();
    };
    let subject = Subject::Session(native_session.to_owned());
    issue_keys(branch)
        .into_iter()
        .map(|k| (format!("other:{k}"), k))
        .filter(|(t, _)| !config.is_rejected(&subject, t) && !bound.contains(t))
        .map(|(target, label)| Suggestion {
            target,
            label,
            evidence: format!("branch:{branch}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bind(c: &mut BindingConfig, s: Subject, t: &str, from: i64, now: i64) -> Result<String> {
        let n = c.bindings.len();
        apply(
            c,
            &BindingOp::Bind {
                subject: s,
                target: t.into(),
                valid_from: Some(from),
            },
            now,
            || format!("bnd_{n}"),
        )
    }

    #[test]
    fn issue_keys_are_read_from_branch_names() {
        assert_eq!(issue_keys("tracr-46-saving-a-review"), ["TRACR-46"]);
        assert_eq!(issue_keys("feature/SERV-7_fix"), ["SERV-7"]);
        assert_eq!(issue_keys("TRACR-12+TRACR-15"), ["TRACR-12", "TRACR-15"]);
        assert!(issue_keys("main").is_empty());
        assert!(issue_keys("fix-utf-8-decoding").is_empty());
        assert!(
            issue_keys("release-2026-11").is_empty(),
            "a year is not a key"
        );
        assert!(issue_keys("a-1").is_empty(), "too short a prefix");
        assert!(
            issue_keys("abc-12x").is_empty(),
            "the number must end the word"
        );
    }

    #[test]
    fn a_binding_is_valid_from_its_start_until_it_ends() {
        let mut c = BindingConfig::default();
        let s = Subject::Session("s1".into());
        let id = bind(&mut c, s.clone(), "other:TRACR-12", 100, 1000).unwrap();
        assert!(
            bind(&mut c, s.clone(), "other:TRACR-12", 200, 1000).is_err(),
            "duplicate"
        );
        apply(
            &mut c,
            &BindingOp::End {
                id: id.clone(),
                at: Some(500),
            },
            1000,
            String::new,
        )
        .unwrap();
        let b = &c.bindings[0];
        assert!(!b.valid_at(99) && b.valid_at(100) && b.valid_at(499) && !b.valid_at(500));
        // Ended: the same pair can be bound again from the end on.
        bind(&mut c, s, "other:TRACR-12", 500, 1000).unwrap();
        assert!(apply(&mut c, &BindingOp::End { id, at: None }, 1000, String::new).is_err());
    }

    #[test]
    fn bindings_name_no_repositories_and_never_start_in_the_future() {
        let mut c = BindingConfig::default();
        let s = Subject::Repository("github.com/example/repo".into());
        assert!(bind(&mut c, s.clone(), "repository:github.com/x/y", 0, 10).is_err());
        assert!(bind(&mut c, s.clone(), "nonsense", 0, 10).is_err());
        assert!(bind(&mut c, s, "category:meeting", 11, 10).is_err());
    }

    #[test]
    fn rejected_and_bound_targets_are_not_suggested() {
        let mut c = BindingConfig::default();
        let s = Subject::Session("s1".into());
        let branch = Some("tracr-12-and-TRACR-15");
        let all = suggestions(&c, "s1", branch, &BTreeSet::new());
        assert_eq!(
            all.iter().map(|x| x.target.as_str()).collect::<Vec<_>>(),
            ["other:TRACR-12", "other:TRACR-15"]
        );
        apply(
            &mut c,
            &BindingOp::Reject {
                subject: s,
                target: "other:TRACR-15".into(),
            },
            5,
            String::new,
        )
        .unwrap();
        let bound: BTreeSet<String> = ["other:TRACR-12".to_owned()].into();
        assert!(suggestions(&c, "s1", branch, &bound).is_empty());
        // Another session is not affected by s1's rejection.
        assert_eq!(suggestions(&c, "s2", branch, &BTreeSet::new()).len(), 2);
    }
}
