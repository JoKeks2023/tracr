# T-0024 Session search over metadata (TRACR-21)

Built while the dev machine was busy: backend, Tauri command and UI types. **No screen yet.**

- `crates/tracr-app/src/search.rs`: `SessionQuery` (period; text in label/folder/branch/repository/ID; provider; project; branch; model; tool; subagents; bound target; minimum estimated cost), `SessionSearch` (matching list with its summed estimate, total before filtering, facets).
- **Facets** list the period's providers, branches, models, tools, projects and bound targets, so a UI can use dropdowns (no free text for enumerable values).
- **Project** match: the session is bound to the project, or its repository is mapped to it.
- **Cost:** an unpriced session never matches a minimum (unpriced is not 0).
- **Never content:** prompts and messages are not stored (mvp M3), so there is no full-text search. The `text` filter only looks at metadata.
- `Reader::search_sessions`. `Reader::sessions` and `day_agents` now share `attach_bindings`, so the sessions list also carries `bound_to` and `suggestions`.
- Tauri `search_sessions`; TS `SessionQuery`, `SessionSearch`, `api.searchSessions`.

## Validation
`tests/search.rs` covers every filter, the facets, another period, and project by mapping and by binding. `cargo test -p tracr-app` and clippy `-D warnings` pass; `cargo check -p tracr-desktop` and `pnpm typecheck` pass. Not verified: a UI (none yet), a real ledger.

## Next (needs the Mac)
A search bar with dropdowns on the Sessions/Projects page.
