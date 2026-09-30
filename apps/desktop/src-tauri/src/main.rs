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

    fn approved_roots(&self) -> Vec<String> {
        self.data_dir
            .as_ref()
            .and_then(|d| Reader::open(d).ok())
            .and_then(|r| r.stats().ok())
            .map(|s| s.approved_provider_roots)
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
    proposed_roots: Vec<ProposedRoot>,
    /// `collecting`, `elsewhere` (another tracr process holds the lock), `stopped` or `not_configured`.
    collector: &'static str,
    collector_detail: Option<String>,
    runtime: Option<RuntimeStatus>,
}

#[tauri::command]
fn status(ctx: tauri::State<'_, Ctx>) -> Status {
    let approved = ctx.approved_roots();
    let runtime = ctx
        .runtime
        .lock()
        .expect("runtime lock")
        .as_ref()
        .map(Runtime::status);
    let error = ctx.runtime_error.lock().expect("error lock").clone();
    let collector = match (&runtime, &error) {
        (Some(r), _) if r.collecting => "collecting",
        (_, Some(e)) if e.contains("another tracr process") => "elsewhere",
        _ if approved.is_empty() => "not_configured",
        _ => "stopped",
    };
    Status {
        data_dir: ctx.data_dir.as_ref().map(|d| d.display().to_string()),
        data_dir_from_env: ctx.from_env,
        ledger_present: ctx
            .data_dir
            .as_ref()
            .is_some_and(|d| d.join("ledger.sqlite").exists()),
        proposed_roots: proposed_provider_roots()
            .into_iter()
            .map(|p| {
                let path = p.display().to_string();
                ProposedRoot {
                    approved: approved.contains(&path),
                    path,
                }
            })
            .collect(),
        approved_roots: approved,
        collector,
        collector_detail: error,
        runtime,
    }
}

/// Approves one proposed provider root (PS-02: an explicit user action).
/// The collector restarts so the new root is watched.
#[tauri::command]
async fn approve_root(ctx: tauri::State<'_, Ctx>, path: String) -> Result<(), String> {
    if !proposed_provider_roots()
        .iter()
        .any(|p| p.display().to_string() == path)
    {
        return Err("only a proposed provider root can be approved here".into());
    }
    let dir = ctx.dir()?;
    ctx.stop_runtime("root approved; restarting");
    {
        let mut app = App::open_writer(&dir, Box::new(SystemClock))
            .map_err(err)?
            .with_client(CLIENT);
        app.add_root(RootKind::Provider, &PathBuf::from(&path))
            .map_err(err)?;
    }
    ctx.start_runtime();
    Ok(())
}

#[tauri::command]
fn days(ctx: tauri::State<'_, Ctx>) -> Result<DayList, String> {
    Reader::open(&ctx.dir()?)
        .and_then(|r| r.days())
        .map_err(err)
}

#[tauri::command]
fn day_agents(ctx: tauri::State<'_, Ctx>, day: String) -> Result<DayAgents, String> {
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
            approve_root,
            days,
            day_agents,
            refresh,
            export_csv,
            export_everything
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
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
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
