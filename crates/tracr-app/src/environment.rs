//! Environment (T-0016): how Claude Code is set up, read from the approved
//! Claude folder, and a check of that setup.
//!
//! Read only, on demand, never stored. Only names, a few allowlisted
//! frontmatter keys, sizes and times leave this module: no file body, no env
//! value, no hook command line, no MCP argument and no URL path. `*.key`
//! files and the sessions folder are never touched.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use serde_json::Value;

/// Frontmatter keys that may be shown. Everything else is dropped.
const FRONTMATTER_KEYS: [&str; 6] = [
    "name",
    "description",
    "model",
    "tools",
    "allowed-tools",
    "argument-hint",
];
const LARGE_COMMAND_BYTES: u64 = 20 * 1024;
const LARGE_MEMORY_CHARS: usize = 40_000;
const MEMORY_INDEX_LINES: usize = 200;
const LONG_DESCRIPTION: usize = 1024;
const MANY_BACKUPS: usize = 3;

#[derive(Debug, Clone, Serialize)]
pub struct Environment {
    /// The approved Claude folders this was read from.
    pub roots: Vec<String>,
    pub skills: Vec<Item>,
    pub commands: Vec<Item>,
    pub agents: Vec<Item>,
    pub plugins: Vec<Plugin>,
    pub mcp_servers: Vec<McpServer>,
    pub hooks: Vec<Hook>,
    pub memory: Vec<MemoryFile>,
    pub permissions: Vec<Permission>,
    pub findings: Vec<Finding>,
    /// 100 minus 10 per error and 3 per warning, never below 0.
    pub score: u32,
}

/// Where an item comes from: your own folder, or a plugin (its id).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "plugin", rename_all = "snake_case")]
pub enum Origin {
    User,
    Plugin(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub name: String,
    pub description: Option<String>,
    /// Allowlisted frontmatter keys only.
    pub fields: BTreeMap<String, String>,
    pub origin: Origin,
    /// False for items of a plugin that is not switched on.
    pub active: bool,
    pub path: String,
    pub bytes: u64,
    pub lines: usize,
    pub modified_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plugin {
    /// `name@marketplace`.
    pub id: String,
    pub name: String,
    pub marketplace: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub enabled: bool,
    pub installed: bool,
    pub path: Option<String>,
    pub skills: usize,
    pub commands: usize,
    pub agents: usize,
    pub hooks: usize,
    pub mcp_servers: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServer {
    pub name: String,
    pub origin: Origin,
    pub active: bool,
    /// `stdio`, `http`, `sse` or what the file states.
    pub transport: String,
    /// The program's file name (stdio) or the URL's host (http, sse); never arguments or paths.
    pub target: Option<String>,
    /// Names of the environment variables it sets; never their values.
    pub env_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hook {
    pub event: String,
    pub matcher: Option<String>,
    /// The program's file name; the command line itself is never shown.
    pub program: Option<String>,
    pub timeout_s: Option<f64>,
    pub origin: Origin,
    pub active: bool,
    /// The settings or hooks file it is defined in.
    pub file: String,
    /// The program is an absolute path that does not exist.
    pub missing: bool,
    #[serde(skip)]
    command: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// `CLAUDE.md` in the Claude folder: loaded in every session.
    Global,
    /// `rules/*.md`.
    Rule,
    /// `projects/<project>/memory/MEMORY.md`, the auto-memory index.
    AutoMemory,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryFile {
    pub kind: MemoryKind,
    /// The file name, or for auto-memory the project folder's name.
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub lines: usize,
    pub chars: usize,
    /// Topic files next to an auto-memory index.
    pub topics: usize,
    pub modified_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Permission {
    /// `allow`, `ask` or `deny`.
    pub list: String,
    pub rule: String,
    pub file: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// e.g. `SKL-01`; the window holds the rule's title and advice.
    pub rule: String,
    pub severity: Severity,
    /// What it is about: a name, a plugin id, a rule.
    pub subject: String,
    pub file: Option<String>,
    /// A short fact for the text, e.g. a size or a count.
    pub detail: Option<String>,
}

/// Reads every approved Claude folder. Missing folders yield nothing.
pub fn read(roots: &[PathBuf]) -> Environment {
    let mut env = Environment {
        roots: roots.iter().map(|r| r.display().to_string()).collect(),
        skills: vec![],
        commands: vec![],
        agents: vec![],
        plugins: vec![],
        mcp_servers: vec![],
        hooks: vec![],
        memory: vec![],
        permissions: vec![],
        findings: vec![],
        score: 100,
    };
    for root in roots.iter().filter(|r| r.is_dir()) {
        read_root(root, &mut env);
    }
    check(&mut env);
    env
}

fn read_root(root: &Path, env: &mut Environment) {
    env.skills
        .extend(skills(&root.join("skills"), &Origin::User, true));
    env.commands
        .extend(markdown_items(&root.join("commands"), &Origin::User, true));
    env.agents
        .extend(markdown_items(&root.join("agents"), &Origin::User, true));

    let mut enabled: BTreeMap<String, bool> = BTreeMap::new();
    for name in ["settings.json", "settings.local.json"] {
        let path = root.join(name);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(settings) = serde_json::from_str::<Value>(&text) else {
            env.findings
                .push(finding("CFG-01", Severity::Error, name, Some(&path), None));
            continue;
        };
        if let Some(map) = settings.get("enabledPlugins").and_then(Value::as_object) {
            for (id, on) in map {
                enabled.insert(id.clone(), on.as_bool().unwrap_or(false));
            }
        }
        env.hooks
            .extend(hooks(&settings, &path, &Origin::User, true, None));
        if let Some(p) = settings.get("permissions") {
            for list in ["allow", "ask", "deny"] {
                for rule in p.get(list).and_then(Value::as_array).into_iter().flatten() {
                    if let Some(rule) = rule.as_str() {
                        env.permissions.push(Permission {
                            list: list.to_owned(),
                            rule: rule.to_owned(),
                            file: path.display().to_string(),
                        });
                    }
                }
            }
        }
    }
    let backups = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("settings.json.") && n.ends_with(".bak")
        })
        .count();
    if backups > MANY_BACKUPS {
        env.findings.push(finding(
            "CFG-03",
            Severity::Info,
            "settings.json",
            Some(&root.join("settings.json")),
            Some(backups.to_string()),
        ));
    }

    read_plugins(root, &enabled, env);
    read_memory(root, env);
}

fn read_plugins(root: &Path, enabled: &BTreeMap<String, bool>, env: &mut Environment) {
    let index = root.join("plugins").join("installed_plugins.json");
    let installed: BTreeMap<String, PathBuf> = fs::read_to_string(&index)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("plugins").cloned())
        .and_then(|p| p.as_object().cloned())
        .map(|plugins| {
            plugins
                .into_iter()
                .filter_map(|(id, entry)| {
                    let first = match &entry {
                        Value::Array(a) => a.first().cloned()?,
                        other => other.clone(),
                    };
                    let path = first.get("installPath")?.as_str()?.to_owned();
                    Some((id, PathBuf::from(path)))
                })
                .collect()
        })
        .unwrap_or_default();
    let versions: BTreeMap<String, String> = fs::read_to_string(&index)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("plugins").and_then(Value::as_object).cloned())
        .map(|plugins| {
            plugins
                .into_iter()
                .filter_map(|(id, entry)| {
                    let first = match &entry {
                        Value::Array(a) => a.first().cloned()?,
                        other => other.clone(),
                    };
                    Some((id, first.get("version")?.as_str()?.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();

    let ids: BTreeSet<&String> = installed.keys().chain(enabled.keys()).collect();
    for id in ids {
        let (name, marketplace) = id.split_once('@').unwrap_or((id.as_str(), ""));
        let on = enabled.get(id).copied().unwrap_or(false);
        let origin = Origin::Plugin(id.clone());
        let path = installed.get(id);
        let exists = path.is_some_and(|p| p.is_dir());
        let mut plugin = Plugin {
            id: id.clone(),
            name: name.to_owned(),
            marketplace: marketplace.to_owned(),
            version: versions.get(id).cloned(),
            description: None,
            enabled: on,
            installed: path.is_some(),
            path: path.map(|p| p.display().to_string()),
            skills: 0,
            commands: 0,
            agents: 0,
            hooks: 0,
            mcp_servers: 0,
        };
        match path {
            None if on => env
                .findings
                .push(finding("PLG-01", Severity::Error, id, None, None)),
            Some(p) if !exists => {
                env.findings
                    .push(finding("PLG-02", Severity::Error, id, Some(p), None))
            }
            Some(p) if !on => {
                env.findings
                    .push(finding("PLG-03", Severity::Info, id, Some(p), None))
            }
            _ => {}
        }
        if let Some(dir) = path.filter(|p| p.is_dir()) {
            plugin.description = fs::read_to_string(dir.join(".claude-plugin").join("plugin.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v.get("description")?.as_str().map(str::to_owned));
            let skills = skills(&dir.join("skills"), &origin, on);
            let commands = markdown_items(&dir.join("commands"), &origin, on);
            let agents = markdown_items(&dir.join("agents"), &origin, on);
            let hooks_file = dir.join("hooks").join("hooks.json");
            let hooks = fs::read_to_string(&hooks_file)
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .map(|v| self::hooks(&v, &hooks_file, &origin, on, Some(dir)))
                .unwrap_or_default();
            let servers = mcp_servers(dir, &origin, on);
            plugin.skills = skills.len();
            plugin.commands = commands.len();
            plugin.agents = agents.len();
            plugin.hooks = hooks.len();
            plugin.mcp_servers = servers.len();
            env.skills.extend(skills);
            env.commands.extend(commands);
            env.agents.extend(agents);
            env.hooks.extend(hooks);
            env.mcp_servers.extend(servers);
        }
        env.plugins.push(plugin);
    }
}

fn read_memory(root: &Path, env: &mut Environment) {
    let global = root.join("CLAUDE.md");
    if let Some(m) = memory_file(&global, MemoryKind::Global, "CLAUDE.md".into(), 0) {
        env.memory.push(m);
    }
    for path in md_files(&root.join("rules")) {
        let name = file_name(&path);
        if let Some(m) = memory_file(&path, MemoryKind::Rule, name, 0) {
            env.memory.push(m);
        }
    }
    for project in dirs(&root.join("projects")) {
        let dir = project.join("memory");
        let index = dir.join("MEMORY.md");
        let topics = md_files(&dir).iter().filter(|p| **p != index).count();
        if let Some(m) = memory_file(&index, MemoryKind::AutoMemory, file_name(&project), topics) {
            env.memory.push(m);
        }
    }
}

fn memory_file(path: &Path, kind: MemoryKind, name: String, topics: usize) -> Option<MemoryFile> {
    let text = fs::read_to_string(path).ok()?;
    Some(MemoryFile {
        kind,
        name,
        path: path.display().to_string(),
        bytes: text.len() as u64,
        lines: text.lines().count(),
        chars: text.chars().count(),
        topics,
        modified_ms: modified(path),
    })
}

fn skills(dir: &Path, origin: &Origin, active: bool) -> Vec<Item> {
    // A folder without SKILL.md (shared helpers, sync state) is not a skill.
    dirs(dir)
        .into_iter()
        .filter(|d| d.join("SKILL.md").is_file())
        .map(|d| {
            let path = d.join("SKILL.md");
            let mut item = item(&path, origin, active).unwrap_or_else(|| Item {
                name: file_name(&d),
                description: None,
                fields: BTreeMap::new(),
                origin: origin.clone(),
                active,
                path: path.display().to_string(),
                bytes: 0,
                lines: 0,
                modified_ms: None,
            });
            if !item.fields.contains_key("name") {
                item.name = file_name(&d);
            }
            item
        })
        .collect()
}

fn markdown_items(dir: &Path, origin: &Origin, active: bool) -> Vec<Item> {
    md_files(dir)
        .iter()
        .filter_map(|p| item(p, origin, active))
        .collect()
}

fn item(path: &Path, origin: &Origin, active: bool) -> Option<Item> {
    let text = fs::read_to_string(path).ok()?;
    let fields = frontmatter(&text);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(Item {
        name: fields.get("name").cloned().unwrap_or(stem),
        description: fields.get("description").cloned(),
        fields,
        origin: origin.clone(),
        active,
        path: path.display().to_string(),
        bytes: text.len() as u64,
        lines: text.lines().count(),
        modified_ms: modified(path),
    })
}

/// Allowlisted top-level `key: value` pairs of a `---` block at the start.
/// Quoted values are unquoted; block scalars and lists are joined into one line.
pub fn frontmatter(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return out;
    }
    let block: Vec<&str> = lines.take_while(|l| l.trim_end() != "---").collect();
    let mut i = 0;
    while i < block.len() {
        let line = block[i];
        i += 1;
        if line.starts_with([' ', '\t', '#']) || line.trim().is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let mut parts: Vec<String> = vec![];
        while i < block.len() && (block[i].starts_with([' ', '\t']) || block[i].trim().is_empty()) {
            let l = block[i].trim();
            if !l.is_empty() {
                parts.push(l.trim_start_matches("- ").to_owned());
            }
            i += 1;
        }
        if !FRONTMATTER_KEYS.contains(&key) {
            continue;
        }
        let value = if matches!(value, "" | ">" | "|" | ">-" | "|-" | ">+" | "|+") {
            let sep = if value.is_empty() { ", " } else { " " };
            parts.join(sep)
        } else {
            unquote(value)
        };
        if !value.is_empty() {
            out.insert(key.to_owned(), value);
        }
    }
    out
}

fn unquote(v: &str) -> String {
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            let inner = &v[1..v.len() - 1];
            return if q == '"' {
                inner.replace("\\\"", "\"").replace("\\\\", "\\")
            } else {
                inner.replace("''", "'")
            };
        }
    }
    v.to_owned()
}

fn hooks(
    settings: &Value,
    file: &Path,
    origin: &Origin,
    active: bool,
    plugin_root: Option<&Path>,
) -> Vec<Hook> {
    let mut out = vec![];
    let Some(events) = settings.get("hooks").and_then(Value::as_object) else {
        return out;
    };
    for (event, groups) in events {
        for group in groups.as_array().into_iter().flatten() {
            let matcher = group
                .get("matcher")
                .and_then(Value::as_str)
                .filter(|m| !m.is_empty())
                .map(str::to_owned);
            for h in group
                .get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let command = h
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let program = program_of(&command, plugin_root);
                out.push(Hook {
                    event: event.clone(),
                    matcher: matcher.clone(),
                    program: program.as_ref().map(|(name, _)| name.clone()),
                    timeout_s: h.get("timeout").and_then(Value::as_f64),
                    origin: origin.clone(),
                    active,
                    file: file.display().to_string(),
                    missing: program
                        .as_ref()
                        .and_then(|(_, p)| p.as_ref())
                        .is_some_and(|p| !p.exists()),
                    command,
                });
            }
        }
    }
    out
}

/// The program of a shell command: its file name, and its path when that is
/// absolute (after `${CLAUDE_PLUGIN_ROOT}` and `~`). Leading `VAR=value` words are skipped.
fn program_of(command: &str, plugin_root: Option<&Path>) -> Option<(String, Option<PathBuf>)> {
    let mut rest = command.trim_start();
    loop {
        let (word, after) = first_word(rest)?;
        let is_assignment = word.split_once('=').is_some_and(|(k, _)| {
            !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
        if !is_assignment {
            let mut word = word;
            if let Some(root) = plugin_root {
                let r = root.display().to_string();
                word = word
                    .replace("${CLAUDE_PLUGIN_ROOT}", &r)
                    .replace("$CLAUDE_PLUGIN_ROOT", &r);
            }
            if let Some(stripped) = word.strip_prefix("~/")
                && let Some(home) = std::env::var_os("HOME")
            {
                word = PathBuf::from(home).join(stripped).display().to_string();
            }
            let path = PathBuf::from(&word);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| word.clone());
            let absolute = (path.is_absolute() && !word.contains('$')).then_some(path);
            return Some((name, absolute));
        }
        rest = after.trim_start();
    }
}

fn first_word(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    let mut word = String::new();
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, c) if c.is_whitespace() => return Some((word, &s[i..])),
            _ => word.push(c),
        }
    }
    Some((word, ""))
}

fn mcp_servers(plugin_dir: &Path, origin: &Origin, active: bool) -> Vec<McpServer> {
    let mut servers = BTreeMap::new();
    let from_file = fs::read_to_string(plugin_dir.join(".mcp.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let from_manifest = fs::read_to_string(plugin_dir.join(".claude-plugin").join("plugin.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("mcpServers").cloned());
    for source in [
        from_file.and_then(|v| v.get("mcpServers").cloned()),
        from_manifest,
    ]
    .into_iter()
    .flatten()
    {
        for (name, def) in source.as_object().into_iter().flatten() {
            servers.insert(name.clone(), mcp_server(name, def, origin, active));
        }
    }
    servers.into_values().collect()
}

fn mcp_server(name: &str, def: &Value, origin: &Origin, active: bool) -> McpServer {
    let url = def.get("url").and_then(Value::as_str);
    let transport = def
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| if url.is_some() { "http" } else { "stdio" }.to_owned());
    let target = match url {
        Some(u) => host_of(u),
        None => def
            .get("command")
            .and_then(Value::as_str)
            .and_then(|c| program_of(c, None))
            .map(|(n, _)| n),
    };
    McpServer {
        name: name.to_owned(),
        origin: origin.clone(),
        active,
        transport,
        target,
        env_keys: def
            .get("env")
            .and_then(Value::as_object)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default(),
    }
}

/// `https://user:pw@mcp.example.com:8443/path?q` → `mcp.example.com`.
fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.split(':').next()?;
    (!host.is_empty()).then(|| host.to_owned())
}

fn check(env: &mut Environment) {
    let mut f = std::mem::take(&mut env.findings);
    for s in &env.skills {
        let file = Some(PathBuf::from(&s.path));
        if s.description.is_none() {
            f.push(finding(
                "SKL-01",
                Severity::Error,
                &s.name,
                file.as_deref(),
                None,
            ));
        }
        let folder = Path::new(&s.path)
            .parent()
            .map(file_name)
            .unwrap_or_default();
        if s.fields.get("name").is_some_and(|n| *n != folder) {
            f.push(finding(
                "SKL-02",
                Severity::Info,
                &s.name,
                file.as_deref(),
                Some(folder),
            ));
        }
        if let Some(d) = &s.description
            && d.chars().count() > LONG_DESCRIPTION
        {
            f.push(finding(
                "SKL-03",
                Severity::Warning,
                &s.name,
                file.as_deref(),
                Some(d.chars().count().to_string()),
            ));
        }
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for s in env.skills.iter().filter(|s| s.active) {
        *seen.entry(s.name.as_str()).or_default() += 1;
    }
    for (name, n) in seen.into_iter().filter(|(_, n)| *n > 1) {
        f.push(finding(
            "SKL-04",
            Severity::Warning,
            name,
            None,
            Some(n.to_string()),
        ));
    }
    for c in &env.commands {
        if c.bytes > LARGE_COMMAND_BYTES {
            f.push(finding(
                "CMD-01",
                Severity::Warning,
                &c.name,
                Some(Path::new(&c.path)),
                Some(c.bytes.to_string()),
            ));
        }
    }
    for a in &env.agents {
        if a.description.is_none() || !a.fields.contains_key("name") {
            f.push(finding(
                "AGT-01",
                Severity::Error,
                &a.name,
                Some(Path::new(&a.path)),
                None,
            ));
        }
    }
    let mut same: BTreeMap<(String, Option<String>, String), usize> = BTreeMap::new();
    for h in env.hooks.iter().filter(|h| h.active) {
        if h.missing {
            f.push(finding(
                "HOOK-01",
                Severity::Warning,
                &format!("{} · {}", h.event, h.program.clone().unwrap_or_default()),
                Some(Path::new(&h.file)),
                None,
            ));
        }
        if !h.command.trim().is_empty() {
            *same
                .entry((h.event.clone(), h.matcher.clone(), h.command.clone()))
                .or_default() += 1;
        }
    }
    for ((event, matcher, command), n) in same.into_iter().filter(|(_, n)| *n > 1) {
        let program = program_of(&command, None)
            .map(|(p, _)| p)
            .unwrap_or_default();
        let subject = match matcher {
            Some(m) => format!("{event} ({m}) · {program}"),
            None => format!("{event} · {program}"),
        };
        f.push(finding(
            "HOOK-02",
            Severity::Warning,
            &subject,
            None,
            Some(n.to_string()),
        ));
    }
    let mut servers: BTreeMap<&str, usize> = BTreeMap::new();
    for s in env.mcp_servers.iter().filter(|s| s.active) {
        *servers.entry(s.name.as_str()).or_default() += 1;
    }
    for (name, n) in servers.into_iter().filter(|(_, n)| *n > 1) {
        f.push(finding(
            "MCP-01",
            Severity::Warning,
            name,
            None,
            Some(n.to_string()),
        ));
    }
    let lists: BTreeMap<&str, BTreeSet<&str>> =
        env.permissions.iter().fold(BTreeMap::new(), |mut m, p| {
            m.entry(p.list.as_str())
                .or_default()
                .insert(p.rule.as_str());
            m
        });
    let allow = lists.get("allow").cloned().unwrap_or_default();
    for other in ["ask", "deny"] {
        for rule in lists.get(other).into_iter().flatten() {
            if allow.contains(rule) {
                f.push(finding(
                    "CFG-02",
                    Severity::Warning,
                    rule,
                    None,
                    Some(other.to_owned()),
                ));
            }
        }
    }
    for m in &env.memory {
        match m.kind {
            MemoryKind::Global | MemoryKind::Rule if m.chars > LARGE_MEMORY_CHARS => {
                f.push(finding(
                    "MEM-01",
                    Severity::Warning,
                    &m.name,
                    Some(Path::new(&m.path)),
                    Some(m.chars.to_string()),
                ))
            }
            MemoryKind::AutoMemory if m.lines > MEMORY_INDEX_LINES => f.push(finding(
                "MEM-02",
                Severity::Warning,
                &m.name,
                Some(Path::new(&m.path)),
                Some(m.lines.to_string()),
            )),
            _ => {}
        }
    }
    f.sort_by(|a, b| (a.severity, &a.rule, &a.subject).cmp(&(b.severity, &b.rule, &b.subject)));
    let errors = f.iter().filter(|x| x.severity == Severity::Error).count() as u32;
    let warnings = f.iter().filter(|x| x.severity == Severity::Warning).count() as u32;
    env.score = 100u32.saturating_sub(errors * 10 + warnings * 3);
    env.findings = f;
}

fn finding(
    rule: &str,
    severity: Severity,
    subject: &str,
    file: Option<&Path>,
    detail: Option<String>,
) -> Finding {
    Finding {
        rule: rule.to_owned(),
        severity,
        subject: subject.to_owned(),
        file: file.map(|p| p.display().to_string()),
        detail,
    }
}

fn dirs(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !file_name(p).starts_with('.'))
        .collect();
    out.sort();
    out
}

fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
        .collect();
    out.sort();
    out
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn modified(p: &Path) -> Option<i64> {
    fs::metadata(p)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_keeps_allowlisted_keys_and_joins_blocks_and_lists() {
        let text = "---\nname: \"clean-code\"\ndescription: >\n  First line\n  second line\nsecret: x\ntools:\n  - Read\n  - Grep\nmodel: 'son''net'\n---\n# Body\nname: not frontmatter\n";
        let f = frontmatter(text);
        assert_eq!(f.get("name").map(String::as_str), Some("clean-code"));
        assert_eq!(
            f.get("description").map(String::as_str),
            Some("First line second line")
        );
        assert_eq!(f.get("tools").map(String::as_str), Some("Read, Grep"));
        assert_eq!(f.get("model").map(String::as_str), Some("son'net"));
        assert!(!f.contains_key("secret"), "only allowlisted keys");
        assert!(frontmatter("# no frontmatter\nname: x").is_empty());
    }

    #[test]
    fn programs_and_hosts_never_carry_arguments_or_paths() {
        let (name, path) =
            program_of("FOO=bar \"/opt/my tools/notify.sh\" --token abc", None).unwrap();
        assert_eq!(name, "notify.sh");
        assert_eq!(path, Some(PathBuf::from("/opt/my tools/notify.sh")));
        let (name, path) =
            program_of("${CLAUDE_PLUGIN_ROOT}/scripts/x.sh", Some(Path::new("/p"))).unwrap();
        assert_eq!(
            (name.as_str(), path),
            ("x.sh", Some(PathBuf::from("/p/scripts/x.sh")))
        );
        assert_eq!(
            program_of("npx -y server", None).unwrap(),
            ("npx".to_owned(), None)
        );
        assert_eq!(
            host_of("https://user:pw@mcp.example.com:8443/v1?key=abc").as_deref(),
            Some("mcp.example.com")
        );
    }
}
