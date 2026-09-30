//! Git context probes (architecture §9): plans read-only `git` commands for
//! an observed working directory and parses their output into retained facts.
//! Pure: no IO. `tracr-ingest` executes the plan, only under approved
//! repository roots (PS-02).
//!
//! Remote URLs are reduced to `host/owner/name`: user names and tokens in a
//! URL never reach the ledger.

use tracr_core::adapter::{ProbeAdapter, ProbeOutput};
use tracr_core::events::FactKind;
use tracr_core::evidence::{Facts, GitFacts, SourceKind};

/// Source kind of git context observations (§9).
pub const SOURCE_KIND: &str = "git";

/// The probes, in order. Each is read-only; the executor also sets
/// `GIT_OPTIONAL_LOCKS=0` so no probe takes a lock or refreshes the index.
pub const PROBES: &[&[&str]] = &[
    &["rev-parse", "--show-toplevel", "--git-common-dir"],
    // Works in a repository without commits; fails (no branch) on a detached HEAD.
    &["symbolic-ref", "--short", "-q", "HEAD"],
    &["remote", "-v"],
];

#[derive(Debug, Default, Clone, Copy)]
pub struct GitProbe;

impl ProbeAdapter for GitProbe {
    fn source_kind(&self) -> SourceKind {
        SourceKind::new(SOURCE_KIND)
    }

    fn adapter_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn fact_kind(&self) -> FactKind {
        FactKind::GitContext
    }

    fn program(&self) -> &'static str {
        "git"
    }

    fn probes(&self) -> &'static [&'static [&'static str]] {
        PROBES
    }

    fn env(&self) -> &'static [(&'static str, &'static str)] {
        &[("GIT_OPTIONAL_LOCKS", "0"), ("GIT_TERMINAL_PROMPT", "0")]
    }

    fn parse(&self, cwd: &str, machine: &str, outputs: &[ProbeOutput]) -> Facts {
        Facts {
            git: Some(parse(cwd, machine, outputs)),
            ..Facts::default()
        }
    }
}

/// Normalizes a remote URL to `host/owner/name`: lowercase, `.git` stripped,
/// SSH (`git@host:owner/name`) and HTTPS forms unified, credentials dropped.
pub fn normalize_remote(url: &str) -> Option<String> {
    let url = url.trim();
    let (host, path) = if let Some(rest) = url.split_once("://").map(|(_, r)| r) {
        let rest = rest.rsplit_once('@').map_or(rest, |(_, r)| r); // drop user[:token]@
        let (host, path) = rest.split_once('/')?;
        (host.split(':').next()?.to_owned(), path.to_owned())
    } else if let Some((user_host, path)) = url.split_once(':') {
        // scp-like: [user@]host:owner/name
        let host = user_host.rsplit_once('@').map_or(user_host, |(_, h)| h);
        if host.contains('/') {
            return None; // a local path
        }
        (host.to_owned(), path.to_owned())
    } else {
        return None;
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if host.is_empty() || path.is_empty() || !path.contains('/') {
        return None;
    }
    Some(format!("{}/{}", host.to_lowercase(), path.to_lowercase()))
}

fn preferred_remote(remote_v: &str) -> Option<String> {
    let mut first: Option<&str> = None;
    for line in remote_v.lines() {
        let mut parts = line.split_whitespace();
        let (Some(name), Some(url)) = (parts.next(), parts.next()) else {
            continue;
        };
        if !line.ends_with("(fetch)") {
            continue;
        }
        if name == "origin" {
            return normalize_remote(url);
        }
        first.get_or_insert(url);
    }
    first.and_then(normalize_remote)
}

/// Parses the probe outputs for `cwd` into git facts. A directory that is not
/// in a repository gives `is_repo = false`, not an error.
pub fn parse(cwd: &str, machine: &str, outputs: &[ProbeOutput]) -> GitFacts {
    let not_repo = || GitFacts {
        cwd: cwd.to_owned(),
        is_repo: false,
        ..GitFacts::default()
    };
    let Some(rev) = outputs.first().filter(|o| o.success) else {
        return not_repo();
    };
    let mut lines = rev.stdout.lines();
    let (Some(toplevel), Some(common)) = (lines.next(), lines.next()) else {
        return not_repo();
    };
    let branch = outputs
        .get(1)
        .filter(|o| o.success)
        .map(|o| o.stdout.trim().to_owned())
        .filter(|b| !b.is_empty());
    // `--git-common-dir` is relative to the cwd when it is the plain `.git`.
    let common_dir = if common.starts_with('/') {
        common.to_owned()
    } else {
        format!("{}/{}", cwd.trim_end_matches('/'), common)
    };
    let remote = outputs
        .get(2)
        .filter(|o| o.success)
        .and_then(|o| preferred_remote(&o.stdout));
    let repository_key = match &remote {
        Some(r) => r.clone(),
        None => format!("local:{machine}:{}", common_dir.trim_end_matches('/')),
    };
    GitFacts {
        cwd: cwd.to_owned(),
        is_repo: true,
        toplevel: Some(toplevel.to_owned()),
        common_dir: Some(common_dir),
        branch,
        remote,
        repository_key: Some(repository_key),
    }
}

/// A short label for a repository key: `owner/name`, or the folder name.
pub fn repository_label(key: &str) -> String {
    if let Some(rest) = key.strip_prefix("local:") {
        let path = rest.split_once(':').map_or(rest, |(_, p)| p);
        let path = path.trim_end_matches("/.git").trim_end_matches('/');
        return path.rsplit('/').next().unwrap_or(path).to_owned();
    }
    key.split_once('/').map_or(key, |(_, p)| p).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remotes_normalize_to_host_owner_name_without_credentials() {
        let want = Some("github.com/jokeks2023/tracr".to_owned());
        assert_eq!(
            normalize_remote("git@github.com:JoKeks2023/tracr.git"),
            want
        );
        assert_eq!(
            normalize_remote("https://github.com/JoKeks2023/tracr"),
            want
        );
        assert_eq!(
            normalize_remote("https://user:ghp_secret@github.com/JoKeks2023/tracr.git"),
            want
        );
        assert_eq!(
            normalize_remote("ssh://git@github.com:22/JoKeks2023/tracr.git"),
            want
        );
        assert_eq!(normalize_remote("/Users/x/repo.git"), None);
        assert_eq!(normalize_remote("../other"), None);
    }

    #[test]
    fn parse_prefers_origin_and_resolves_relative_common_dir() {
        let out = [
            ProbeOutput { success: true, stdout: "/Users/x/p\n.git\n".into() },
            ProbeOutput { success: true, stdout: "main\n".into() },
            ProbeOutput {
                success: true,
                stdout: "fork\tgit@github.com:me/p.git (fetch)\norigin\thttps://github.com/Org/P.git (fetch)\n".into(),
            },
        ];
        let g = parse("/Users/x/p", "m1", &out);
        assert!(g.is_repo);
        assert_eq!(g.common_dir.as_deref(), Some("/Users/x/p/.git"));
        assert_eq!(g.remote.as_deref(), Some("github.com/org/p"));
        assert_eq!(g.repository_key.as_deref(), Some("github.com/org/p"));
        assert_eq!(g.branch.as_deref(), Some("main"));
        assert_eq!(repository_label("github.com/org/p"), "org/p");
    }

    #[test]
    fn a_repo_without_remote_is_keyed_by_machine_and_common_dir() {
        let out = [
            ProbeOutput {
                success: true,
                stdout: "/Users/x/p/wt\n/Users/x/p/.git\n".into(),
            },
            ProbeOutput {
                success: false,
                stdout: String::new(),
            },
            ProbeOutput {
                success: true,
                stdout: String::new(),
            },
        ];
        let g = parse("/Users/x/p/wt", "m1", &out);
        assert_eq!(
            g.repository_key.as_deref(),
            Some("local:m1:/Users/x/p/.git")
        );
        assert_eq!(g.branch, None, "a detached HEAD has no branch");
        assert_eq!(repository_label("local:m1:/Users/x/p/.git"), "p");
    }

    #[test]
    fn a_plain_directory_is_not_a_repo() {
        let out = [ProbeOutput {
            success: false,
            stdout: String::new(),
        }];
        assert!(!parse("/tmp", "m1", &out).is_repo);
    }
}
