//! tracr desktop host (architecture §10, §13.4): the read-only Day view.
//!
//! The window talks only to `tracr-app` through the commands below. It gets
//! no filesystem, shell or HTTP capability (AD-12); transcript content is
//! never rendered because none is stored (PS-03).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use serde::Serialize;
use tracr_app::day::{DayAgents, DayList};
use tracr_app::{App, Reader, SystemClock, default_data_dir};

struct Ctx {
    data_dir: Option<PathBuf>,
    from_env: bool,
}

impl Ctx {
    fn dir(&self) -> Result<PathBuf, String> {
        self.data_dir
            .clone()
            .ok_or_else(|| "no data directory: set TRACR_DATA_DIR".to_owned())
    }
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[derive(Serialize)]
struct Status {
    data_dir: Option<String>,
    data_dir_from_env: bool,
    ledger_present: bool,
}

#[tauri::command]
fn status(ctx: tauri::State<'_, Ctx>) -> Status {
    Status {
        data_dir: ctx.data_dir.as_ref().map(|d| d.display().to_string()),
        data_dir_from_env: ctx.from_env,
        ledger_present: ctx
            .data_dir
            .as_ref()
            .is_some_and(|d| d.join("ledger.sqlite").exists()),
    }
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

#[derive(Serialize)]
struct Refreshed {
    inserted: u64,
    sessions: u64,
    seconds: f64,
}

/// Import from the approved roots, then rebuild the derived store. Takes
/// the single-writer lock for its duration (AD-03); fails cleanly if the
/// CLI holds it.
#[tauri::command]
async fn refresh(ctx: tauri::State<'_, Ctx>) -> Result<Refreshed, String> {
    let dir = ctx.dir()?;
    tauri::async_runtime::spawn_blocking(move || {
        let started = std::time::Instant::now();
        let mut app = App::open_writer(&dir, Box::new(SystemClock))?;
        let imported = app.import(&mut |_, _| {})?;
        let rebuilt = app.rebuild()?;
        Ok::<_, anyhow::Error>(Refreshed {
            inserted: imported.inserted,
            sessions: rebuilt.sessions,
            seconds: started.elapsed().as_secs_f64(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(err)
}

fn main() {
    let env_dir = std::env::var_os("TRACR_DATA_DIR").map(PathBuf::from);
    let ctx = Ctx {
        from_env: env_dir.is_some(),
        data_dir: env_dir.or_else(default_data_dir),
    };
    tauri::Builder::default()
        .manage(ctx)
        .invoke_handler(tauri::generate_handler![status, days, day_agents, refresh])
        .run(tauri::generate_context!())
        .expect("run tracr desktop");
}
