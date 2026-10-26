//! tracr desktop host (architecture §10, §13.4, AD-03).
//!
//! The app process hosts the collector runtime while it runs: closing the
//! window only hides it, and Quit (tray menu or ⌘Q) stops the collector
//! cleanly with `coverage_ended`. The window talks only to `tracr-app`
//! through the commands below and gets no filesystem, shell or HTTP
//! capability (AD-12); transcript content is never rendered (PS-03).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, RunEvent, WindowEvent};
use tracr_app::day::{DayAgents, DayList};
use tracr_app::runtime::{Refreshed, Runtime, RuntimeStatus};
use tracr_app::{App, Reader, RootKind, SystemClock, default_data_dir, proposed_provider_roots};

const CLIENT: &str = "tracr-desktop";

struct Ctx {
    data_dir: Option<PathBuf>,
    from_env: bool,
    runtime: Mutex<Option<Runtime>>,
    /// Why the collector is not running here (e.g. `tracr run` holds the lock).
    runtime_error: Mutex<Option<String>>,
}

impl Ctx {
    fn dir(&self) -> Result<PathBuf, String> {
        self.data_dir
            .clone()
            .ok_or_else(|| "no data directory: set TRACR_DATA_DIR".to_owned())
    }

    fn repository_roots(&self) -> Vec<String> {
        self.data_dir
            .as_ref()
            .and_then(|d| Reader::open(d).ok())
            .and_then(|r| r.repository_roots().ok())
            .unwrap_or_default()
    }

    /// Runs `change` with the writer lock while the collector is stopped, then
    /// restarts the collector so a changed root set is watched (or dropped).
    fn change_roots(
        &self,
        reason: &str,
        change: impl FnOnce(&mut App) -> anyhow::Result<()>,
    ) -> Result<(), String> {
        let dir = self.dir()?;
        self.stop_runtime(reason);
        let result = App::open_writer(&dir, Box::new(SystemClock))
            .map(|app| app.with_client(CLIENT))
            .and_then(|mut app| change(&mut app));
        self.start_runtime();
        result.map_err(err)
    }

    fn approved_roots(&self) -> Vec<String> {
        self.data_dir
            .as_ref()
            .and_then(|d| Reader::open(d).ok())
            .and_then(|r| r.provider_roots().ok())
            .unwrap_or_default()
    }

    /// Starts the collector if there is something to collect.
    fn start_runtime(&self) {
        let Ok(dir) = self.dir() else { return };
        if self.approved_roots().is_empty() {
            return;
        }
        let mut slot = self.runtime.lock().expect("runtime lock");
        if slot.is_some() {
            return;
        }
        match Runtime::start(&dir, CLIENT) {
            Ok(rt) => {
                *slot = Some(rt);
                *self.runtime_error.lock().expect("error lock") = None;
            }
            Err(e) => *self.runtime_error.lock().expect("error lock") = Some(format!("{e:#}")),
        }
    }

    fn stop_runtime(&self, reason: &str) {
        if let Some(rt) = self.runtime.lock().expect("runtime lock").take() {
            rt.stop(reason);
        }
    }
}

/// Approved roots are stored resolved (`add_root` canonicalizes), while a
/// proposal is the plain path, which may be a symlink (dotfile managers do
/// that to `~/.claude`). Compare both spellings.
fn is_approved(proposal: &std::path::Path, approved: &[String]) -> bool {
    let plain = proposal.display().to_string();
    let resolved = std::fs::canonicalize(proposal)
        .ok()
        .map(|p| p.display().to_string());
    approved.contains(&plain) || resolved.is_some_and(|r| approved.contains(&r))
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[derive(Serialize)]
struct ProposedRoot {
    path: String,
    approved: bool,
}

#[derive(Serialize)]
struct Status {
    data_dir: Option<String>,
    data_dir_from_env: bool,
    ledger_present: bool,
    approved_roots: Vec<String>,
    approved_repository_roots: Vec<String>,
    proposed_roots: Vec<ProposedRoot>,
    /// The user's home folder, so the window can show `~/…` instead of a user name.
    home: Option<String>,
    app_version: &'static str,
    /// The reporting zone in force, whether the user chose it, and the system zone it would fall back to.
    timezone: Option<String>,
    timezone_chosen: bool,
    system_timezone: Option<String>,
    /// Reviewed days fix the zone (AD-14), so the window shows it as locked.
    reviewed_days: i64,
    /// `collecting`, `stalled` (no heartbeat for 3 min), `elsewhere` (another
    /// tracr process holds the lock), `stopped` or `not_configured`.
    collector: &'static str,
    collector_detail: Option<String>,
    runtime: Option<RuntimeStatus>,
    running_timer: Option<tracr_app::day::ManualEntryInfo>,
}

// Read commands are async: Tauri runs sync commands on the main thread, and
// the UI polls `status` every 5 s. They must never block the event loop.
#[tauri::command]
async fn status(ctx: tauri::State<'_, Ctx>) -> Result<Status, String> {
    let approved = ctx.approved_roots();
    let runtime = ctx
        .runtime
        .lock()
        .expect("runtime lock")
        .as_ref()
        .map(Runtime::status);
    let error = ctx.runtime_error.lock().expect("error lock").clone();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    // Heartbeats come every 60 s: older than 3 min means the collector is stuck.
    let stalled = runtime
        .as_ref()
        .and_then(|r| r.last_heartbeat)
        .is_some_and(|t| now - t > 3 * 60_000);
    let collector = match (&runtime, &error) {
        (Some(r), _) if r.collecting && stalled => "stalled",
        (Some(r), _) if r.collecting => "collecting",
        (_, Some(e)) if e.contains("another tracr process") => "elsewhere",
        _ if approved.is_empty() => "not_configured",
        _ => "stopped",
    };
    let reader = ctx.data_dir.as_ref().and_then(|d| Reader::open(d).ok());
    let zone = reader.as_ref().and_then(|r| r.reporting_zone().ok());
    Ok(Status {
        data_dir: ctx.data_dir.as_ref().map(|d| d.display().to_string()),
        data_dir_from_env: ctx.from_env,
        ledger_present: ctx
            .data_dir
            .as_ref()
            .is_some_and(|d| d.join("ledger.sqlite").exists()),
        proposed_roots: proposed_provider_roots()
            .into_iter()
            .map(|p| ProposedRoot {
                approved: is_approved(&p, &approved),
                path: p.display().to_string(),
            })
            .collect(),
        approved_roots: approved,
        approved_repository_roots: ctx.repository_roots(),
        home: std::env::var("HOME").ok(),
        app_version: env!("CARGO_PKG_VERSION"),
        timezone: zone.as_ref().map(|z| z.name.clone()),
        timezone_chosen: zone.as_ref().is_some_and(|z| z.from_setting),
        system_timezone: tracr_app::day::ReportingZone::resolve(None)
            .ok()
            .map(|z| z.name),
        reviewed_days: reader
            .as_ref()
            .and_then(|r| r.reviewed_day_count().ok())
            .unwrap_or(0),
        collector,
        collector_detail: error,
        runtime,
        running_timer: ctx
            .data_dir
            .as_ref()
            .and_then(|d| Reader::open(d).ok())
            .and_then(|r| r.running_timer().ok().flatten()),
    })
}

/// Approves proposed provider roots, several at once (PS-02: an explicit user
/// action). The collector restarts once so the new roots are watched.
#[tauri::command]
async fn approve_roots(ctx: tauri::State<'_, Ctx>, paths: Vec<String>) -> Result<(), String> {
    let proposed: Vec<String> = proposed_provider_roots()
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    if paths.is_empty() || paths.iter().any(|p| !proposed.contains(p)) {
        return Err("only proposed provider roots can be approved here".into());
    }
    ctx.change_roots("roots approved; restarting", |app| {
        for p in &paths {
            app.add_root(RootKind::Provider, &PathBuf::from(p))?;
        }
        Ok(())
    })
}

fn root_kind(kind: &str) -> Result<RootKind, String> {
    match kind {
        "provider" => Ok(RootKind::Provider),
        "repository" => Ok(RootKind::Repository),
        other => Err(format!("unknown root kind {other}")),
    }
}

/// Chooses the reporting time zone (AD-14), or `None` for the system zone, and
/// rebuilds the derived store because day boundaries move. Refused once a day
/// has been reviewed.
#[tauri::command]
async fn set_timezone(ctx: tauri::State<'_, Ctx>, name: Option<String>) -> Result<(), String> {
    ctx.change_roots("time zone changed; restarting", |app| {
        app.set_timezone(name.as_deref())?;
        app.rebuild()?;
        Ok(())
    })
}

/// Lets the user pick a folder in the native panel and approves it. The
/// window itself has no filesystem access (AD-12); the panel and the write
/// happen here. Resolves to the approved path, or `None` if cancelled.
#[tauri::command]
async fn choose_root(
    app: tauri::AppHandle,
    ctx: tauri::State<'_, Ctx>,
    kind: String,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let kind = root_kind(&kind)?;
    let title = match kind {
        RootKind::Provider => "Choose a folder with agent history tracr may read",
        RootKind::Repository => "Choose a folder that contains your git repositories",
    };
    let Some(path) = app
        .dialog()
        .file()
        .set_title(title)
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    ctx.change_roots("root approved; restarting", |app| {
        app.add_root(kind, &path)?;
        Ok(())
    })?;
    Ok(Some(path.display().to_string()))
}

/// Stops reading an approved root. What was imported stays in the ledger.
#[tauri::command]
async fn remove_root(ctx: tauri::State<'_, Ctx>, kind: String, path: String) -> Result<(), String> {
    let kind = root_kind(&kind)?;
    ctx.change_roots("root removed; restarting", |app| {
        app.remove_root(kind, &PathBuf::from(&path))?;
        Ok(())
    })
}

#[tauri::command]
async fn days(ctx: tauri::State<'_, Ctx>) -> Result<DayList, String> {
    Reader::open(&ctx.dir()?)
        .and_then(|r| r.days())
        .map_err(err)
}

#[tauri::command]
async fn day_agents(ctx: tauri::State<'_, Ctx>, day: String) -> Result<DayAgents, String> {
    Reader::open(&ctx.dir()?)
        .and_then(|r| r.day_agents(&day))
        .map_err(err)
}

/// Reconcile everything now and rebuild the derived store, through the
/// running collector (it holds the writer lock).
#[tauri::command]
async fn refresh(ctx: tauri::State<'_, Ctx>) -> Result<Refreshed, String> {
    let handle = ctx
        .runtime
        .lock()
        .expect("runtime lock")
        .as_ref()
        .map(Runtime::handle);
    let Some(handle) = handle else {
        return Err(ctx
            .runtime_error
            .lock()
            .expect("error lock")
            .clone()
            .unwrap_or_else(|| "the collector is not running".into()));
    };
    tauri::async_runtime::spawn_blocking(move || handle.refresh())
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

/// Exports `[from, to]` as CSV (`timesheet` or `agent_sessions`) to a file
/// the user picks in a native save panel. The dialog and the write happen
/// here, in the host; the webview gets no filesystem access (AD-12).
#[tauri::command]
async fn export_csv(
    app: tauri::AppHandle,
    ctx: tauri::State<'_, Ctx>,
    kind: String,
    from: String,
    to: String,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let reader = Reader::open(&ctx.dir()?).map_err(err)?;
    let (csv, name) = match kind.as_str() {
        "timesheet" => (
            tracr_app::export::timesheet_csv(&reader, &from, &to),
            format!("tracr-timesheet_{from}_{to}.csv"),
        ),
        "agent_sessions" => (
            tracr_app::export::agent_sessions_csv(&reader, &from, &to),
            tracr_app::export::default_file_name(&from, &to),
        ),
        other => return Err(format!("unknown export {other}")),
    };
    let csv = csv.map_err(err)?;
    let picked = app
        .dialog()
        .file()
        .set_file_name(name)
        .add_filter("CSV", &["csv"])
        .blocking_save_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(None); // cancelled
    };
    std::fs::write(&path, csv).map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

/// Exports everything (one CSV per data set) into a new folder inside the
/// folder the user picks.
#[tauri::command]
async fn export_everything(
    app: tauri::AppHandle,
    ctx: tauri::State<'_, Ctx>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let reader = Reader::open(&ctx.dir()?).map_err(err)?;
    let Some(parent) = app
        .dialog()
        .file()
        .set_title("Choose where to put the export folder")
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    let dir = parent.join(reader.export_folder_name().map_err(err)?);
    reader.export_everything(&dir).map_err(err)?;
    Ok(Some(dir.display().to_string()))
}

/// Records a user correction through the collector (it holds the writer
/// lock); returns once the derived store reflects it.
#[tauri::command]
async fn correct(
    ctx: tauri::State<'_, Ctx>,
    req: tracr_app::correct::CorrectionRequest,
) -> Result<String, String> {
    let handle = ctx
        .runtime
        .lock()
        .expect("runtime lock")
        .as_ref()
        .map(Runtime::handle);
    let Some(handle) = handle else {
        return Err("corrections need the collector of this app; it is not running here".into());
    };
    tauri::async_runtime::spawn_blocking(move || handle.correct(req))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

/// Timer and manual-entry operations through the collector.
#[tauri::command]
async fn manual(
    app: tauri::AppHandle,
    ctx: tauri::State<'_, Ctx>,
    req: tracr_app::manual::ManualRequest,
) -> Result<String, String> {
    let handle = ctx
        .runtime
        .lock()
        .expect("runtime lock")
        .as_ref()
        .map(Runtime::handle);
    let Some(handle) = handle else {
        return Err("manual entries need the collector of this app; it is not running here".into());
    };
    let id = tauri::async_runtime::spawn_blocking(move || handle.manual(req))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)?;
    refresh_tray(&app);
    Ok(id)
}

fn runtime_handle(ctx: &Ctx) -> Result<tracr_app::runtime::RuntimeHandle, String> {
    ctx.runtime
        .lock()
        .expect("runtime lock")
        .as_ref()
        .map(Runtime::handle)
        .ok_or_else(|| "this needs the collector of this app; it is not running here".into())
}

/// Reviews a day: stores its current resolved state as the next revision.
#[tauri::command]
async fn review(
    ctx: tauri::State<'_, Ctx>,
    day: String,
    reason: Option<String>,
) -> Result<String, String> {
    let handle = runtime_handle(&ctx)?;
    tauri::async_runtime::spawn_blocking(move || handle.review(day, reason))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

/// Keeps the reviewed revision for exactly this difference.
#[tauri::command]
async fn dismiss(
    ctx: tauri::State<'_, Ctx>,
    day: String,
    fingerprint: String,
) -> Result<String, String> {
    let handle = runtime_handle(&ctx)?;
    tauri::async_runtime::spawn_blocking(move || handle.dismiss(day, fingerprint))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

/// Targets time can be assigned to (dropdown, UI-01).
#[tauri::command]
async fn targets(ctx: tauri::State<'_, Ctx>) -> Result<Vec<tracr_app::day::TargetOption>, String> {
    Reader::open(&ctx.dir()?)
        .and_then(|r| r.targets())
        .map_err(err)
}

fn running_timer(app: &tauri::AppHandle) -> Option<tracr_app::day::ManualEntryInfo> {
    let ctx = app.state::<Ctx>();
    ctx.data_dir
        .as_ref()
        .and_then(|d| Reader::open(d).ok())
        .and_then(|r| r.running_timer().ok().flatten())
}

fn hm(ms: i64) -> String {
    let m = (ms + 30_000) / 60_000;
    if m < 60 {
        format!("{m}m")
    } else {
        format!("{}h {:02}m", m / 60, m % 60)
    }
}

/// Today's counted and unknown human minutes, for the menu (S1.7).
fn today_line(app: &tauri::AppHandle) -> String {
    let ctx = app.state::<Ctx>();
    let Some(reader) = ctx.data_dir.as_ref().and_then(|d| Reader::open(d).ok()) else {
        return "Today: no data yet".into();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let today = reader
        .reporting_zone()
        .ok()
        .and_then(|z| z.day_of(now))
        .map(|d| d.to_string());
    match today.and_then(|d| reader.day_agents(&d).ok()) {
        Some(v) => format!(
            "Today: {} counted · {} unknown, not counted",
            hm(v.human.counted_ms),
            hm(v.human.unknown_ms)
        ),
        None => "Today: no data yet".into(),
    }
}

fn collector_line(app: &tauri::AppHandle) -> &'static str {
    let collecting = app
        .state::<Ctx>()
        .runtime
        .lock()
        .expect("runtime lock")
        .as_ref()
        .map(Runtime::status)
        .is_some_and(|s| s.collecting);
    if collecting {
        "Collecting"
    } else {
        "Not collecting"
    }
}

/// The menu-bar item shows the running timer's elapsed time; its menu shows
/// today's minutes and the collector state, and offers start or stop.
fn refresh_tray(app: &tauri::AppHandle) {
    let Some(tray) = app.tray_by_id("tracr") else {
        return;
    };
    let running = running_timer(app);
    let title = running.as_ref().map(|t| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(t.start);
        let mins = (now - t.start).max(0) / 60_000;
        format!("{}:{:02}", mins / 60, mins % 60)
    });
    let _ = tray.set_title(title.as_deref());
    let label = if running.is_some() {
        "Stop timer"
    } else {
        "Start timer"
    };
    if let (Ok(today), Ok(state), Ok(open), Ok(timer), Ok(quit)) = (
        MenuItem::with_id(app, "today", today_line(app), false, None::<&str>),
        MenuItem::with_id(app, "state", collector_line(app), false, None::<&str>),
        MenuItem::with_id(app, "open", "Open tracr", true, None::<&str>),
        MenuItem::with_id(app, "timer", label, true, None::<&str>),
        MenuItem::with_id(app, "quit", "Quit tracr", true, None::<&str>),
    ) && let Ok(menu) = Menu::with_items(app, &[&today, &state, &open, &timer, &quit])
    {
        let _ = tray.set_menu(Some(menu));
    }
}

fn toggle_timer(app: &tauri::AppHandle) {
    let op = if running_timer(app).is_some() {
        "stop_timer"
    } else {
        "start_timer"
    };
    let handle = app
        .state::<Ctx>()
        .runtime
        .lock()
        .expect("runtime lock")
        .as_ref()
        .map(Runtime::handle);
    let app = app.clone();
    if let Some(handle) = handle {
        tauri::async_runtime::spawn_blocking(move || {
            let _ = handle.manual(tracr_app::manual::ManualRequest {
                op: op.into(),
                id: None,
                start: None,
                end: None,
                target: None,
                note: None,
            });
            refresh_tray(&app);
        });
    }
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn main() {
    let env_dir = std::env::var_os("TRACR_DATA_DIR").map(PathBuf::from);
    let ctx = Ctx {
        from_env: env_dir.is_some(),
        data_dir: env_dir.or_else(default_data_dir),
        runtime: Mutex::new(None),
        runtime_error: Mutex::new(None),
    };
    let app = tauri::Builder::default()
        .manage(ctx)
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            status,
            approve_roots,
            choose_root,
            remove_root,
            set_timezone,
            days,
            day_agents,
            refresh,
            export_csv,
            export_everything,
            correct,
            targets,
            manual,
            review,
            dismiss
        ])
        .setup(|app| {
            app.state::<Ctx>().start_runtime();
            let open = MenuItem::with_id(app, "open", "Open tracr", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit tracr", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let mut tray = TrayIconBuilder::with_id("tracr")
                .tooltip("tracr")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => show_main(app),
                    "timer" => toggle_timer(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            refresh_tray(app.handle());
            // Keep the elapsed time in the menu bar current.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    refresh_tray(&handle);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps collecting (AD-03); Quit stops.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("build tracr desktop");
    app.run(|app, event| match event {
        RunEvent::Exit => app.state::<Ctx>().stop_runtime("quit"),
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_main(app),
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::is_approved;

    #[test]
    fn a_symlinked_proposal_counts_as_approved_when_its_target_is() {
        let base = std::env::temp_dir().join(format!("tracr-approved-{}", std::process::id()));
        let (real, link) = (base.join("real"), base.join("link"));
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let stored = std::fs::canonicalize(&real).unwrap().display().to_string();

        assert!(is_approved(&link, std::slice::from_ref(&stored)), "symlink");
        assert!(
            is_approved(&real, std::slice::from_ref(&stored)),
            "real path"
        );
        assert!(!is_approved(&link, &[]), "nothing approved");
        assert!(!is_approved(
            &base.join("other"),
            std::slice::from_ref(&stored)
        ));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
