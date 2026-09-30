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

## M2: derivation (2026-09-30)

| Part | Where |
|---|---|
| Coverage per key from runs, heartbeats, stalls, suspends and the live edge | `crates/tracr-core/src/coverage.rs` |
| Device lane (precedence suspended > locked > idle > active; unknown outside sensor coverage; run-start reset) | `crates/tracr-core/src/device.rs` (`device_lane`) |
| Human timeline and counting types (§7.2–§7.4) | `crates/tracr-core/src/human.rs` |
| Rebuild integration: collector/sensor rows, degraded extents, per-run device facts, day split, contexts, proposals | `crates/tracr-app/src/derive_human.rs`, `crates/tracr-store/src/{ledger,derived}.rs`, `schema/derived.sql` (v2) |
| Day DTO: human atoms, counted/unknown, device and coverage lanes | `crates/tracr-app/src/day.rs` |
| UI: human/device/coverage lanes, counted/unknown summary, export kinds | `apps/desktop/ui/src/{Timeline,App,ExportSheet}.tsx` |
| Timesheet CSV and export everything | `crates/tracr-app/src/export.rs`, `docs/export.md` |

### Bugs found by testing against the real sensor (fixed)

1. **A run's reset erased its own first sample.** The first sample places the initial state at the last input, before the run start. Fix: a transition applies from the start of the run that recorded it.
2. **The runtime took its first sample before `coverage_started`.** That sample was attributed to the previous run. Fix: the runtime samples after the run begins, and derivation attributes transitions by run ID, not by time.

Without these fixes the device lane stayed `unknown` after every start, and nothing could ever be counted.

### Finding: long unknown windows (product decision pending)

A literal reading of §7.3 turns every unbridged gap between consecutive prompts into `unknown`, and adjacent ones merge. On the real history, before the sensor ran, this yields one unknown window chaining weeks: 1,440 unknown minutes on every day, including nights and days without any prompt. With the sensor, a night with the Mac awake but idle (not locked, not asleep) would also be one unknown window.

This is spec-conformant but useless in a timesheet. It was raised with the user as a product decision.

**Decision (2026-09-30):** unbridged gaps longer than 2 h are blank, and they separate unknown windows (`human-timeline@2`).

- On the real history, September days now show 2–10 h unknown (the hours actually worked), instead of 24 h.
- History is still never counted: there was no sensor.
- **Deviation from §7.3 / activity model §6:** those texts make every unbridged gap unknown. The decision adds a length cap.

## M3: incremental derivation, git context and attribution, completeness (2026-09-30)

| Part | Where |
|---|---|
| `derive::update`: dirty sessions (observations past the watermark) closed over copy-election peers; human, device and coverage layers re-read from the store | `crates/tracr-app/src/derive.rs`, `crates/tracr-store/src/{derived,ledger}.rs` |
| Runtime: incremental update at each heartbeat with changes; full rebuild only on a missing or outdated store | `crates/tracr-app/src/runtime.rs`, `lib.rs` (`update_derived`) |
| Collector rows selected by source instead of channel: an empty update went from about 7 s to 0.1 s | `ledger::collector_and_sensor_rows` |
| `ProbeAdapter` trait (core); `tracr-git` (pure plan and parse, remote normalization without credentials); probe executor (approved roots only, `GIT_OPTIONAL_LOCKS=0`, unchanged results skipped) | `crates/tracr-core/src/adapter.rs`, `crates/tracr-git/`, `crates/tracr-ingest/src/probe.rs` |
| Attribution of work atoms (worktree and repository levels, report target), session repository; `counted_by_target`; timesheet `repository`, `worktree`, `attribution_state` | `crates/tracr-app/src/{derive_human,day,export}.rs`, schema v3 |
| Completeness: ingestion watermark, stream gap and degraded extents, recovered records per day, provisional reasons; UI state line and coverage extents | `derive_human.rs`, `day.rs`, schema v4, `apps/desktop/ui/src/{App,Timeline}.tsx` |

### Deviations and findings

- **Branch probe split.** `rev-parse --abbrev-ref HEAD` fails in a repository without commits and took down the whole probe. The branch now comes from `symbolic-ref --short -q HEAD` (no branch when HEAD is detached).
- **Provisional.** Every day is provisional until review exists (S2). The meaningful state today is the ingestion state (complete, or still arriving) and is shown separately.
- **Project level (repository → tracr Project mapping)** follows with mappings (S3). The report target stops at repository.
- **Real data:** 21 repositories identified by remote. Agent worktrees and subfolders collapse onto their repository. A nested repository (`avmsdesk/libs/hbb_common`) resolves to its own remote. A renamed repository is shown by its remote name (`jukebox` → `jokeks2023/cabinet`).
