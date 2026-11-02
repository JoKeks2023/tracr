# Validation report (2026-11-02)

| Check | Result |
|---|---|
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -D warnings` | green |
| `cargo test --workspace --exclude tracr-desktop` | green, incl. `tests/environment.rs` (3) and 2 unit tests |
| `pnpm --dir apps/desktop typecheck` | green |
| Secrets test | planted env value, hook arguments, file body, key file, URL password/query, MCP args and env value: none in the serialized result |
| Real folder | 46 ms for the developer's `~/.claude`; findings plausible after two fixes (see 04) |
| Runtime | debug app with a throwaway data dir that approves only `~/.claude`; ⌘5 opened Setup; window screenshot shows the check (score 88, 0 errors, 4 warnings, notes) in German |

**Not verified:** the other panes (skills, plugins, MCP, hooks, memory, permissions) and opening a row were not clicked through; the empty state and its approve button were not shown; English, light mode and the narrow window were not looked at. The approval button in the empty state uses the existing `approve_config_root` command.
