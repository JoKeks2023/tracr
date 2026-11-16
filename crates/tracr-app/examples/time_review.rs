//! Times review requests through a running collector (GH #32).
//!
//! Usage: `cargo run -p tracr-app --example time_review -- <data-dir> <day>`
//! Use a COPY of a data directory, never the live one: this writes reviews.
//! Set `TRACR_JOB_TRACE=1` to see every collector job's run time.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tracr_app::runtime::Runtime;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("data dir"));
    let day = args.next().expect("day, e.g. 2026-11-11");
    let rounds: usize = args.next().map_or(2, |n| n.parse().expect("rounds"));

    let t = Instant::now();
    let rt = Runtime::start(&dir, "time_review")?;
    let h = rt.handle();
    let r = h.refresh()?;
    eprintln!(
        "startup+refresh {:.2}s ({} new)",
        t.elapsed().as_secs_f64(),
        r.inserted
    );

    for i in 1..=rounds {
        // Let the timer queue its usual background jobs in between, as in the app.
        std::thread::sleep(Duration::from_secs(20));
        let t = Instant::now();
        // A reason is always given: the day may already be reviewed in the copy.
        let id = h.review(day.clone(), Some(format!("timing round {i}")))?;
        eprintln!("review r{i} {:.2}s ({id})", t.elapsed().as_secs_f64());
    }
    rt.stop("quit");
    Ok(())
}
