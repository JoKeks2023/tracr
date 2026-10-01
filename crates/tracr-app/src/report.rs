//! `tracr day`: the day report as text, in the walkthrough layout, with the
//! quality footer (architecture §13.1 S1.5, §13.4; reporting §4).
//!
//! Human time, agent activity and money are separate blocks. Nothing adds
//! unknown minutes, agent minutes or session spans to human time, and no
//! line adds money of different kinds.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::day::{CostView, DayAgents, SessionDay};

/// Ledger counts the day view does not carry (reporting §4).
#[derive(Debug, Clone, Default)]
pub struct LedgerCounts {
    /// Lines kept with bounded facts only, recorded during the day.
    pub quarantined: i64,
    /// Known record keys whose content changed, recorded during the day.
    pub key_conflicts: i64,
}

fn min(ms: i64) -> i64 {
    (ms + 30_000) / 60_000
}

fn clock(ms: i64, tz: &str) -> String {
    let zone = jiff::tz::TimeZone::get(tz).unwrap_or(jiff::tz::TimeZone::UTC);
    jiff::Timestamp::from_millisecond(ms)
        .map(|t| t.to_zoned(zone).strftime("%H:%M").to_string())
        .unwrap_or_else(|_| "?".into())
}

fn compact(n: i64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1} M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.1} k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

fn provider_name(p: &str) -> &str {
    match p {
        "claude_code" => "Claude Code",
        "codex" => "Codex",
        other => other,
    }
}

fn session_name(s: &SessionDay) -> String {
    let name = s
        .repository_label
        .clone()
        .or_else(|| s.label.clone())
        .unwrap_or_else(|| s.native_session_id.chars().take(8).collect());
    if name.chars().count() > 28 {
        format!("{}…", name.chars().take(27).collect::<String>())
    } else {
        name
    }
}

fn tokens(s: &SessionDay) -> (i64, i64) {
    let mut sum = 0;
    let mut unknown = 0;
    for u in &s.usage {
        for c in [u.input_fresh, u.cache_creation, u.cache_read, u.output] {
            sum += c.sum;
            unknown += c.unknown_records;
        }
    }
    (sum, unknown)
}

/// An estimate as text: unpriced is never zero, a partial estimate is a floor.
pub fn estimate(c: &CostView, currency: &str) -> String {
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

pub fn day_text(d: &DayAgents, counts: &LedgerCounts) -> String {
    let tz = d.timezone.as_str();
    let mut o = String::new();
    let state = match (&d.review, &d.discrepancy) {
        (Some(r), Some(_)) => format!("reviewed r{} · new evidence differs", r.rev),
        (Some(r), None) => format!("reviewed r{}", r.rev),
        (None, _) => "provisional (not reviewed)".into(),
    };
    let _ = writeln!(o, "tracr · {} · {tz} · {state}", d.day);
    if let Some(t) = d.derived_as_of {
        let _ = writeln!(o, "derived as of {}", clock(t, tz));
    }
    let _ = writeln!(o);

    // Human time.
    let h = &d.human;
    let mut by_basis: BTreeMap<&str, i64> = BTreeMap::new();
    let mut non_work = 0;
    for a in &h.atoms {
        match a.status.as_str() {
            "work" => *by_basis.entry(a.duration_basis.as_str()).or_default() += a.end - a.start,
            "non_work" => non_work += a.end - a.start,
            _ => {}
        }
    }
    let _ = writeln!(
        o,
        "{:<44}{:>4} min",
        if d.review.is_some() {
            "Human time, reviewed"
        } else {
            "Human time, inferred (provisional)"
        },
        min(h.counted_ms)
    );
    let mut targets: Vec<(&String, &i64)> = h.counted_by_target.iter().collect();
    targets.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (t, ms) in &targets {
        let _ = writeln!(o, "  {:<40}{:>4}", t, min(**ms));
    }
    let basis: Vec<String> = by_basis
        .iter()
        .map(|(b, ms)| {
            let name = match *b {
                "inferred_continuity" => "inferred",
                "user_asserted" => "user-asserted",
                "timer_measured" => "timer",
                other => other,
            };
            format!("{name} {}", min(*ms))
        })
        .collect();
    if !basis.is_empty() {
        let _ = writeln!(o, "  by basis: {}", basis.join(" · "));
    }
    let _ = writeln!(o, "{:<44}{:>4}", "Unknown, not counted", min(h.unknown_ms));
    for a in h.atoms.iter().filter(|a| a.status == "unknown") {
        let _ = writeln!(
            o,
            "  {}–{}  {:>3}  {}",
            clock(a.start, tz),
            clock(a.end, tz),
            min(a.end - a.start),
            a.unknown_reason
                .as_deref()
                .unwrap_or("unknown")
                .replace('_', " ")
        );
    }
    let _ = writeln!(
        o,
        "{:<44}{:>4}",
        "Non-work (excluded by you)",
        min(non_work)
    );
    if let Some((s, e)) = h.evidence_span {
        let _ = writeln!(
            o,
            "Human evidence span {}–{} = {} min: an upper bound, not a total",
            clock(s, tz),
            clock(e, tz),
            min(e - s)
        );
    }
    let _ = writeln!(o);

    // Agent activity.
    let _ = writeln!(
        o,
        "Agent activity (not human time)            exec min   span          tokens"
    );
    let mut exec_total = 0;
    let mut tok_total = 0;
    let (mut first, mut last) = (i64::MAX, i64::MIN);
    for s in &d.sessions {
        let exec: i64 = s.actors.iter().map(|a| a.execution_ms).sum();
        exec_total += exec;
        let (tok, unknown) = tokens(s);
        tok_total += tok;
        for g in s.actors.iter().flat_map(|a| &a.segments) {
            first = first.min(g.start);
            last = last.max(g.end);
        }
        let span = match (s.first_evidence_at, s.last_evidence_at) {
            (Some(a), Some(b)) => {
                format!(
                    "{}–{}",
                    clock(a.max(d.start), tz),
                    clock(b.min(d.end - 1), tz)
                )
            }
            _ => "-".into(),
        };
        let _ = writeln!(
            o,
            "  {:<12} {:<28} {:>5}   {:<12} {:>8}{}",
            provider_name(&s.provider),
            session_name(s),
            min(exec),
            span,
            compact(tok),
            if unknown > 0 { " +?" } else { "" }
        );
    }
    if !d.sessions.is_empty() && first <= last {
        let _ = writeln!(
            o,
            "  {} execution minutes within {} min of agent wall clock; sessions overlap, never merged",
            min(exec_total),
            min(last - first)
        );
        let _ = writeln!(o, "  {} tokens across sessions", compact(tok_total));
    }
    let _ = writeln!(o);

    // Money: estimate and reported figures, never added.
    let p = &d.cost.pricing;
    let _ = writeln!(
        o,
        "Estimated API cost · {} · catalog {} as of {}",
        p.label,
        p.catalog_version,
        p.oldest_fetched_at.as_deref().unwrap_or("unknown")
    );
    for s in &d.sessions {
        let _ = writeln!(
            o,
            "  {:<12} {:<28} {:>14}",
            provider_name(&s.provider),
            session_name(s),
            estimate(&s.cost, &p.currency)
        );
    }
    let _ = writeln!(
        o,
        "  {:<41} {:>14}",
        "identified usage of the day",
        estimate(&d.cost.identified, &p.currency)
    );
    let unidentified = &d.cost.unidentified;
    if unidentified.status().is_some() {
        let _ = writeln!(
            o,
            "  {:<41} {:>14}",
            "unidentified usage, a separate line",
            estimate(unidentified, &p.currency)
        );
    }
    if d.cost.spanning_records > 0 {
        let _ = writeln!(
            o,
            "  {:<41} {:>14}",
            format!(
                "spanning the day boundary ({} records)",
                d.cost.spanning_records
            ),
            estimate(&d.cost.spanning, &p.currency)
        );
    }
    let reported: Vec<(&SessionDay, &crate::day::ReportedView)> = d
        .sessions
        .iter()
        .flat_map(|s| s.reported.iter().map(move |r| (s, r)))
        .collect();
    if !reported.is_empty() {
        let _ = writeln!(
            o,
            "Provider-reported figures · the provider's own running totals, not spend, never added"
        );
        for (s, r) in reported {
            let _ = writeln!(
                o,
                "  {:<12} {:<28} {} {} ({}, last seen {})",
                provider_name(&s.provider),
                session_name(s),
                r.value_as_stated,
                r.currency,
                r.source_field,
                clock(r.recorded_at, tz)
            );
        }
    }
    let _ = writeln!(o);

    // Quality footer.
    let _ = writeln!(o, "Quality");
    let q = &d.quality;
    let collecting: Vec<String> = d
        .coverage
        .iter()
        .map(|(kind, lanes)| {
            let mut states: BTreeMap<&str, i64> = BTreeMap::new();
            for l in lanes {
                *states.entry(l.state.as_str()).or_default() += l.end - l.start;
            }
            let parts: Vec<String> = states
                .iter()
                .map(|(s, ms)| format!("{s} {}", min(*ms)))
                .collect();
            format!("{kind}: {}", parts.join(", "))
        })
        .collect();
    let _ = writeln!(
        o,
        "  coverage (min): {}",
        if collecting.is_empty() {
            "none recorded".into()
        } else {
            collecting.join(" · ")
        }
    );
    let _ = writeln!(
        o,
        "  ingestion: {}{}",
        if q.ingestion_complete {
            "complete".to_owned()
        } else {
            match q.ingestion_verified_through {
                Some(t) => format!("verified through {}", clock(t, tz)),
                None => "not verified".into(),
            }
        },
        if q.recovered_records > 0 {
            format!(" · {} records recovered late", q.recovered_records)
        } else {
            String::new()
        }
    );
    let gaps = q.stream_extents.iter().filter(|e| e.state == "gap").count();
    if gaps > 0 {
        let _ = writeln!(o, "  {gaps} stream gaps");
    }
    for r in &q.provisional_reasons {
        let _ = writeln!(o, "  provisional: {r}");
    }
    let _ = writeln!(
        o,
        "  corrections {} · manual entries {}",
        d.corrections.len(),
        d.manual_entries.len()
    );
    let _ = writeln!(
        o,
        "  records: {} quarantined · {} key-content conflicts (recorded this day)",
        counts.quarantined, counts.key_conflicts
    );
    let mut diags: BTreeMap<&str, u64> = BTreeMap::new();
    for s in &d.sessions {
        for (k, n) in &s.diagnostics {
            *diags.entry(k.as_str()).or_default() += n;
        }
    }
    for key in [
        "producer_version_not_fixture_validated",
        "unlinked_subagent",
        "prompt_from_non_interactive_source",
        "prompt_from_non_interactive_entrypoint",
        "cumulative_counter_reset",
    ] {
        if let Some(n) = diags.get(key) {
            let _ = writeln!(o, "  {}: {n}", key.replace('_', " "));
        }
    }
    let unknown_actor_records: i64 = d
        .sessions
        .iter()
        .flat_map(|s| {
            s.usage.iter().filter(move |u| {
                s.actors
                    .iter()
                    .any(|a| Some(&a.actor_id) == u.actor_id.as_ref() && a.actor_kind == "unknown")
            })
        })
        .map(|u| u.records)
        .sum();
    let _ = writeln!(
        o,
        "  usage: {} records with actor unknown (main chain not documented, AD-21)",
        unknown_actor_records
    );
    let id = &d.cost.identified;
    let tok = id.priced_tokens + id.unpriced_tokens;
    let mut kinds = vec!["estimated API cost"];
    if d.sessions.iter().any(|s| !s.reported.is_empty()) {
        kinds.push("provider-reported");
    }
    let _ = writeln!(
        o,
        "  cost kinds: {} · unpriced {}% of tokens{}",
        kinds.join(", "),
        if tok > 0 {
            (id.unpriced_tokens * 100 + tok / 2) / tok
        } else {
            0
        },
        if id.assumptions.is_empty() {
            String::new()
        } else {
            format!(" · assumed: {}", id.assumptions.join(", "))
        }
    );
    let _ = writeln!(
        o,
        "  payload never stored: prompt, message and tool text never entered tracr; search, \
         prompt-based suggestions and re-extraction are unavailable"
    );
    o
}
