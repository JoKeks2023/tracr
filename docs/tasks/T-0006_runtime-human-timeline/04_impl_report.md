# Implementation report

## M1: collection (2026-09-30)

| Part | Where |
|---|---|
| Fact kinds `coverage_started`, `collector_heartbeat`, `coverage_ended`, `device_*`; `DeviceFacts`; trigger `sample` | `crates/tracr-core/src/{events,evidence,device}.rs` |
| `DeviceTracker`: samples → transitions (idle at the last input, observed; active bounded by samples; lock bounded; suspend/resume from kernel times, observed) | `crates/tracr-core/src/device.rs` |
| `DeviceSensor` trait; macOS implementation via IOKit `HIDIdleTime`, `CGSessionCopyCurrentDictionary`, `kern.sleeptime`/`kern.waketime`; unsupported fallback (all unavailable) | `crates/tracr-platform/src/{lib,macos_sensor,unsupported}.rs` |
| Runtime run context, OS-sensor source, generic record writer | `crates/tracr-ingest/src/source.rs` |
| `Collector` (writer-side job handling) and `Runtime` (writer, timer and `notify` watcher threads; `RuntimeHandle`) | `crates/tracr-app/src/runtime.rs` |
| `App::reconcile_all(trigger)` shared by import and the runtime; `with_client` | `crates/tracr-app/src/lib.rs` |
| CLI `tracr run` (Ctrl-C writes `coverage_ended`) | `apps/cli/src/main.rs` |
| Desktop app hosts the runtime; hide on close; tray menu Open/Quit; first-run approval of proposed roots; collector status in the header | `apps/desktop/src-tauri/src/main.rs`, `apps/desktop/ui/src/App.tsx` |

## Switch to the permanent data directory (2026-09-30)

- The Claude root was approved via the CLI. `tracr run` backfilled the full history in under 25 s and collected from 18:49.
- A standalone `tracr.app` (release bundle, `~/Applications/tracr.app`) took over at 18:56:53, after `coverage_ended` at 18:56:43. The 10 s in between will show as not collected.
- Bundling needed an `.icns` icon. The icon set was generated with `tauri icon`; Android and iOS outputs were removed.

## CSV export (pulled forward from S4)

| Part | Where |
|---|---|
| `agent-sessions` CSV: one row per day × session × actor; RFC 4180; empty = unknown; prompts only on the main-chain row | `crates/tracr-app/src/export.rs`, `docs/export.md` |
| CLI `tracr export agent-sessions --from --to [--out]` | `apps/cli/src/main.rs` |
| App: Export CSV sheet (From/To dropdowns of days with data) → native save panel via `tauri-plugin-dialog`, called from Rust; the webview still has no filesystem access | `apps/desktop/src-tauri/src/main.rs`, `apps/desktop/ui/src/ExportSheet.tsx` |
| Test kit: `uuid_prefix`, so streams of one session never collide by accident | `crates/tracr-testkit/src/claude.rs` |
