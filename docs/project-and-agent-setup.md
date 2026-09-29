# Project and agent setup diagnostics

Status: **Draft specification v0.2 (2026-09-28)**. **Entire capability: later phase (not MVP).** The research package does not cover this area in depth. This document scopes it conservatively and marks what needs its own research.

## 1. Idea

tracr already observes which projects, repositories, worktrees and agents are in use. It could also report whether a project is **prepared for work with multiple coding agents**: whether agents receive consistent instructions, whether integrations tracr relies on (hooks, issue mappings) are configured, and whether handoff material exists.

This is a **diagnostic** feature. tracr reports and recommends. Any modification of a project's files or agent configuration is a separate, explicit, later capability (§5).

## 2. Three categories

| Category | Definition | Reliability |
|---|---|---|
| **Facts** | Directly inspectable, deterministic presence/absence or values | High, as of inspection time |
| **Diagnostics** | Rule-based interpretations of facts, e.g. inconsistency or missing coverage | Medium. Rules are versioned and explainable. |
| **Recommendations / actions** | Suggested remedies, and potentially automated changes | Suggestions only, unless the user explicitly invokes an action |

## 3. Facts tracr can inspect

**Proposed**, subject to per-provider verification of exact file locations:

| Area | Fact examples | Evidence status |
|---|---|---|
| Agent instruction files | Presence, size and last modification of `CLAUDE.md`, `AGENTS.md` at repository root and nested levels. Content read: only reference/import statements, i.e. the path targets they name. No other instruction text is extracted or retained. | File presence is deterministic. Which files each provider version reads, and how, must be taken from provider docs (**U** in research). |
| Agent skills / commands | Presence of provider-specific skill or command directories | **U**: locations per provider version not researched |
| MCP configuration | Presence, size and last modification of project-level MCP configuration. Content read: only configured server names/identifiers. Environment values, arguments, headers and URLs (which can carry tokens) are never extracted, retained or displayed. | **U**: file names/locations per provider not researched |
| Hooks | Whether tracr's own passive hooks are installed for a provider, as recorded by tracr's own installation step (no reading of other hook-configuration content) | Needed by tracr itself; installation must preserve existing hooks (research, claude-code; OQ-I-11). Which hook-configuration content a diagnostic may read, including whether other (non-tracr) hooks exist, is not covered by the scope below: **Deferred** until the same separate privacy review. |
| Git | Worktrees in use; default branch; remote identity; submodules | **D** Git |
| Repository conventions | Presence of handoff/task folders (e.g. `docs/tasks/T-xxxx_*`), contribution docs | Deterministic presence. The *convention* is user-defined. |
| Integration configuration in tracr | Repository ↔ Project ↔ Huly project (workspace-scoped) ↔ GitHub repository mappings | tracr's own records |
| Remote setup | Huly project status set; GitHub↔Huly mapping reachable | Via context adapters. Mapping readability is **U** for the custom Huly MCP. |

**Principle** (inspection scope). Inspecting project and agent files is limited to:

- file presence, size and last-modification time;
- within agent instruction files: only reference/import statements (the path targets they name);
- within MCP configuration: only configured server names/identifiers.

Inspection never extracts environment values, command arguments, headers or URLs that can carry tokens, and never reads credential files. Secrets and tokens are never extracted, retained or displayed. Only the extracted facts are kept; no file content is retained beyond them. Inspection results are not sent to any remote service. Facts that come from tracr's own records, its existing Git evidence or its read-only context adapters are not additional file inspection.

Any broader content inspection, for example detecting divergence between instruction files, needs a separate privacy review before it is specified. This section does not broaden what tracr collects.

## 4. Diagnostics tracr can infer

**Candidate** rule examples. Each diagnostic has an ID, rule version, severity (finite list), the facts it used, and an explanation.

| Diagnostic | Basis |
|---|---|
| "Agents may receive different instructions": only one of `CLAUDE.md` / `AGENTS.md` exists, or both exist and neither references the other | Facts: presence + reference statements. Divergence detection beyond presence and references is **Deferred**: it would read instruction content and needs a separate privacy review first (§3). |
| "Referenced instruction file missing": an instruction file imports or references a path that does not exist | Facts: references + presence |
| "tracr live coverage missing for provider P in this repository" | tracr hook presence + coverage intervals |
| "Repository not mapped to a tracr Project / Huly project" | tracr mappings |
| "Issue workflow statuses differ from user convention" | Huly project status set vs a user-declared convention. Example observed during research: TRACR lacks *Review in progress* and *Merged* ([research/huly.md](../research/huly.md)). |
| "Several active worktrees without issue binding" | Worktree facts + bindings |
| "Handoff documentation absent for active task" | Convention presence. The user must declare the convention. |

Diagnostics describe **state**, not quality of the project or of the developer.

## 5. Recommendations and actions

| Level | Example | Phase |
|---|---|---|
| Recommend | "Add an `AGENTS.md` that references `CLAUDE.md`" with an explanation | Later |
| Prepare | Generate a proposed file or diff for the user to review and apply | Later |
| Automate | Apply changes (install hooks, create files, set Huly statuses) | Later, per action, explicit consent each time |

**Principles:** no silent writes to project files or agent configuration. Hook installation, the one write that tracr's own collection requires, is a separate, explicit setup step that is reversible and preserves existing hooks. Creating Huly projects is outside tracr's actions (user convention: the developer creates Huly projects).

## 6. "Canon" and project decisions

The brief mentions "Canon/project-governance concepts". **Neither the repository nor the research package identifies what "Canon" refers to** (no document, tool, or convention found; research also records that the intended imported CLAUDE.md/workflow/MCP documentation was not available). This specification therefore does **not** assume a technology or format.

A neutral placeholder concept is kept: **Project Decision Record**, a durable, user-maintained statement of a project decision that agents should respect. Whether tracr should detect, index, or link such records to activity and issues is **Unresolved** ([OQ-P-13](open-questions.md#product-questions)). It is pending clarification of what Canon means for this user.

## 7. Multi-agent workflows and handoffs

Observable today (from provider evidence): which agents and sessions worked in which worktree, in what order, with which issue attribution. A **handoff** could later be represented as a relation between consecutive sessions (possibly of different providers) on the same issue, supported by evidence such as a handoff document change or an explicit user marker. The detection rules are **Unresolved**. No handoff is inferred from ordering alone.
