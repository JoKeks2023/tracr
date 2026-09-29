# tracr desktop

The Tauri 2 host with the read-only Day view (architecture §10, §13.4). The window talks only to `tracr-app` through four commands: `status`, `days`, `day_agents` and `refresh`. It gets no filesystem, shell or HTTP capability (AD-12).

## Run it

```sh
cd apps/desktop
pnpm install
TRACR_DATA_DIR=/path/to/data pnpm tauri dev   # Vite dev server + debug build of the host
```

Without `TRACR_DATA_DIR`, the app uses the platform data directory, `~/Library/Application Support/tracr` on macOS.

The data directory needs a ledger with an approved provider root:

```sh
tracr --data-dir /path/to/data roots add --provider ~/.claude/projects
```

**Refresh** then runs import and rebuild. It takes the single-writer lock (AD-03) and fails cleanly while the CLI holds it.

## Layout

- `src-tauri/` holds the Rust host (workspace member `tracr-desktop`). It embeds `dist/` in bundles, so run `pnpm build` before `cargo build --workspace`.
- `ui/` holds the React + TypeScript UI (Vite). The DTO types in `ui/src/types.ts` mirror `crates/tracr-app/src/day.rs`.

CI builds the host on macOS only; Linux would need WebKitGTK. Linux and Windows exclude `tracr-desktop`.
