# T-0030 Screens for search, setup facts and bindings (TRACR-21, TRACR-23, TRACR-40)

The backends of T-0023, T-0024 and T-0027 had no screens. This adds them; there are no backend changes.

- **Sessions area** (`v2/SessionsPage.tsx`, new rail entry under Projects): session search over metadata for the period chosen in the bar (day/week/month).
  - Free text (label, folder, branch, repository, ID; it waits 250 ms after typing).
  - Every other filter is a dropdown of the period's facet values: provider, project, branch, model, tool, bound to, subagents with/without, and minimum cost as presets (0.10/1/5/10/50 in the pricing currency).
  - Shows "n of total" and states that unpriced sessions never match a cost filter. Clicking a row opens the session sheet, including Bind/Reject.
  - Placement decision (default, open to change): its own area rather than a mode of Projects, because Projects is already a list plus detail.
- **Environment → Repositories** (`v2/Environment.tsx`): per repository, the instruction files with size, date and `@path` imports, canon counts (`canon` of `records`), MCP server names and findings. Findings get localized titles keyed by rule ID without its version; only the missing path is repeated from the backend message. Plans: count, total size and newest date only, with the reason names are hidden.
- **Settings → Projects → Bindings** (`v2/Settings.tsx`): active bindings (subject → target, since) with End (op `end`, so the history stays), a count of ended ones, and rejected suggestions read-only. There is no un-reject op in the backend; adding one is a separate decision.
- `targetName` is exported from `SessionSheet.tsx` so that target names read the same everywhere. EN and DE strings added; filter bar CSS (`.sfilters`, `.sfilter`).

## Validation
`pnpm typecheck` and `pnpm build` (Vite) pass. **Not verified at runtime:** the UI has no browser mock, and tracr.app stays closed until the Mac is free. To click through: the Sessions area (every filter, cost presets, the sheet, then Bind and see the list update); Environment → Repositories with an approved repository root (the live ledger has none yet, so it shows the empty state); Settings → Projects → Bindings (End).
