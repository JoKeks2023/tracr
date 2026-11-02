# Done when

- [ ] `Reader::environment()` returns the inventory for the approved Claude folders; nothing is read without approval; nothing is written.
- [ ] No secret values reach the window: env values, hook command lines, MCP args and URL paths are dropped in the backend (tests assert it).
- [ ] The check runs the rules listed in `04_impl_report.md`; each finding has rule id, severity, file and a short text; the score is reproducible.
- [ ] Desktop: a rail item "Environment" with sub-navigation (Check, Skills, Commands, Agents, Plugins, MCP, Hooks, Memory, Permissions), empty state with the approve button when no Claude folder is approved, German and English.
- [ ] `cargo fmt`, clippy `-D warnings`, `cargo test --workspace --exclude tracr-desktop`, `pnpm typecheck` green.
- [ ] Runtime: the debug app shows the real inventory (window screenshot); honest "not verified" for anything not observed.
