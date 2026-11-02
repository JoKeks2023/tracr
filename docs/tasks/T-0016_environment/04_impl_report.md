# Implementation report

## Design brief (apple-enterprise-design, before UI code)
- **Work:** you check how Claude Code is set up and whether something is broken, occasionally (after installing a plugin, when a skill is not picked). Read only, so a mistake costs nothing. Objects: findings, skills, commands, agents, plugins, MCP servers, hooks, memory files, permission rules.
- **Workflow:** open Setup → the check first (health, counts, findings grouped by rule with advice and file) → filter by severity → fix the file outside tracr → "Read again". Browsing: pick a category, filter by name, open a row for its facts.
- **Pattern:** the Settings split view (sub-navigation + pane), list rows that open in place: details are a handful of facts, an inspector would be empty most of the time.
- **Hierarchy:** primary the findings (Check) and lists; navigation with counts; contextual: name filter and "Read again"; secondary: row facts (allowlisted frontmatter, size, lines, date, path). No destructive action exists.
- **Layout:** `.st` grid (sub-nav 240 px, 180 px under 900 px), pane max 46 rem; rows are buttons (keyboard: Tab, Enter/Space opens), ⌘5 and the palette reach the area.

## What was built
- `crates/tracr-app/src/environment.rs`: `read(roots)` inventory + `check`, `Reader::environment()`; Tauri command `environment`.
- Frontmatter: own line parser, allowlist `name, description, model, tools, allowed-tools, argument-hint`.
- Secrets never leave the module: hook commands are reduced to the program's file name (kept internally only to find duplicates), MCP servers to transport, program name or URL host and env key names; env values of settings are not read at all.
- Rules (id, severity): SKL-01 no description (error), SKL-02 name ≠ folder (info), SKL-03 description > 1024 chars (warning), SKL-04 name used twice among active items (warning), CMD-01 command > 20 KB (warning), AGT-01 agent without name or description (error), HOOK-01 absolute program path missing (warning), HOOK-02 same command twice for one event and matcher (warning), PLG-01 enabled but not installed (error), PLG-02 install folder missing (error), PLG-03 installed but off (info), MCP-01 server name twice (warning), CFG-01 settings not JSON (error), CFG-02 rule in allow and ask/deny (warning), CFG-03 > 3 settings backups (info), MEM-01 memory/rule > 40,000 chars (warning), MEM-02 auto-memory index > 200 lines (warning). Score = 100 − 10·errors − 3·warnings.
- UI: `v2/Environment.tsx` (rail item "Setup"/"Umgebung", ⌘5, menu View, palette), rule titles and advice in the page (EN/DE), styles in `theme.css`.

## Deviations and decisions (defaults, labelled)
- `~/.claude.json` (user MCP servers) is not read: outside the approved folder. The MCP pane says so.
- Project `.claude/` folders and canon files: not in this step.
- SKL-02 downgraded to info after the real run: Claude Code lists such skills under both names.
- A skill folder without `SKILL.md` (e.g. `_shared`) is not a skill.

## Real run (2026-11-02, the developer's `~/.claude`, numbers only)
Read in 46 ms: 79 skills, 55 commands, 6 agents, 5 plugins, 6 MCP servers, 38 hooks, 21 memory files, 3 permission rules. Before the two fixes above: score 53 with two false SKL-01 errors and five SKL-02 warnings; after them the findings are four CMD-01 warnings (plugin commands of 21–26 KB) and notes.
