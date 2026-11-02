# Scope

**In (phase 1, inventory):** from the approved Claude folder only
- skills (`skills/*/SKILL.md`), commands (`commands/*.md`), agents (`agents/*.md`): name, description and a few allowlisted frontmatter keys, size, lines, modified time
- plugins (`plugins/installed_plugins.json`, `settings.json` `enabledPlugins`, each plugin's `.claude-plugin/plugin.json`): enabled, version, marketplace, and the skills, commands, agents, hooks and MCP servers it brings
- MCP servers that plugins bring (`.mcp.json`): name, transport, program name or URL host, env key names
- hooks from `settings.json`, `settings.local.json` and plugins (`hooks/hooks.json`): event, matcher, program name, timeout
- memory: `CLAUDE.md`, `rules/*.md`, auto-memory indexes `projects/*/memory/MEMORY.md`: size and lines
- permissions: the allow, ask and deny rules from both settings files

**In (phase 2, check):** about 15 rules in families SKL, CMD, AGT, HOOK, PLG, MCP, CFG, MEM with severity (error, warning, info), a health score, a severity filter, grouped by rule.

**Out (default, documented):**
- `~/.claude.json` (user-level MCP servers and much other state): outside the approved folder, not read. The view says so.
- project `.claude/` folders and canon files: only inside approved repository roots, later.
- hook runtime (what ran, how long): phase 3, needs extractor work.
- any file body, env value, hook command line, MCP argument or URL path: never shown.
