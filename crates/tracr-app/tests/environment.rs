//! Environment (T-0016): inventory and check of an approved Claude folder.

use std::fs;
use std::path::Path;

use serde_json::json;
use tempfile::TempDir;
use tracr_app::environment::{self, Severity};

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A Claude folder with one of everything, a few mistakes and several secrets.
fn claude_home(dir: &Path) -> std::path::PathBuf {
    let home = dir.join(".claude");
    let plugin = dir.join("cache").join("tools");
    write(
        &home.join("settings.json"),
        &json!({
            "env": {"API_TOKEN": "SECRET-ENV-VALUE"},
            "enabledPlugins": {"tools@market": true, "ghost@market": true},
            "permissions": {"allow": ["Bash(git push *)", "Read"], "deny": ["Bash(git push *)"]},
            "hooks": {
                "Stop": [{"hooks": [
                    {"type": "command", "command": "TOKEN=SECRET-HOOK-ARG /nowhere/notify.sh --key SECRET-HOOK-ARG", "timeout": 5},
                    {"type": "command", "command": "TOKEN=SECRET-HOOK-ARG /nowhere/notify.sh --key SECRET-HOOK-ARG"}
                ]}]
            }
        })
        .to_string(),
    );
    write(&home.join("settings.local.json"), "{ not json");
    for i in 0..4 {
        write(&home.join(format!("settings.json.{i}.bak")), "{}");
    }
    write(
        &home.join("skills/review/SKILL.md"),
        "---\nname: review\ndescription: Reviews a diff.\n---\nSECRET-BODY\n",
    );
    write(
        &home.join("skills/renamed/SKILL.md"),
        "---\nname: other\ndescription: Name differs from the folder.\n---\n",
    );
    write(&home.join("skills/bare/SKILL.md"), "no frontmatter\n");
    write(&home.join("skills/_shared/helpers.md"), "not a skill\n");
    write(&home.join("commands/big.md"), &"x".repeat(25 * 1024));
    write(
        &home.join("agents/helper.md"),
        "---\ndescription: no name\n---\n",
    );
    write(&home.join("CLAUDE.md"), "be kind\n");
    write(
        &home.join("projects/-x-app/memory/MEMORY.md"),
        &"- line\n".repeat(210),
    );
    write(&home.join("projects/-x-app/memory/topic.md"), "t\n");
    write(&home.join("sessions/1.deadbeef.key"), "SECRET-KEY");

    write(
        &home.join("plugins/installed_plugins.json"),
        &json!({"version": 2, "plugins": {
            "tools@market": [{"scope": "user", "installPath": plugin.display().to_string(), "version": "1.2.0"}],
            "idle@market": [{"scope": "user", "installPath": dir.join("missing").display().to_string(), "version": "0.1.0"}]
        }})
        .to_string(),
    );
    write(
        &plugin.join(".claude-plugin/plugin.json"),
        &json!({"name": "tools", "description": "Handy tools"}).to_string(),
    );
    write(
        &plugin.join("skills/review/SKILL.md"),
        "---\nname: review\ndescription: The plugin's review.\n---\n",
    );
    write(
        &plugin.join(".mcp.json"),
        &json!({"mcpServers": {
            "remote": {"type": "http", "url": "https://user:SECRET-PW@mcp.example.com/v1?key=SECRET-QUERY"},
            "local": {"command": "/usr/local/bin/srv", "args": ["--token", "SECRET-ARG"], "env": {"SRV_KEY": "SECRET-MCP-ENV"}}
        }})
        .to_string(),
    );
    write(
        &plugin.join("hooks/hooks.json"),
        &json!({"hooks": {"PostToolUse": [{"matcher": "Edit", "hooks": [
            {"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/scripts/fmt.sh"}
        ]}]}})
        .to_string(),
    );
    write(&plugin.join("scripts/fmt.sh"), "#!/bin/sh\n");
    home
}

#[test]
fn the_inventory_lists_what_is_set_up_and_no_secret_leaves_it() {
    let dir = TempDir::new().unwrap();
    let home = claude_home(dir.path());
    let env = environment::read(&[home]);

    let names = |items: &[environment::Item]| -> Vec<String> {
        items.iter().map(|i| i.name.clone()).collect()
    };
    assert_eq!(names(&env.skills), ["bare", "other", "review", "review"]);
    assert_eq!(names(&env.commands), ["big"]);
    assert_eq!(env.plugins.len(), 3, "installed, enabled-only and idle");
    let tools = env.plugins.iter().find(|p| p.id == "tools@market").unwrap();
    assert!(tools.enabled && tools.installed);
    assert_eq!(
        (tools.skills, tools.hooks, tools.mcp_servers),
        (1, 1, 2),
        "the plugin's components"
    );
    assert_eq!(tools.description.as_deref(), Some("Handy tools"));

    let remote = env.mcp_servers.iter().find(|s| s.name == "remote").unwrap();
    assert_eq!(
        (remote.transport.as_str(), remote.target.as_deref()),
        ("http", Some("mcp.example.com"))
    );
    let local = env.mcp_servers.iter().find(|s| s.name == "local").unwrap();
    assert_eq!(local.target.as_deref(), Some("srv"));
    assert_eq!(local.env_keys, ["SRV_KEY"]);

    let stop: Vec<_> = env.hooks.iter().filter(|h| h.event == "Stop").collect();
    assert_eq!(stop.len(), 2);
    assert_eq!(stop[0].program.as_deref(), Some("notify.sh"));
    assert!(stop[0].missing);
    let fmt = env.hooks.iter().find(|h| h.event == "PostToolUse").unwrap();
    assert!(!fmt.missing, "the plugin root is resolved");
    assert_eq!(fmt.matcher.as_deref(), Some("Edit"));

    let auto = env.memory.iter().find(|m| m.name == "-x-app").unwrap();
    assert_eq!((auto.lines, auto.topics), (210, 1));
    assert_eq!(env.permissions.len(), 3);

    let text = serde_json::to_string(&env).unwrap();
    for secret in [
        "SECRET-ENV-VALUE",
        "SECRET-HOOK-ARG",
        "SECRET-BODY",
        "SECRET-KEY",
        "SECRET-PW",
        "SECRET-QUERY",
        "SECRET-ARG",
        "SECRET-MCP-ENV",
    ] {
        assert!(!text.contains(secret), "{secret} leaked");
    }
}

#[test]
fn the_check_finds_each_planted_mistake_and_scores_them() {
    let dir = TempDir::new().unwrap();
    let home = claude_home(dir.path());
    let env = environment::read(&[home]);
    let rules: Vec<(&str, Severity)> = env
        .findings
        .iter()
        .map(|f| (f.rule.as_str(), f.severity))
        .collect();
    for (rule, sev) in [
        ("SKL-01", Severity::Error),
        ("AGT-01", Severity::Error),
        ("PLG-01", Severity::Error),
        ("PLG-02", Severity::Error),
        ("CFG-01", Severity::Error),
        ("SKL-02", Severity::Info),
        ("SKL-04", Severity::Warning),
        ("CMD-01", Severity::Warning),
        ("HOOK-01", Severity::Warning),
        ("HOOK-02", Severity::Warning),
        ("CFG-02", Severity::Warning),
        ("MEM-02", Severity::Warning),
        ("CFG-03", Severity::Info),
    ] {
        assert!(rules.contains(&(rule, sev)), "{rule} missing in {rules:?}");
    }
    assert!(
        env.findings
            .windows(2)
            .all(|w| w[0].severity <= w[1].severity),
        "errors first"
    );
    let errors = rules.iter().filter(|(_, s)| *s == Severity::Error).count() as u32;
    let warnings = rules
        .iter()
        .filter(|(_, s)| *s == Severity::Warning)
        .count() as u32;
    assert_eq!(env.score, 100u32.saturating_sub(errors * 10 + warnings * 3));
}

#[test]
fn nothing_is_read_without_an_approved_folder() {
    let env = environment::read(&[]);
    assert!(env.skills.is_empty() && env.findings.is_empty());
    assert_eq!(env.score, 100);
    let env = environment::read(&[Path::new("/does/not/exist").to_path_buf()]);
    assert!(env.plugins.is_empty());
}
