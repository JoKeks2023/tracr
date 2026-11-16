# T-0027 Setup inspector: project facts and plans (rest of TRACR-23)

T-0016 (TRACR-37) shipped the Claude folder inventory and lint. This adds the project side and plans, within the inspection scope of docs/project-and-agent-setup.md §3.

- `crates/tracr-app/src/setup.rs`, `Reader::project_setup`: for each repository observed by a git probe under an approved repository root:
  - `CLAUDE.md`/`AGENTS.md`: presence, size, time, and `@path` references only (not mentions, e-mails, code spans or fences);
  - `.claude/canon.md`: number of records and how many are `status: canon`. **Titles are not read**: that would be content and needs the separate privacy review §3 asks for;
  - `.mcp.json`: server names only (never command, args, env, URLs);
  - diagnostics (setup §4, versioned rule IDs): `reference_missing@1` (warning), `instructions_unlinked@1` (both files, neither references the other; warning), `single_instruction_file@1` and `no_instructions@1` (info).
- **Plans** in the approved Claude folders: count, sizes and times only. Plan file names are made from prompt words, so they are not shown.
- Tauri `project_setup`; TS `SetupView`, `ProjectSetup`, `api.projectSetup`. **No screen yet** (Environment area, needs the Mac).

## Validation
Unit tests (references parser, findings, canon counts, `.mcp.json` secrets never leave, plan names never leave) and `tests/setup.rs` (an observed repository under an approved root is inspected end to end). `cargo test -p tracr-app` and clippy `-D warnings` pass; `cargo check -p tracr-desktop` and `pnpm typecheck` pass. Not verified: real repositories, UI.

## Left in TRACR-23
Canon titles and plan contents: only after a privacy decision (setup §3).
