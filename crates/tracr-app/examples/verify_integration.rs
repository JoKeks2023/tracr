//! Checks the T-0020..T-0029 work on a COPY of a real data directory:
//! full rebuild, compactions, suggestions, search and setup facts.
//! Prints counts only, no names or paths.
//!
//! `cargo run -p tracr-app --example verify_integration -- <copied-data-dir>`

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use tracr_app::search::SessionQuery;
use tracr_app::{App, Reader, SystemClock};

fn main() -> anyhow::Result<()> {
    let dir = PathBuf::from(std::env::args().nth(1).expect("copied data dir"));
    let t = Instant::now();
    let mut app = App::open_writer(&dir, Box::new(SystemClock))?;
    let r = app.rebuild()?;
    println!(
        "full rebuild: {:.1}s, {} sessions",
        t.elapsed().as_secs_f64(),
        r.sessions
    );
    drop(app);

    let reader = Reader::open(&dir)?;
    let days = reader.days()?;
    let from = days.days.last().cloned().unwrap_or_default();
    let to = days.days.first().cloned().unwrap_or_default();
    let t = Instant::now();
    let list = reader.sessions(&from, &to)?;
    println!(
        "sessions {from}..{to}: {} (read in {:.1}s)",
        list.sessions.len(),
        t.elapsed().as_secs_f64()
    );
    let compacted = list.sessions.iter().filter(|s| s.compactions > 0).count();
    let compactions: i64 = list.sessions.iter().map(|s| s.compactions).sum();
    let suggested = list
        .sessions
        .iter()
        .filter(|s| !s.suggestions.is_empty())
        .count();
    let keys: std::collections::BTreeSet<&str> = list
        .sessions
        .iter()
        .flat_map(|s| s.suggestions.iter().map(|g| g.target.as_str()))
        .collect();
    println!("compactions: {compactions} in {compacted} sessions");
    println!(
        "suggestions: {} sessions, {} distinct keys",
        suggested,
        keys.len()
    );

    let t = Instant::now();
    let q = SessionQuery {
        from: from.clone(),
        to: to.clone(),
        ..SessionQuery::default()
    };
    let s = reader.search_sessions(&q)?;
    println!(
        "search: total {}, facets providers {} branches {} models {} tools {} ({:.1}s)",
        s.total,
        s.facets.providers.len(),
        s.facets.branches.len(),
        s.facets.models.len(),
        s.facets.tools.len(),
        t.elapsed().as_secs_f64()
    );
    let bash = SessionQuery {
        tool: Some("Bash".into()),
        ..q
    };
    println!(
        "search tool=Bash: {}",
        reader.search_sessions(&bash)?.list.sessions.len()
    );

    let setup = reader.project_setup()?;
    let mut rules: BTreeMap<&str, usize> = BTreeMap::new();
    for p in &setup.projects {
        for f in &p.findings {
            *rules.entry(f.rule).or_default() += 1;
        }
    }
    println!(
        "setup: {} repositories, plans {}, findings {:?}",
        setup.projects.len(),
        setup.plans.len(),
        rules
    );
    Ok(())
}
