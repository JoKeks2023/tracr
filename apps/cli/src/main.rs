//! `tracr` command-line host (T-0004): `roots add`, `import`, `sources`, `stats`.

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
