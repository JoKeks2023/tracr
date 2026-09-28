# Cross-platform desktop candidates

Research date: **2026-09-28**. This is a tradeoff study, **not a framework selection**. Official documentation: [D1](SOURCES.md#d1--desktop-candidates); concrete native example: [T2](SOURCES.md#t2--claudoscope-native-macos).

## Framework comparison

| Candidate | Shared logic/UI | Relevant documented facilities | Main uncertainty for tracr |
|---|---|---|---|
| Tauri 2 | Rust backend; web UI; system WebView | Tray ecosystem, commands/IPC, scoped capabilities, autostart and SQL plugins | OS WebView differences, native focus/idle adapter work, packaging tests |
| Electron | JS/TS main process and Chromium UI; native modules possible | Main/renderer split, tray, power/idle/lock events | Background resource cost and native-module/update maintenance must be measured |
| Flutter | Dart UI/core; native plugins or FFI | macOS/Windows desktop and plugin support | Tray/autostart/idle plugin quality and lifecycle parity need validation |
| Native macOS + native Windows | Swift/AppKit/SwiftUI and Windows-native shell | Direct platform integration | Two UI implementations; share contracts/core through a chosen boundary |
| Rust/native hybrid | Rust parser/store/reconciliation with native shells or thin web shell | Reuse portable core; direct OS adapters | FFI/IPC versioning, distribution and team expertise |
| .NET MAUI | C# logic; WinUI Windows and Mac Catalyst | Official Windows/macOS targets | Mac Catalyst fit for deep desktop observation and tray behavior |
| .NET + Avalonia | C# shared desktop UI/core | Official macOS/Windows support | Native integration adapters and packaging verification |

Tauri avoids bundling a browser engine by using the system WebView; this does not itself establish tracr's RAM usage. Electron's browser/main process model offers a different consistency/resource tradeoff. No comparative benchmark was run. Flutter's desktop support is not evidence that every mobile plugin supports both desktop OSs. MAUI and Avalonia are different choices, not interchangeable “.NET desktop.” [D1](SOURCES.md#d1--desktop-candidates)

## Product needs independent of framework

| Requirement | Candidate implementation boundary | Verification needed later |
|---|---|---|
| macOS menu bar / Windows tray | Thin native shell, keep ownership outside dashboard window | Closing UI does not accidentally stop tracking; explicit Quit does |
| Background collection | User-session process; optional helper | Crash/restart, suspend/resume and machine logout |
| Autostart | OS login mechanism exposed by shell/plugin | User consent, removal, updates and no duplicate processes |
| Notifications | Local OS notification API | Permissions, quiet hours, click routing |
| File access/watch | Scoped provider directories plus overflow rescans | Rotation, atomic saves, partial JSON, network/WSL paths |
| SQLite | Core owns schema and writes | Transactions, backups, migration, recovery and concurrent readers |
| IPC | Typed commands/events; local pipes/socket or framework bridge | Caller trust, versioning, backpressure and shutdown |
| Process/focus detection | Optional OS-specific adapter | Permissions and terminal-tab ambiguity |
| Install/update | Signed OS packages and explicit update channel | Fresh install, upgrade, rollback, helper cleanup |

These are **P** component boundaries to evaluate, not an application architecture being committed to.

## Background-process alternatives

1. **Single tray app with worker:** simplest deployment and local communication; collector uptime tied to app process.
2. **User-session helper plus UI:** collector survives UI restart; more lifecycle/IPC/update work.
3. **System service/daemon:** stronger independence but wrong privilege/session boundaries may complicate observing the logged-in user; not a justified default for a personal diary.

**P:** Start comparisons with a user-session process. Windows idle information is session-scoped; moving to a service is not a free reliability upgrade. Observe sleep/lock separately from agent execution. Electron explicitly offers system idle time and lock/unlock/suspend/resume events, but no per-agent human-focus truth. [Windows API](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getlastinputinfo), [Electron powerMonitor](https://www.electronjs.org/docs/latest/api/power-monitor)

## Permission and privacy model to test

**P:** Limit filesystem scope to user-selected provider/repository directories; keep OS credentials in their platform stores. A SQLite file is not automatically encrypted. Avoid exposing full transcripts to a web renderer unless required. Treat transcript Markdown/tool outputs as untrusted display data, not executable HTML or instructions.

Foreground-app identity, window-title access, automation and input monitoring have different permission implications. Do not promise “no permissions” based on reading logs alone. Validate actual macOS signing/sandbox/privacy behavior and Windows user/session behavior for each optional sensor. Screen recording and key-content capture are excluded by the brief.

A loopback HTTP server is still an access surface; evaluate bind address, authentication, origin checks and available filesystem routes. A local IPC transport also needs identity/access control. Framework permission declarations are only one layer of this review.

## Storage and analytics alternatives

SQLite is a plausible transactional store for revisions/timers/sync receipts; T2/T5 provide concrete usage examples. DuckDB is a plausible analytical projection engine; T4 demonstrates SQL normalization. Using both adds packaging and consistency costs. Keep this as a measured choice: mutation volume, search, report latency, migration complexity and recovery matter more than database popularity.

## Deferred evaluation matrix

After research approval/design, evaluate minimal equivalent collectors on macOS and Windows with synthetic logs: idle CPU/wakeups/RAM, cold start, 100k-event import, partial writes, sleep, tray persistence, update recovery, Unicode paths, accessibility and export. Use cloud CI for heavy platform builds; never boot a simulator on the user's 8-GB M1. No build, simulator, app shell, package installation or benchmark was performed here. The framework remains open.
