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
