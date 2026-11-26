//! Project setup facts and diagnostics (TRACR-23, docs/project-and-agent-setup.md).
//!
//! For each repository tracr has observed under an approved repository
//! root: which agent instruction files exist, which paths they reference,
//! whether a canon and a project MCP configuration exist. Read on demand,
//! never stored, never sent anywhere.
//!
//! Inspection scope (setup §3): presence, size and modification time; inside
//! `CLAUDE.md`/`AGENTS.md` only reference statements (`@path` imports, the
//! path targets they name); inside `.mcp.json` only server names; inside the
//! canon only the number of records and how many are canon. No other text
//! is extracted: canon titles and instruction content would need the
//! separate privacy review setup §3 asks for.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

const INSTRUCTION_FILES: [&str; 2] = ["CLAUDE.md", "AGENTS.md"];
/// Reading stops here: an instruction file larger than this is unusual.
const MAX_READ_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileFact {
    /// Path relative to the repository.
    pub path: String,
    pub bytes: u64,
    pub modified: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InstructionFile {
    pub file: FileFact,
    /// Paths named by `@path` imports, as written (relative to the file).
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CanonFacts {
    pub file: FileFact,
    pub records: usize,
    /// Records whose status is `canon` (the rest are superseded history).
    pub canon: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SetupFinding {
    /// Rule id with version, e.g. `setup.reference_missing@1`.
    pub rule: &'static str,
    /// `warning` or `info`.
    pub severity: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectSetup {
    /// The repository folder (under an approved repository root).
    pub path: String,
    pub instructions: Vec<InstructionFile>,
    pub canon: Option<CanonFacts>,
    /// Server names in the project's `.mcp.json` (never commands, args, env or URLs).
    pub mcp_servers: Option<Vec<String>>,
    pub findings: Vec<SetupFinding>,
}

fn fact(repo: &Path, rel: &str) -> Option<FileFact> {
    let meta = fs::metadata(repo.join(rel)).ok().filter(|m| m.is_file())?;
    Some(FileFact {
        path: rel.to_owned(),
        bytes: meta.len(),
        modified: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64),
    })
}

fn read_small(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if meta.len() > MAX_READ_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// `@path` imports in an instruction file: a token starting with `@` that
/// names a path (contains `/` or ends in a file extension), outside code
/// spans. `@someone` mentions and e-mail addresses are not references.
pub fn references(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let mut in_code = false;
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '`' {
                in_code = !in_code;
            } else if c == '@' && !in_code && (i == 0 || chars[i - 1].is_whitespace()) {
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && (chars[j].is_alphanumeric() || "/._-~".contains(chars[j]))
                {
                    j += 1;
                }
                let token: String = chars[start..j].iter().collect();
                let token = token.trim_end_matches('.').to_owned();
                let has_ext = token.rsplit_once('.').is_some_and(|(stem, ext)| {
                    !stem.is_empty()
                        && (1..=5).contains(&ext.len())
                        && ext.chars().all(|c| c.is_ascii_alphanumeric())
                });
                if (token.contains('/') || has_ext) && !out.contains(&token) {
                    out.push(token);
                }
                i = j;
                continue;
            }
            i += 1;
        }
    }
    out
}

/// Canon record count from `## ` headings and `status: canon` metadata.
fn canon_counts(text: &str) -> (usize, usize) {
    let records = text.lines().filter(|l| l.starts_with("## ")).count();
    let canon = text
        .lines()
        .filter(|l| l.split('|').any(|part| part.trim() == "status: canon"))
        .count();
    (records, canon)
}

fn mcp_server_names(repo: &Path) -> Option<Vec<String>> {
    let text = read_small(&repo.join(".mcp.json"))?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let servers = v.get("mcpServers")?.as_object()?;
    Some(servers.keys().cloned().collect())
}

/// Facts and findings for one repository folder.
pub fn inspect(repo: &Path) -> ProjectSetup {
    let mut instructions = Vec::new();
    for name in INSTRUCTION_FILES {
        if let Some(file) = fact(repo, name) {
            let references = read_small(&repo.join(name))
                .map(|t| references(&t))
                .unwrap_or_default();
            instructions.push(InstructionFile { file, references });
        }
    }
    let canon = fact(repo, ".claude/canon.md").map(|file| {
        let (records, canon) = read_small(&repo.join(".claude/canon.md"))
            .map(|t| canon_counts(&t))
            .unwrap_or((0, 0));
        CanonFacts {
            file,
            records,
            canon,
        }
    });
    let mut findings = Vec::new();
    // setup §4: referenced instruction file missing.
    for f in &instructions {
        let base = repo.join(&f.file.path);
        let dir = base.parent().unwrap_or(repo);
        for r in &f.references {
            let target = if let Some(rest) = r.strip_prefix("~/") {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(rest))
            } else {
                Some(dir.join(r))
            };
            if target.is_some_and(|t| !t.exists()) {
                findings.push(SetupFinding {
                    rule: "setup.reference_missing@1",
                    severity: "warning",
                    message: format!("{} references {r}, which does not exist", f.file.path),
                });
            }
        }
    }
    // setup §4: agents may receive different instructions.
    let names: BTreeSet<&str> = instructions.iter().map(|f| f.file.path.as_str()).collect();
    match (names.contains("CLAUDE.md"), names.contains("AGENTS.md")) {
        (true, true) => {
            let refers = |from: &str, to: &str| {
                instructions
                    .iter()
                    .find(|f| f.file.path == from)
                    .is_some_and(|f| {
                        f.references
                            .iter()
                            .any(|r| r.trim_start_matches("./") == to)
                    })
            };
            if !refers("CLAUDE.md", "AGENTS.md") && !refers("AGENTS.md", "CLAUDE.md") {
                findings.push(SetupFinding {
                    rule: "setup.instructions_unlinked@1",
                    severity: "warning",
                    message: "CLAUDE.md and AGENTS.md both exist and neither references the other: agents may receive different instructions".into(),
                });
            }
        }
        (true, false) | (false, true) => findings.push(SetupFinding {
            rule: "setup.single_instruction_file@1",
            severity: "info",
            message: format!(
                "Only {} exists: agents that read the other file get no project instructions",
                names.iter().next().copied().unwrap_or_default()
            ),
        }),
        (false, false) => findings.push(SetupFinding {
            rule: "setup.no_instructions@1",
            severity: "info",
            message: "No CLAUDE.md or AGENTS.md at the repository root".into(),
        }),
    }
    ProjectSetup {
        path: repo.to_string_lossy().into_owned(),
        instructions,
        canon,
        mcp_servers: mcp_server_names(repo),
        findings,
    }
}

/// Plans in the approved Claude folders: count, sizes and times only.
/// Plan file names are made from prompt words, so they are not shown.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PlanFact {
    pub bytes: u64,
    pub modified: Option<i64>,
}

pub fn plans(roots: &[PathBuf]) -> Vec<PlanFact> {
    let mut out = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root.join("plans")) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "md")
                && let Ok(meta) = e.metadata()
                && meta.is_file()
            {
                out.push(PlanFact {
                    bytes: meta.len(),
                    modified: meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64),
                });
            }
        }
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.modified));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_path_imports_only() {
        let text = "See @docs/guide.md and @AGENTS.md.\nPing @dependabot or mail a@example.com.\n\
                    `@not/in-code.md`\n```\n@not/in/fence.md\n```\n@~/.claude/rules.md";
        assert_eq!(
            references(text),
            ["docs/guide.md", "AGENTS.md", "~/.claude/rules.md"]
        );
    }

    #[test]
    fn findings_name_missing_references_and_unlinked_files() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path();
        fs::write(repo.join("CLAUDE.md"), "Read @docs/missing.md first.").unwrap();
        fs::write(repo.join("AGENTS.md"), "Rules here.").unwrap();
        fs::create_dir_all(repo.join(".claude")).unwrap();
        fs::write(
            repo.join(".claude/canon.md"),
            "# Canon\n\n## A\nkind: choice | date: 2026-10-01 | status: canon\nx\n\n## B\nkind: gotcha | date: 2026-10-02 | status: non-canon, superseded by: A\ny\n",
        )
        .unwrap();
        fs::write(
            repo.join(".mcp.json"),
            r#"{"mcpServers":{"huly":{"command":"npx","args":["secret-token"],"env":{"K":"v"}}}}"#,
        )
        .unwrap();
        let s = inspect(repo);
        let rules: Vec<&str> = s.findings.iter().map(|f| f.rule).collect();
        assert_eq!(
            rules,
            ["setup.reference_missing@1", "setup.instructions_unlinked@1"]
        );
        let canon = s.canon.unwrap();
        assert_eq!((canon.records, canon.canon), (2, 1));
        assert_eq!(s.mcp_servers.unwrap(), ["huly"]);
        // Nothing of the server's command, args or env leaves the module.
        let json = serde_json::to_string(&s.instructions).unwrap();
        assert!(!json.contains("secret-token") && !json.contains("Rules here"));

        // Linking the files clears the warning.
        fs::write(repo.join("CLAUDE.md"), "Read @AGENTS.md first.").unwrap();
        let rules: Vec<&str> = inspect(repo).findings.iter().map(|f| f.rule).collect();
        assert!(rules.is_empty(), "{rules:?}");
    }

    #[test]
    fn plans_show_no_names() {
        let dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("plans")).unwrap();
        fs::write(dir.path().join("plans/words-from-a-prompt.md"), "plan").unwrap();
        let p = plans(&[dir.path().to_path_buf()]);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].bytes, 4);
        assert!(!serde_json::to_string(&p).unwrap().contains("words"));
    }
}
