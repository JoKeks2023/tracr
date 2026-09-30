//! The collector runtime (architecture §4.5, §4.7, §4.8, AD-03).
//!
//! One writer thread owns the [`App`] (and with it `tracr.lock` and the
//! ledger connection) and processes [`Job`]s. Timer and watcher threads only
//! send jobs; they never read transcripts or write the ledger.
//!
//! * start: `coverage_started`, then a startup reconcile of every source
//!   (discovery first) and a derived rebuild;
//! * every 15 s: an OS-sensor sample; transitions are written as
//!   `device_*` observations; a new kernel wake schedules a wake reconcile;
//! * every 60 s: `collector_heartbeat` with each coverage key's state, and
//!   an incremental update of the derived store if something changed;
//! * file hints (debounced 500 ms): reconcile the file's source; an unknown
//!   file runs discovery (directory hint) and reads the new sources;
//! * every 5 min: the stat sweep; every 10 min: discovery;
//! * stop: `coverage_ended`.
//!
//! The derived store is updated in place (dirty units only); a full rebuild
//! happens only when it is missing or from another schema version.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use serde::Serialize;
use tracr_core::device::{DeviceEvent, DeviceSample, DeviceTracker};
use tracr_core::evidence::{CollectorFacts, IngestTrigger};
use tracr_core::ids::SourceId;
use tracr_ingest::source::{
    OS_SENSOR_KIND, RunContext, ensure_os_sensor_source, write_collector_record,
    write_device_record,
};
use tracr_ingest::{IngestEnv, ReconcileOptions, reconcile, record_failed_run, sweep};
use tracr_platform::{Clock, DeviceSensor, SystemClock, device_sensor};
use tracr_store::ledger;

use crate::{App, RootKind, read_roots};

pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(15);
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
pub const SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);
pub const DISCOVERY_INTERVAL: Duration = Duration::from_secs(10 * 60);
pub const HINT_DEBOUNCE: Duration = Duration::from_millis(500);
pub const REBUILD_MIN_INTERVAL: Duration = Duration::from_secs(60);

/// Work for the writer thread.
pub enum Job {
    Hints(Vec<PathBuf>),
    Sample(DeviceSample),
    Heartbeat,
    Sweep,
    Discovery,
    /// Watcher error or overflow: events may have been lost.
    Overflow,
    Refresh(Sender<Result<Refreshed, String>>),
    Stop(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct Refreshed {
    pub inserted: u64,
    pub sessions: u64,
    pub seconds: f64,
}

/// What the runtime is doing, for the UI.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RuntimeStatus {
    pub collecting: bool,
    pub run_id: Option<String>,
    pub keys: BTreeMap<String, String>,
    pub last_heartbeat: Option<i64>,
    pub last_rebuild: Option<i64>,
    pub last_error: Option<String>,
    pub sensor: String,
}

/// The writer side: owns the app, the run and the device tracker.
pub struct Collector {
    app: App,
    run: RunContext,
    os_source: SourceId,
    tracker: DeviceTracker,
    sensor_available: bool,
    dirty: bool,
    last_rebuild: Option<Instant>,
    status: Arc<Mutex<RuntimeStatus>>,
}

fn key_states(app: &App, sensor_available: bool) -> BTreeMap<String, String> {
    let mut keys: BTreeMap<String, String> = app
        .registry
        .adapters()
        .iter()
        .map(|a| (a.source_kind().as_str().to_owned(), "collecting".to_owned()))
        .collect();
    keys.insert(
        OS_SENSOR_KIND.to_owned(),
        if sensor_available {
            "collecting"
        } else {
            "unavailable"
        }
        .to_owned(),
    );
    keys
}

impl Collector {
    /// Begins a run: `coverage_started` for every configured key.
    pub fn begin(mut app: App, client: &str, sensor_available: bool) -> Result<Self> {
        let keys = key_states(&app, sensor_available);
        let env = IngestEnv {
            clock: &*app.clock,
            file_identity: &*app.fid,
        };
        let run = RunContext::begin_runtime(
            &mut app.conn,
            &env,
            &app.machine,
            client,
            keys.keys().cloned().collect(),
            HEARTBEAT_INTERVAL.as_millis() as i64,
            SAMPLE_INTERVAL.as_millis() as i64,
        )?;
        let os_source = ensure_os_sensor_source(&app.conn, &env, &app.machine)?.id;
        let status = Arc::new(Mutex::new(RuntimeStatus {
            collecting: true,
            run_id: Some(run.run_id.clone()),
            keys,
            ..RuntimeStatus::default()
        }));
        Ok(Self {
            app,
            run,
            os_source,
            tracker: DeviceTracker::new(),
            sensor_available,
            dirty: true,
            last_rebuild: None,
            status,
        })
    }

    pub fn status_handle(&self) -> Arc<Mutex<RuntimeStatus>> {
        self.status.clone()
    }

    pub fn app(&self) -> &App {
        &self.app
    }

    fn note_error(&self, e: &anyhow::Error) {
        if let Ok(mut s) = self.status.lock() {
            s.last_error = Some(format!("{e:#}"));
        }
    }

    /// Discovery plus a reconcile of every source (startup, wake, refresh).
    pub fn reconcile_all(&mut self, trigger: IngestTrigger) -> Result<u64> {
        if read_roots(&self.app.conn, RootKind::Provider)?.is_empty() {
            return Ok(0);
        }
        let r = self
            .app
            .reconcile_all(&mut self.run, trigger, &mut |_, _| {})?;
        if r.inserted > 0 || r.new_sources > 0 {
            self.dirty = true;
        }
        Ok(r.inserted)
    }

    /// Brings the derived store up to date (incremental where possible).
    pub fn rebuild(&mut self) -> Result<u64> {
        let r = self.app.update_derived()?;
        self.dirty = false;
        self.last_rebuild = Some(Instant::now());
        if let Ok(mut s) = self.status.lock() {
            s.last_rebuild = Some(self.app.clock.now().0);
        }
        Ok(r.sessions)
    }

    fn rebuild_if_due(&mut self) -> Result<()> {
        let due = self
            .last_rebuild
            .is_none_or(|t| t.elapsed() >= REBUILD_MIN_INTERVAL);
        if self.dirty && due {
            self.rebuild()?;
        }
        Ok(())
    }

    fn reconcile_source(
        &mut self,
        source: &ledger::SourceRow,
        trigger: IngestTrigger,
    ) -> Result<()> {
        let env = IngestEnv {
            clock: &*self.app.clock,
            file_identity: &*self.app.fid,
        };
        match reconcile(
            &mut self.app.conn,
            &env,
            &self.app.registry,
            &mut self.run,
            source,
            trigger,
            &ReconcileOptions::default(),
        ) {
            Ok(r) => {
                if r.inserted > 0 || r.gap.is_some() {
                    self.dirty = true;
                }
            }
            Err(e) => {
                record_failed_run(&mut self.app.conn, &env, &mut self.run, source, trigger, &e)?;
            }
        }
        Ok(())
    }

    /// File hints: reconcile known sources; unknown files mean new streams.
    pub fn hints(&mut self, paths: &[PathBuf]) -> Result<()> {
        let mut unknown = false;
        let mut seen: HashSet<String> = HashSet::new();
        for p in paths {
            let text = p.to_string_lossy().into_owned();
            match ledger::source_for_path(&self.app.conn, &text)? {
                Some(source) if seen.insert(source.id.to_string()) => {
                    self.reconcile_source(&source, IngestTrigger::Hint)?;
                }
                Some(_) => {}
                None => unknown = true,
            }
        }
        if unknown {
            self.discover_new(IngestTrigger::DirHint)?;
        }
        Ok(())
    }

    /// Discovery, then a first read of every source it created.
    pub fn discover_new(&mut self, trigger: IngestTrigger) -> Result<()> {
        let roots: Vec<PathBuf> = read_roots(&self.app.conn, RootKind::Provider)?
            .into_iter()
            .map(PathBuf::from)
            .collect();
        if roots.is_empty() {
            return Ok(());
        }
        let env = IngestEnv {
            clock: &*self.app.clock,
            file_identity: &*self.app.fid,
        };
        tracr_ingest::discover(
            &mut self.app.conn,
            &env,
            &self.app.registry,
            &mut self.run,
            &self.app.machine,
            &roots,
            trigger,
        )?;
        for source in ledger::unread_sources(&self.app.conn)? {
            if self.app.registry.for_kind(&source.kind).is_some() {
                self.reconcile_source(&source, trigger)?;
            }
        }
        Ok(())
    }

    pub fn sweep(&mut self) -> Result<()> {
        let env = IngestEnv {
            clock: &*self.app.clock,
            file_identity: &*self.app.fid,
        };
        let r = sweep(
            &mut self.app.conn,
            &env,
            &self.app.registry,
            &mut self.run,
            &ReconcileOptions::default(),
        )?;
        if r.reconciles
            .iter()
            .any(|x| x.inserted > 0 || x.gap.is_some())
        {
            self.dirty = true;
        }
        Ok(())
    }

    /// Records device transitions; returns whether the device just woke.
    pub fn sample(&mut self, s: &DeviceSample) -> Result<bool> {
        let transitions = self.tracker.observe(s);
        if transitions.is_empty() {
            return Ok(false);
        }
        let woke = transitions.iter().any(|t| t.event == DeviceEvent::Resumed);
        let env = IngestEnv {
            clock: &*self.app.clock,
            file_identity: &*self.app.fid,
        };
        let tx = self.app.conn.transaction()?;
        for t in transitions {
            write_device_record(&tx, &env, &mut self.run, &self.os_source, t)?;
        }
        tx.commit()?;
        self.dirty = true;
        Ok(woke)
    }

    pub fn heartbeat(&mut self) -> Result<()> {
        let keys = key_states(&self.app, self.sensor_available);
        let env = IngestEnv {
            clock: &*self.app.clock,
            file_identity: &*self.app.fid,
        };
        write_collector_record(
            &self.app.conn,
            &env,
            &mut self.run,
            CollectorFacts::CollectorHeartbeat { keys: keys.clone() },
            IngestTrigger::Startup,
            None,
        )?;
        if let Ok(mut s) = self.status.lock() {
            s.keys = keys;
            s.last_heartbeat = Some(self.app.clock.now().0);
        }
        self.rebuild_if_due()
    }

    /// `coverage_ended` (a clean stop).
    pub fn end(mut self, reason: &str) -> Result<()> {
        let env = IngestEnv {
            clock: &*self.app.clock,
            file_identity: &*self.app.fid,
        };
        write_collector_record(
            &self.app.conn,
            &env,
            &mut self.run,
            CollectorFacts::CoverageEnded {
                reason: reason.to_owned(),
            },
            IngestTrigger::Startup,
            None,
        )?;
        if let Ok(mut s) = self.status.lock() {
            s.collecting = false;
        }
        Ok(())
    }

    fn handle(&mut self, job: Job) -> Result<()> {
        match job {
            Job::Hints(paths) => self.hints(&paths),
            Job::Sample(s) => {
                if self.sample(&s)? {
                    self.reconcile_all(IngestTrigger::Wake)?;
                }
                Ok(())
            }
            Job::Heartbeat => self.heartbeat(),
            Job::Sweep => self.sweep(),
            Job::Discovery => self.discover_new(IngestTrigger::Discovery),
            Job::Overflow => self.reconcile_all(IngestTrigger::Overflow).map(|_| ()),
            Job::Refresh(reply) => {
                let started = Instant::now();
                let result = (|| {
                    let inserted = self.reconcile_all(IngestTrigger::Sweep)?;
                    let sessions = self.rebuild()?;
                    Ok::<_, anyhow::Error>(Refreshed {
                        inserted,
                        sessions,
                        seconds: started.elapsed().as_secs_f64(),
                    })
                })();
                let _ = reply.send(result.map_err(|e| format!("{e:#}")));
                Ok(())
            }
            Job::Stop(_) => Ok(()),
        }
    }
}

/// Requests to a running collector; cheap to clone, never holds the writer.
#[derive(Clone)]
pub struct RuntimeHandle {
    jobs: Sender<Job>,
    status: Arc<Mutex<RuntimeStatus>>,
}

impl RuntimeHandle {
    pub fn status(&self) -> RuntimeStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Reconcile everything now and rebuild; blocks until the writer is done.
    pub fn refresh(&self) -> Result<Refreshed> {
        let (tx, rx) = mpsc::channel();
        self.jobs
            .send(Job::Refresh(tx))
            .map_err(|_| anyhow!("collector stopped"))?;
        rx.recv()
            .map_err(|_| anyhow!("collector stopped"))?
            .map_err(|e| anyhow!(e))
    }
}

/// A running collector: writer, timer and watcher threads.
pub struct Runtime {
    jobs: Sender<Job>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    status: Arc<Mutex<RuntimeStatus>>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl Runtime {
    /// Takes the writer lock and starts collecting. Fails if another tracr
    /// process holds the lock (AD-03).
    pub fn start(data_dir: &Path, client: &'static str) -> Result<Self> {
        let app = App::open_writer(data_dir, Box::new(SystemClock))?.with_client(client);
        let sensor: Box<dyn DeviceSensor> = device_sensor();
        let sensor_available = sensor.sample(SystemClock.now()).idle_ms.is_some();
        let mut collector = Collector::begin(app, client, sensor_available)?;
        let status = collector.status_handle();
        if let Ok(mut s) = status.lock() {
            s.sensor = sensor.name().to_owned();
        }
        let roots: Vec<PathBuf> = read_roots(&collector.app().conn, RootKind::Provider)?
            .into_iter()
            .map(PathBuf::from)
            .collect();

        let (jobs, rx): (Sender<Job>, Receiver<Job>) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));

        // Filesystem hints: collected here, flushed by the timer (debounce).
        let pending: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
        let overflow = Arc::new(AtomicBool::new(false));
        let watcher = {
            let pending = pending.clone();
            let overflow = overflow.clone();
            let mut w =
                notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
                    Ok(ev) => {
                        if ev.need_rescan() {
                            overflow.store(true, Ordering::SeqCst);
                        }
                        if let Ok(mut p) = pending.lock() {
                            p.extend(
                                ev.paths
                                    .into_iter()
                                    .filter(|p| p.extension().is_some_and(|e| e == "jsonl")),
                            );
                        }
                    }
                    Err(_) => overflow.store(true, Ordering::SeqCst),
                })?;
            for r in &roots {
                use notify::Watcher;
                // A missing root is not fatal: sweep and discovery still run.
                let _ = w.watch(r, notify::RecursiveMode::Recursive);
            }
            Some(w)
        };

        let writer = {
            let status = status.clone();
            std::thread::Builder::new()
                .name("tracr-writer".into())
                .spawn(move || {
                    let start = (|| {
                        collector.reconcile_all(IngestTrigger::Startup)?;
                        collector.rebuild()?;
                        collector.heartbeat()
                    })();
                    if let Err(e) = start {
                        collector.note_error(&e);
                    }
                    let mut reason = "channel closed".to_owned();
                    while let Ok(job) = rx.recv() {
                        if let Job::Stop(r) = &job {
                            reason = r.clone();
                            break;
                        }
                        if let Err(e) = collector.handle(job) {
                            collector.note_error(&e);
                        }
                    }
                    if let Err(e) = collector.end(&reason)
                        && let Ok(mut s) = status.lock()
                    {
                        s.last_error = Some(format!("{e:#}"));
                    }
                })?
        };

        let timer = {
            let jobs = jobs.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("tracr-timer".into())
                .spawn(move || {
                    let t0 = Instant::now();
                    let (mut sample, mut beat, mut swp, mut disc, mut flush) = (t0, t0, t0, t0, t0);
                    while !stop.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(100));
                        let now = Instant::now();
                        let send = |job: Job| jobs.send(job).is_ok();
                        if now - flush >= HINT_DEBOUNCE {
                            flush = now;
                            if overflow.swap(false, Ordering::SeqCst) && !send(Job::Overflow) {
                                break;
                            }
                            let batch: Vec<PathBuf> = pending
                                .lock()
                                .map(|mut p| p.drain().collect())
                                .unwrap_or_default();
                            if !batch.is_empty() && !send(Job::Hints(batch)) {
                                break;
                            }
                        }
                        if now - sample >= SAMPLE_INTERVAL {
                            sample = now;
                            if !send(Job::Sample(sensor.sample(SystemClock.now()))) {
                                break;
                            }
                        }
                        if now - beat >= HEARTBEAT_INTERVAL {
                            beat = now;
                            if !send(Job::Heartbeat) {
                                break;
                            }
                        }
                        if now - swp >= SWEEP_INTERVAL {
                            swp = now;
                            if !send(Job::Sweep) {
                                break;
                            }
                        }
                        if now - disc >= DISCOVERY_INTERVAL {
                            disc = now;
                            if !send(Job::Discovery) {
                                break;
                            }
                        }
                    }
                })?
        };
        // The first sample is recorded right away, not after 15 s, and is
        // taken after `coverage_started` so it belongs to this run.
        let _ = jobs.send(Job::Sample(device_sensor().sample(SystemClock.now())));

        Ok(Self {
            jobs,
            stop,
            threads: vec![writer, timer],
            status,
            _watcher: watcher,
        })
    }

    pub fn status(&self) -> RuntimeStatus {
        self.handle().status()
    }

    /// Reconcile everything now and rebuild; waits for the writer.
    pub fn refresh(&self) -> Result<Refreshed> {
        self.handle().refresh()
    }

    /// A cloneable handle for requests from other threads.
    pub fn handle(&self) -> RuntimeHandle {
        RuntimeHandle {
            jobs: self.jobs.clone(),
            status: self.status.clone(),
        }
    }

    /// Stops the threads and writes `coverage_ended`.
    pub fn stop(mut self, reason: &str) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.jobs.send(Job::Stop(reason.to_owned()));
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}
