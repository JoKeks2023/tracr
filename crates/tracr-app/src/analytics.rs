//! Usage analytics over a range of days (T-0015 B1, after Claudoscope's
//! Analytics): cost per day and model, how well the prompt cache worked, and
//! a what-if on a cheaper model. Everything is an API-equivalent estimate from
//! the bundled price catalog, never spend; unpriced stays unpriced.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use rusqlite::Connection;
use serde::Serialize;
use tracr_core::pricing::{PricedUsage, estimate};

use crate::day::{AUTOMATED_MODELS, ReportingZone};
use crate::pricing::{PricingInfo, bundled};

/// A prompt cache entry lives five minutes (one hour for the one-hour tier); a gap longer than this makes the next request re-prime it.
const CACHE_TTL_MS: i64 = 5 * 60_000;
const CACHE_TTL_1H_MS: i64 = 60 * 60_000;
/// A request that wrote at least this share of its context to the cache after a gap re-primed it ("cold").
const COLD_SHARE: f64 = 0.5;
/// The model the what-if reprices Opus usage on.
const WHATIF_MODEL: &str = "claude-sonnet-5-5";

#[derive(Debug, Clone, Default, Serialize)]
pub struct ModelCost {
    pub model: String,
    pub records: i64,
    pub tokens: i64,
    /// Sum of the priced parts; `None` when nothing was priced (unpriced, not zero).
    pub amount_nano: Option<i64>,
    /// Some records of this model were unpriced or only partly priced: the amount is a floor.
    pub partial: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CostDay {
    pub day: String,
    pub models: Vec<ModelCost>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CacheStats {
    pub fresh_tokens: i64,
    pub read_tokens: i64,
    pub write_5m_tokens: i64,
    pub write_1h_tokens: i64,
    pub write_untiered_tokens: i64,
    /// Share of the input that was read from the cache (0..1); `None` without input.
    pub hit_ratio: Option<f64>,
    /// What the cached reads saved against reading the same tokens fresh, at catalog prices.
    pub saved_nano: i64,
    /// Requests that re-primed the cache after it had expired.
    pub cold_requests: i64,
    pub cold_tokens: i64,
    /// What re-priming cost beyond reading the same tokens from a warm cache.
    pub cold_extra_nano: i64,
    /// Sessions with the lowest cache share first (at least a small amount of input).
    pub worst_sessions: Vec<SessionCache>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionCache {
    pub session_id: String,
    pub label: Option<String>,
    pub tokens: i64,
    pub hit_ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WhatIf {
    pub from_family: String,
    pub to_model: String,
    pub tokens: i64,
    pub current_nano: i64,
    pub repriced_nano: i64,
    /// `current - repriced`; negative when the other model would cost more.
    pub saving_nano: i64,
    /// Some of the usage was only partly priced: both sides are floors.
    pub partial: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Analytics {
    pub timezone: String,
    pub from: String,
    pub to: String,
    pub pricing: PricingInfo,
    pub days: Vec<CostDay>,
    pub models: Vec<ModelCost>,
    pub cache: CacheStats,
    pub what_if: Option<WhatIf>,
}

struct Row {
    session: String,
    actor: Option<String>,
    provider: String,
    model: Option<String>,
    tier: Option<String>,
    at: i64,
    fresh: Option<i64>,
    total: Option<i64>,
    read: Option<i64>,
    creation: Option<i64>,
    tiers: BTreeMap<String, u64>,
    output: Option<i64>,
    nano: Option<i64>,
    status: Option<String>,
}

fn rows(conn: &Connection, start: i64, end: i64, revision: &str) -> Result<Vec<Row>> {
    let mut stmt = conn.prepare(
        "SELECT u.session_id, u.actor_id, u.provider, u.model, u.service_tier, u.extent_start,
                u.input_fresh, u.input_total, u.cache_read, u.cache_creation, u.cache_creation_tiers, u.output,
                c.amount_nano, c.status
         FROM usage_record u
         LEFT JOIN cost_estimate c ON c.usage_id = u.id AND c.pricing_revision = ?3
         WHERE u.canonical = 1 AND u.identified = 1 AND u.extent_start IS NOT NULL
           AND u.extent_start >= ?1 AND u.extent_start < ?2
         ORDER BY u.session_id, u.actor_id, u.extent_start",
    )?;
    let it = stmt.query_map(rusqlite::params![start, end, revision], |r| {
        let tiers: Option<String> = r.get(10)?;
        Ok(Row {
            session: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
            actor: r.get(1)?,
            provider: r.get(2)?,
            model: r.get(3)?,
            tier: r.get(4)?,
            at: r.get(5)?,
            fresh: r.get(6)?,
            total: r.get(7)?,
            read: r.get(8)?,
            creation: r.get(9)?,
            tiers: tiers
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default(),
            output: r.get(11)?,
            nano: r.get(12)?,
            status: r.get(13)?,
        })
    })?;
    Ok(it.collect::<rusqlite::Result<_>>()?)
}

fn nano_of(e: &tracr_core::pricing::Estimate) -> Option<i64> {
    e.amount.as_ref().and_then(|a| i64::try_from(a.0.nano).ok())
}

fn family(model: &str) -> &'static str {
    if model.contains("opus") {
        "opus"
    } else if model.contains("sonnet") {
        "sonnet"
    } else if model.contains("haiku") {
        "haiku"
    } else {
        "other"
    }
}

/// The analytics of `[from, to]` (reporting days, inclusive). Automated models are left out unless `show_automated`.
pub fn analytics(
    conn: &Connection,
    zone: &ReportingZone,
    from: &str,
    to: &str,
    show_automated: bool,
) -> Result<Analytics> {
    let parse = |d: &str| -> Result<jiff::civil::Date> {
        d.parse()
            .map_err(|e| anyhow!("day must be YYYY-MM-DD: {e}"))
    };
    let (from_d, to_d) = (parse(from)?, parse(to)?);
    if to_d < from_d {
        return Err(anyhow!("range ends before it starts"));
    }
    let (start, _) = zone.bounds(from_d)?;
    let (_, end) = zone.bounds(to_d)?;
    let pricing = bundled();
    let rows = rows(conn, start, end, &pricing.revision)?;
    let rows: Vec<Row> = rows
        .into_iter()
        .filter(|r| {
            show_automated
                || !r
                    .model
                    .as_deref()
                    .is_some_and(|m| AUTOMATED_MODELS.contains(&m))
        })
        .collect();

    // ---- cost per day and model ----
    let mut per_day: BTreeMap<String, BTreeMap<String, ModelCost>> = BTreeMap::new();
    let mut totals: BTreeMap<String, ModelCost> = BTreeMap::new();
    let add = |m: &mut ModelCost, r: &Row| {
        m.records += 1;
        m.tokens += r.fresh.unwrap_or(0)
            + r.read.unwrap_or(0)
            + r.creation.unwrap_or(0)
            + r.output.unwrap_or(0);
        if let Some(n) = r.nano {
            m.amount_nano = Some(m.amount_nano.unwrap_or(0) + n);
        }
        if r.status.as_deref() != Some("priced") {
            m.partial = true;
        }
    };
    for r in &rows {
        let model = r.model.clone().unwrap_or_else(|| "unknown".to_owned());
        let Some(day) = zone.day_of(r.at) else {
            continue;
        };
        let slot = |map: &mut BTreeMap<String, ModelCost>| {
            let m = map.entry(model.clone()).or_insert_with(|| ModelCost {
                model: model.clone(),
                ..ModelCost::default()
            });
            add(m, r);
        };
        slot(per_day.entry(day.to_string()).or_default());
        slot(&mut totals);
    }
    let mut days = Vec::new();
    let mut d = from_d;
    while d <= to_d {
        let key = d.to_string();
        let mut models: Vec<ModelCost> = per_day
            .remove(&key)
            .map(|m| m.into_values().collect())
            .unwrap_or_default();
        models.sort_by_key(|m| std::cmp::Reverse(m.amount_nano.unwrap_or(0)));
        days.push(CostDay { day: key, models });
        d = d.tomorrow()?;
    }
    let mut models: Vec<ModelCost> = totals.into_values().collect();
    models.sort_by(|a, b| {
        b.amount_nano
            .unwrap_or(0)
            .cmp(&a.amount_nano.unwrap_or(0))
            .then_with(|| b.tokens.cmp(&a.tokens))
    });

    // ---- cache health ----
    let mut cache = CacheStats::default();
    let mut by_session: BTreeMap<String, (i64, i64)> = BTreeMap::new(); // (read, all input)
    let mut read_by_model: BTreeMap<(String, String, String), u64> = BTreeMap::new();
    for r in &rows {
        let fresh = r
            .fresh
            .unwrap_or_else(|| (r.total.unwrap_or(0) - r.read.unwrap_or(0)).max(0));
        let read = r.read.unwrap_or(0);
        let creation = r.creation.unwrap_or(0);
        cache.fresh_tokens += fresh;
        cache.read_tokens += read;
        let t5: u64 = r
            .tiers
            .get("ephemeral_5m_input_tokens")
            .copied()
            .unwrap_or(0);
        let t1: u64 = r
            .tiers
            .get("ephemeral_1h_input_tokens")
            .copied()
            .unwrap_or(0);
        cache.write_5m_tokens += t5 as i64;
        cache.write_1h_tokens += t1 as i64;
        cache.write_untiered_tokens += (creation - t5 as i64 - t1 as i64).max(0);
        let all = fresh + read + creation;
        let s = by_session.entry(r.session.clone()).or_default();
        s.0 += read;
        s.1 += all;
        if read > 0
            && let Some(m) = &r.model
        {
            *read_by_model
                .entry((
                    r.provider.clone(),
                    m.clone(),
                    r.tier.clone().unwrap_or_default(),
                ))
                .or_default() += read as u64;
        }
    }
    let input_all = cache.fresh_tokens
        + cache.read_tokens
        + cache.write_5m_tokens
        + cache.write_1h_tokens
        + cache.write_untiered_tokens;
    cache.hit_ratio = (input_all > 0).then(|| cache.read_tokens as f64 / input_all as f64);

    // Saved: the same tokens read from the cache instead of fresh, per model.
    for ((provider, model, tier), n) in &read_by_model {
        let base = PricedUsage {
            provider,
            model: Some(model),
            service_tier: Some(if tier.is_empty() { "standard" } else { tier }),
            ..PricedUsage::default()
        };
        let fresh = estimate(
            &pricing.catalog,
            &PricedUsage {
                input_fresh: Some(*n),
                ..base.clone()
            },
        );
        let read = estimate(
            &pricing.catalog,
            &PricedUsage {
                cache_read: Some(*n),
                ..base
            },
        );
        if let (Some(f), Some(r)) = (nano_of(&fresh), nano_of(&read)) {
            cache.saved_nano += f - r;
        }
    }

    // Cold requests: after a gap longer than the cache lives, most of the context was written again.
    let mut prev: Option<(&str, &Option<String>, i64, bool)> = None; // session, actor, at, wrote 1h tier
    for r in &rows {
        let creation = r.creation.unwrap_or(0);
        let all = r.fresh.unwrap_or(0) + r.read.unwrap_or(0) + creation;
        if let Some((s, a, at, one_hour)) = prev
            && s == r.session
            && a == &r.actor
        {
            let ttl = if one_hour {
                CACHE_TTL_1H_MS
            } else {
                CACHE_TTL_MS
            };
            if r.at - at > ttl && all > 0 && creation as f64 / all as f64 >= COLD_SHARE {
                cache.cold_requests += 1;
                cache.cold_tokens += creation;
                if let Some(m) = &r.model {
                    let tiers: BTreeMap<String, u64> =
                        [("ephemeral_5m_input_tokens".to_owned(), creation as u64)].into();
                    let base = PricedUsage {
                        provider: &r.provider,
                        model: Some(m),
                        service_tier: Some(r.tier.as_deref().unwrap_or("standard")),
                        ..PricedUsage::default()
                    };
                    let write = estimate(
                        &pricing.catalog,
                        &PricedUsage {
                            cache_creation: Some(creation as u64),
                            cache_creation_tiers: Some(&tiers),
                            ..base.clone()
                        },
                    );
                    let read = estimate(
                        &pricing.catalog,
                        &PricedUsage {
                            cache_read: Some(creation as u64),
                            ..base
                        },
                    );
                    if let (Some(w), Some(rd)) = (nano_of(&write), nano_of(&read)) {
                        cache.cold_extra_nano += (w - rd).max(0);
                    }
                }
            }
        }
        prev = Some((
            &r.session,
            &r.actor,
            r.at,
            r.tiers
                .get("ephemeral_1h_input_tokens")
                .copied()
                .unwrap_or(0)
                > 0,
        ));
    }

    let mut ranked: Vec<(String, i64, f64)> = by_session
        .into_iter()
        .filter(|(id, (_, all))| !id.is_empty() && *all >= 200_000)
        .map(|(id, (read, all))| (id, all, read as f64 / all as f64))
        .collect();
    ranked.sort_by(|a, b| a.2.total_cmp(&b.2));
    for (id, tokens, hit_ratio) in ranked.into_iter().take(5) {
        let label = conn
            .query_row(
                "SELECT label FROM agent_session WHERE id = ?1",
                [&id],
                |r| r.get(0),
            )
            .ok();
        cache.worst_sessions.push(SessionCache {
            session_id: id,
            label,
            tokens,
            hit_ratio,
        });
    }

    // ---- what if the Opus usage ran on a cheaper model ----
    let mut opus_current = 0i64;
    let mut opus_repriced = 0i64;
    let mut opus_tokens = 0i64;
    let mut complete = true;
    let mut opus_partial = false;
    for r in rows
        .iter()
        .filter(|r| r.model.as_deref().is_some_and(|m| family(m) == "opus"))
    {
        let (Some(now), Some(m)) = (r.nano, r.model.as_deref()) else {
            complete = false;
            continue;
        };
        let _ = m;
        opus_partial |= r.status.as_deref() != Some("priced");
        let to = estimate(
            &pricing.catalog,
            &PricedUsage {
                provider: &r.provider,
                model: Some(WHATIF_MODEL),
                service_tier: Some(r.tier.as_deref().unwrap_or("standard")),
                input_fresh: r.fresh.map(|v| v as u64),
                input_total: r.total.map(|v| v as u64),
                cache_read: r.read.map(|v| v as u64),
                cache_creation: r.creation.map(|v| v as u64),
                cache_creation_tiers: Some(&r.tiers),
                output: r.output.map(|v| v as u64),
                ..PricedUsage::default()
            },
        );
        match nano_of(&to) {
            Some(n) => {
                opus_current += now;
                opus_repriced += n;
                opus_tokens += r.fresh.unwrap_or(0)
                    + r.read.unwrap_or(0)
                    + r.creation.unwrap_or(0)
                    + r.output.unwrap_or(0);
            }
            None => complete = false,
        }
    }
    let what_if = (opus_tokens > 0 && complete).then(|| WhatIf {
        from_family: "opus".to_owned(),
        to_model: WHATIF_MODEL.to_owned(),
        tokens: opus_tokens,
        current_nano: opus_current,
        repriced_nano: opus_repriced,
        saving_nano: opus_current - opus_repriced,
        partial: opus_partial,
    });

    Ok(Analytics {
        timezone: zone.name.clone(),
        from: from_d.to_string(),
        to: to_d.to_string(),
        pricing: pricing.info(),
        days,
        models,
        cache,
        what_if,
    })
}
