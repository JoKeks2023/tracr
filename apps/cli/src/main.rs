//! `tracr` command-line host: `roots add`, `import`, `sources`, `stats`,
//! `rebuild`, `days`, `sessions --day`.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{Args, Parser, Subcommand};
use tracr_app::{App, Reader, RootKind, RootsChange, SystemClock, UtcMillis, default_data_dir};

#[derive(Parser)]
#[command(
    name = "tracr",
    version,
    about = "tracr: local development diary and time ledger"
)]
struct Cli {
    /// Data directory holding ledger.sqlite (default: the platform data dir).
    #[arg(long, env = "TRACR_DATA_DIR", global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage approved roots (PS-02).
    Roots {
        #[command(subcommand)]
        command: RootsCommand,
    },
    /// Discover and reconcile every source under the approved provider roots.
    Import,
    /// List sources with their cursors and observation counts.
    Sources {
        /// Only sources of this kind (e.g. claude_transcript, collector).
        #[arg(long)]
        kind: Option<String>,
    },
    /// Ledger statistics: per-type counts, quarantine, gaps, versions.
    Stats {
        #[arg(long)]
        json: bool,
    },
    /// Export CSV (see docs/export.md).
    Export {
        #[command(subcommand)]
        what: ExportWhat,
    },
    /// Collect live until Ctrl-C: transcript hints, sweeps, the OS sensor, heartbeats.
    Run,
    /// Rebuild derived.sqlite from the ledger (normalization, copy election, agent timelines).
    Rebuild {
        /// Update only the dirty units in place (a full rebuild if that is not possible).
        #[arg(long)]
        incremental: bool,
    },
    /// Days with derived agent activity (reporting timezone).
    Days,
    /// Agent sessions of one day, each listed individually. Agent time is never human time.
    Sessions {
        /// Reporting day, YYYY-MM-DD (default: the latest day with data).
        #[arg(long)]
        day: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ExportWhat {
    /// Human time: one row per interval; counted and unknown minutes in separate columns.
    Timesheet {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Everything: one CSV per data set into a folder, with a README.
    All {
        /// Folder to write into (created if missing).
        #[arg(long)]
        out: PathBuf,
    },
    /// One row per day × agent session × actor: execution, turns, tool calls, tokens.
    AgentSessions {
        /// First day, YYYY-MM-DD (inclusive).
        #[arg(long)]
        from: String,
        /// Last day, YYYY-MM-DD (inclusive).
        #[arg(long)]
        to: String,
        /// Write to this file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum RootsCommand {
    /// Approve a provider root or a repository root.
    Add(RootsAdd),
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct RootsAdd {
    /// A provider root, e.g. ~/.claude/projects.
    #[arg(long)]
    provider: Option<PathBuf>,
    /// A repository root under which git may be probed (used from S1.3).
    #[arg(long)]
    repo: Option<PathBuf>,
}

fn data_dir(cli: &Cli) -> Result<PathBuf> {
    cli.data_dir
        .clone()
        .or_else(default_data_dir)
        .ok_or_else(|| anyhow!("no default data directory on this platform; pass --data-dir"))
}

fn table(title: &str, rows: &[(String, i64)]) {
    println!("{title}");
    if rows.is_empty() {
        println!("  (none)");
    }
    for (k, n) in rows {
        println!("  {n:>9}  {k}");
    }
}

fn hm(ms: i64, zone: &str) -> String {
    use std::str::FromStr;
    let tz = jiff::tz::TimeZone::get(zone).unwrap_or(jiff::tz::TimeZone::UTC);
    jiff::Timestamp::from_millisecond(ms)
        .map(|t| t.to_zoned(tz).strftime("%H:%M").to_string())
        .unwrap_or_else(|_| String::from_str("?").unwrap())
}

fn minutes(ms: i64) -> String {
    format!("{}m", (ms + 30_000) / 60_000)
}

fn tokens(t: tracr_app::day::TokenCount) -> String {
    let n = t.sum as f64;
    let base = if n >= 1e6 {
        format!("{:.2}M", n / 1e6)
    } else if n >= 1e3 {
        format!("{:.1}k", n / 1e3)
    } else {
        format!("{}", t.sum)
    };
    if t.unknown_records > 0 {
        format!("{base}+{}?", t.unknown_records)
    } else {
        base
    }
}

/// An estimate as text: unpriced is never printed as zero; a partial estimate is a floor.
fn est(c: &tracr_app::day::CostView, currency: &str) -> String {
    match (c.amount_nano, c.status()) {
        (_, None) => "-".into(),
        (None, _) => "unpriced".into(),
        (Some(n), Some(status)) => format!(
            "{}{} {currency}",
            if status == "priced" { "" } else { ">= " },
            tracr_core::money::Money::new(n.into(), currency).display(2)
        ),
    }
}

fn print_day(v: &tracr_app::day::DayAgents) {
    let tz = v.timezone.as_str();
    println!("{}  ({})  {} agent sessions", v.day, tz, v.sessions.len());
    println!("agent execution time is not human time; sessions overlap and are never merged\n");
    for s in &v.sessions {
        let span = match (s.first_evidence_at, s.last_evidence_at) {
            (Some(a), Some(b)) => {
                format!("{}–{}", hm(a.max(v.start), tz), hm(b.min(v.end - 1), tz))
            }
            _ => "-".into(),
        };
        println!(
            "■ {}  [{}]  {}{}",
            s.label.as_deref().unwrap_or("(no cwd)"),
            &s.native_session_id[..8.min(s.native_session_id.len())],
            span,
            s.git_branch
                .as_deref()
                .map(|b| format!("  branch {b}"))
                .unwrap_or_default()
        );
        if !s.engagement_instants.is_empty() {
            let shown: Vec<String> = s
                .engagement_instants
                .iter()
                .take(12)
                .map(|&t| hm(t, tz))
                .collect();
            println!(
                "    human prompts: {}{}",
                shown.join(" "),
                if s.engagement_instants.len() > 12 {
                    format!(" … ({} total)", s.engagement_instants.len())
                } else {
                    String::new()
                }
            );
        }
        for a in &s.actors {
            let who = match (a.actor_kind.as_str(), &a.native_agent_id) {
                ("known", Some(id)) => format!("agent {}", &id[..12.min(id.len())]),
                (k, _) => format!("{k} actor"),
            };
            println!(
                "    {:<20} exec {:>5}  {:>3} segments  {:>3} turns  {:>4} tool calls",
                who,
                minutes(a.execution_ms),
                a.segments.len(),
                a.turns,
                a.tool_calls
            );
        }
        let currency = v.cost.pricing.currency.as_str();
        for u in &s.usage {
            println!(
                "    tokens {:<28} in {:>8}  cache+ {:>8}  cache↺ {:>8}  out {:>8}  est. {:>12}  ({} records{})",
                u.model.as_deref().unwrap_or("(model unknown)"),
                tokens(u.input_fresh),
                tokens(u.cache_creation),
                tokens(u.cache_read),
                tokens(u.output),
                est(&u.cost, currency),
                u.records,
                if u.identified { "" } else { ", unidentified" }
            );
        }
        for r in &s.reported {
            println!(
                "    provider-reported {} {} {} (its own running total, not spend; never summed)",
                r.source_field, r.value_as_stated, r.currency
            );
        }
        if !s.diagnostics.is_empty() {
            let d: Vec<String> = s
                .diagnostics
                .iter()
                .map(|(k, n)| format!("{k}={n}"))
                .collect();
            println!("    diagnostics: {}", d.join(", "));
        }
    }
    let c = &v.cost;
    println!();
    println!(
        "estimated API cost: {} (API-equivalent estimate, not spend; catalog {} as of {})",
        est(&c.identified, &c.pricing.currency),
        c.pricing.catalog_version,
        c.pricing.oldest_fetched_at.as_deref().unwrap_or("unknown"),
    );
    if c.identified.unpriced_tokens > 0 {
        println!(
            "    {} tokens unpriced: {}",
            c.identified.unpriced_tokens,
            c.identified.reasons.join(", ")
        );
    }
    if c.spanning_records > 0 {
        println!(
            "    usage spanning the day boundary, not included: {} records, {}",
            c.spanning_records,
            est(&c.spanning, &c.pricing.currency)
        );
    }
    let h = &v.human;
    println!(
        "human time: counted {} (inferred, provisional) · unknown {} (not counted){}",
        minutes(h.counted_ms),
        minutes(h.unknown_ms),
        if h.unknown_by_reason.is_empty() {
            String::new()
        } else {
            format!(
                " [{}]",
                h.unknown_by_reason
                    .iter()
                    .map(|(r, ms)| format!("{r} {}", minutes(*ms)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    );
    if let Some((a, b)) = h.evidence_span {
        println!(
            "evidence span {}–{} = {} (an upper bound, not a total)",
            hm(a, tz),
            hm(b, tz),
            minutes(b - a)
        );
    }
    for a in &h.atoms {
        println!(
            "    {}–{}  {:<8} {}",
            hm(a.start, tz),
            hm(a.end, tz),
            a.status,
            a.unknown_reason.as_deref().unwrap_or(&a.duration_basis)
        );
    }
    let device: BTreeMap<&str, i64> = v.device.iter().fold(BTreeMap::new(), |mut m, d| {
        *m.entry(d.state.as_str()).or_default() += d.end - d.start;
        m
    });
    println!(
        "device: {}",
        device
            .iter()
            .map(|(s, ms)| format!("{s} {}", minutes(*ms)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!();
    for p in &v.pending {
        println!("not derived yet — {}: {}", p.lane, p.reason);
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = data_dir(&cli)?;
    match &cli.command {
        Command::Roots {
            command: RootsCommand::Add(add),
        } => {
            let (kind, path) = match (&add.provider, &add.repo) {
                (Some(p), None) => (RootKind::Provider, p),
                (None, Some(p)) => (RootKind::Repository, p),
                _ => unreachable!("clap enforces exactly one"),
            };
            let mut app = App::open_writer(&dir, Box::new(SystemClock))?;
            match app.add_root(kind, path)? {
                RootsChange::Added(roots) => println!("approved; {kind:?} roots now: {roots:?}"),
                RootsChange::AlreadyApproved(roots) => {
                    println!("already approved; {kind:?} roots: {roots:?}")
                }
            }
        }
        Command::Import => {
            let mut app = App::open_writer(&dir, Box::new(SystemClock))?;
            let started = std::time::Instant::now();
            let mut progress = |i: u64, n: u64| {
                if i == 1 || i.is_multiple_of(50) || i == n {
                    eprint!("\rreconciling source {i}/{n}");
                    let _ = std::io::stderr().flush();
                }
            };
            let r = app.import(&mut progress)?;
            eprintln!();
            println!(
                "import run {} in {:.1}s (data dir {})",
                r.run_id,
                started.elapsed().as_secs_f64(),
                dir.display()
            );
            println!(
                "discovery: {} files seen, {} claimed, {} unclaimed, {} new sources, {} unreadable dirs{}",
                r.files_seen,
                r.files_claimed,
                r.files_unclaimed,
                r.new_sources,
                r.unreadable_dirs,
                if r.missing_roots.is_empty() {
                    String::new()
                } else {
                    format!(", missing roots {:?}", r.missing_roots)
                }
            );
            println!(
                "reconciled {} sources: {} inserted, {} duplicates, {} quarantined, {} key conflicts, {} full re-reads, {} ended, {} missing, {} with a partial trailing line",
                r.sources_reconciled,
                r.inserted,
                r.duplicates,
                r.quarantined,
                r.key_conflicts,
                r.full_rereads,
                r.ended,
                r.missing,
                r.incomplete_trailing
            );
            println!(
                "provider observations: {} before, {} after (+{})",
                r.provider_observations_before,
                r.provider_observations_after,
                r.provider_observations_after - r.provider_observations_before
            );
            let by_kind: Vec<(String, i64)> = r
                .inserted_by_kind
                .iter()
                .map(|(k, v)| (k.clone(), *v as i64))
                .collect();
            table("inserted by kind:", &by_kind);
            let ignored: Vec<(String, i64)> = r
                .ignored_by_type
                .iter()
                .map(|(k, v)| (k.clone(), *v as i64))
                .collect();
            table(
                "ignored lines by record type (no observation written):",
                &ignored,
            );
            if !r.unvalidated_versions.is_empty() {
                println!(
                    "producer versions not fixture-validated: {:?}",
                    r.unvalidated_versions
                );
            }
            for g in &r.gaps {
                println!(
                    "coverage gap: source {} {} → {} ({})",
                    g.source_id, g.start, g.end, g.reason
                );
            }
            for e in &r.errors {
                println!("error: {e}");
            }
        }
        Command::Sources { kind } => {
            let reader = Reader::open(&dir)?;
            let rows = reader.sources(kind.as_deref())?;
            println!(
                "{:<18} {:>7} {:>11} {:>11} {:<24} {:<24} stream",
                "kind", "obs", "offset", "size", "first record", "last record"
            );
            for s in &rows {
                let fmt_t =
                    |t: Option<UtcMillis>| t.map(|t| t.to_rfc3339()).unwrap_or_else(|| "-".into());
                println!(
                    "{:<18} {:>7} {:>11} {:>11} {:<24} {:<24} {}{}",
                    s.source.kind,
                    s.observations,
                    s.byte_offset
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    s.size_at_read
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    fmt_t(s.first_occurred_at),
                    fmt_t(s.last_occurred_at),
                    s.source.stream_key,
                    if s.ended_at.is_some() {
                        "  [ended]"
                    } else {
                        ""
                    }
                );
            }
            println!("{} sources", rows.len());
        }
        Command::Export {
            what: ExportWhat::All { out },
        } => {
            for (file, rows) in Reader::open(&dir)?.export_everything(out)? {
                println!("{rows:>8}  {file}");
            }
            println!("wrote {}", out.display());
        }
        Command::Export { what } => {
            let reader = Reader::open(&dir)?;
            let (csv, out) = match what {
                ExportWhat::AgentSessions { from, to, out } => (
                    tracr_app::export::agent_sessions_csv(&reader, from, to)?,
                    out,
                ),
                ExportWhat::Timesheet { from, to, out } => {
                    (tracr_app::export::timesheet_csv(&reader, from, to)?, out)
                }
                ExportWhat::All { .. } => unreachable!("handled above"),
            };
            match out {
                Some(path) => {
                    std::fs::write(path, &csv)?;
                    eprintln!(
                        "wrote {} rows to {}",
                        csv.lines().count() - 1,
                        path.display()
                    );
                }
                None => print!("{csv}"),
            }
        }
        Command::Run => {
            let runtime = tracr_app::runtime::Runtime::start(&dir, "tracr-cli")?;
            let (tx, rx) = std::sync::mpsc::channel();
            ctrlc::set_handler(move || {
                let _ = tx.send(());
            })?;
            let s = runtime.status();
            println!(
                "collecting into {} (run {}); sensor {}; keys {:?}",
                dir.display(),
                s.run_id.as_deref().unwrap_or("-"),
                s.sensor,
                s.keys
            );
            println!("press Ctrl-C to stop (writes coverage_ended)");
            let _ = rx.recv();
            let s = runtime.status();
            if let Some(e) = s.last_error {
                eprintln!("last error: {e}");
            }
            runtime.stop("interrupted");
            println!("stopped");
        }
        Command::Rebuild { incremental } => {
            let mut app = App::open_writer(&dir, Box::new(SystemClock))?;
            let started = std::time::Instant::now();
            let r = if *incremental {
                app.update_derived()?
            } else {
                app.rebuild()?
            };
            println!(
                "rebuilt derived store in {:.1}s (as_of {}, ledger seq ≤ {})",
                started.elapsed().as_secs_f64(),
                r.as_of,
                r.obs_watermark
            );
            println!(
                "read {} observations in {} streams ({} skipped); {} later versions of a record key not elected",
                r.observations_read, r.streams, r.streams_skipped, r.record_versions_not_elected
            );
            println!(
                "{} sessions, {} actors, {} turns, {} tool calls, {} execution segments, {} engagement instants",
                r.sessions, r.actors, r.turns, r.tool_calls, r.segments, r.engagement_instants
            );
            println!(
                "{} events ({} copied context); {} copied records",
                r.events, r.copied_events, r.copied_records
            );
            println!(
                "{} usage records: {} duplicates of another stream's occurrence, {} unidentified",
                r.usage_records, r.usage_duplicates, r.usage_unidentified
            );
            let diag: Vec<(String, i64)> = r
                .diagnostics
                .iter()
                .map(|(k, v)| (k.clone(), *v as i64))
                .collect();
            table("diagnostics:", &diag);
        }
        Command::Days => {
            let d = Reader::open(&dir)?.days()?;
            println!(
                "timezone {}{}",
                d.timezone,
                if d.timezone_from_setting {
                    ""
                } else {
                    " (system zone; no timezone setting)"
                }
            );
            for day in &d.days {
                println!("  {day}");
            }
            println!("{} days", d.days.len());
        }
        Command::Sessions { day, json } => {
            let reader = Reader::open(&dir)?;
            let day = match day {
                Some(d) => d.clone(),
                None => reader.days()?.days.first().cloned().ok_or_else(|| {
                    anyhow!("no days with data; run `tracr import` and `tracr rebuild`")
                })?,
            };
            let v = reader.day_agents(&day)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
            }
            print_day(&v);
        }
        Command::Stats { json } => {
            let stats = Reader::open(&dir)?.stats()?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
                return Ok(());
            }
            println!(
                "machine {}  schema v{}  data dir {}",
                stats.machine.as_deref().unwrap_or("-"),
                stats.schema_version,
                dir.display()
            );
            println!(
                "approved provider roots: {:?}",
                stats.approved_provider_roots
            );
            println!(
                "approved repository roots: {:?}",
                stats.approved_repository_roots
            );
            table("sources by kind:", &stats.sources_by_kind);
            println!("  ended sources: {}", stats.ended_sources);
            table("observations by channel:", &stats.observations_by_channel);
            table(
                "transcript observations by kind/subtype:",
                &stats.transcript_kinds,
            );
            table("parse status:", &stats.parse_status);
            table("quarantined (native type / reason):", &stats.quarantined);
            table(
                "producer version  manifest range:",
                &stats.producer_versions,
            );
            table(
                "ignored lines by record type (summed over runs):",
                &stats.ignored_lines,
            );
            table("collector records:", &stats.collector_records);
            println!("coverage gaps: {}", stats.coverage_gaps.len());
            for g in &stats.coverage_gaps {
                println!(
                    "  {} → {}  {}  source {}",
                    g.start, g.end, g.reason, g.source_id
                );
            }
            let u = stats.usage_records;
            println!(
                "usage-bearing records: {} ({} distinct request/message identities, {} without identity); \
                 record counts only, not token totals: streaming re-emissions are elected in S1.3",
                u.usage_bearing_observations, u.distinct_identities, u.without_identity
            );
        }
    }
    Ok(())
}
