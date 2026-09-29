# Constraints and context

- Speed over theoretical perfection. Settled v0.2 semantics are binding; open product questions get labeled 0.9 defaults only where code needs a behavior.
- Developer machine: M1 with 8 GB RAM. No simulators or emulators; heavy builds (release bundles, notarization, Windows) go to CI.
- Toolchains checked on 2026-09-29: Node 24, pnpm 12, Swift 6.4, Python 3.14, sqlite3 3.54 are installed. **Rust is not installed.** Installing it via `rustup` is the first prerequisite of T-0004.
- Local agent history checked on 2026-09-29 (file listing and record key names only; no prompt, tool or message content was read): 364 Claude Code transcript files, 321 of them subagent files; 36 Codex rollouts (sessions + archived). Record shapes are summarized in the header of [architecture.md](../../architecture.md).
- Global user rules apply: repository truth over chat, no silent scope changes, dropdowns for enumerable values, workspace-explicit Huly, no squash merges, commit only on request.
- No Huly or GitHub issue was created: this is a planning document, not a discovered bug or TODO, and nothing asked for remote writes.
