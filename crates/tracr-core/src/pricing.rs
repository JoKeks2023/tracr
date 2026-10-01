//! Price rules and per-record estimation (cost model §3–§4, architecture §8.3).
//!
//! A rule matches the exact `(provider, model, service tier, speed,
//! inference geography)`; there is no substring or family matching (CO-04).
//! Estimation runs per usage record, before aggregation (CO-03). A category
//! without a price leaves its tokens unpriced; unpriced is never zero (CO-02).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::money::{EstimatedApiCost, Money, parse_scaled};

/// Token prices are entered per million tokens with at most three decimals,
/// which is exactly nano-units per token.
const TOKEN_PRICE_SCALE: u32 = 3;
/// Per-unit prices (tool charges) are entered in currency units.
const UNIT_PRICE_SCALE: u32 = 9;

/// The pricing catalog as written (`pricing/catalog.toml`).
#[derive(Debug, Clone, Deserialize)]
pub struct Catalog {
    pub catalog_version: String,
    pub currency: String,
    pub sources: Vec<PricingSource>,
    /// Maps a provider's cache-write tier key to a price category.
    #[serde(default)]
    pub tier_categories: BTreeMap<String, String>,
    pub rules: Vec<PriceRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PricingSource {
    pub id: String,
    pub url: String,
    /// `YYYY-MM-DD`: the day the values were read from the source.
    pub fetched_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PriceRule {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub service_tier: String,
    pub speed: String,
    pub inference_geo: String,
    pub source: String,
    /// `YYYY-MM-DD` (UTC), when known.
    #[serde(default)]
    pub effective_from: Option<String>,
    #[serde(default)]
    pub effective_to: Option<String>,
    /// Category → price per million tokens, as text.
    pub per_mtok: BTreeMap<String, String>,
    /// Tool charge key → price per unit, as text.
    #[serde(default)]
    pub per_unit: BTreeMap<String, String>,
}

/// Token categories a rule can price.
pub const INPUT: &str = "input";
pub const CACHE_READ: &str = "cache_read";
pub const OUTPUT: &str = "output";

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("rule {rule}: {what}")]
    Rule { rule: String, what: String },
    #[error("duplicate rule key {0}")]
    Duplicate(String),
}

impl Catalog {
    /// Rejects prices that are not exact decimals, unknown sources, and two
    /// rules for one key.
    pub fn validate(&self) -> Result<(), CatalogError> {
        let mut keys = std::collections::BTreeSet::new();
        for r in &self.rules {
            let err = |what: String| CatalogError::Rule {
                rule: r.id.clone(),
                what,
            };
            if !self.sources.iter().any(|s| s.id == r.source) {
                return Err(err(format!("unknown source {}", r.source)));
            }
            for (cat, p) in &r.per_mtok {
                parse_scaled(p, TOKEN_PRICE_SCALE)
                    .ok_or_else(|| err(format!("{cat}: {p} is not an exact price")))?;
            }
            for (key, p) in &r.per_unit {
                parse_scaled(p, UNIT_PRICE_SCALE)
                    .ok_or_else(|| err(format!("{key}: {p} is not an exact price")))?;
            }
            for d in [&r.effective_from, &r.effective_to].into_iter().flatten() {
                day_ms(d).ok_or_else(|| err(format!("{d} is not a date")))?;
            }
            let key = (
                &r.provider,
                &r.model,
                &r.service_tier,
                &r.speed,
                &r.inference_geo,
            );
            if !keys.insert(key) {
                return Err(CatalogError::Duplicate(r.id.clone()));
            }
        }
        Ok(())
    }

    fn source(&self, id: &str) -> Option<&PricingSource> {
        self.sources.iter().find(|s| s.id == id)
    }

    /// The oldest `fetched_at` among the sources (shown with every estimate).
    pub fn oldest_fetched_at(&self) -> Option<&str> {
        self.sources.iter().map(|s| s.fetched_at.as_str()).min()
    }
}

fn day_ms(d: &str) -> Option<i64> {
    let date: jiff::civil::Date = d.parse().ok()?;
    let ts = date.to_zoned(jiff::tz::TimeZone::UTC).ok()?.timestamp();
    Some(ts.as_millisecond())
}

/// One usage record as the estimator sees it.
#[derive(Debug, Clone, Default)]
pub struct PricedUsage<'a> {
    pub provider: &'a str,
    pub model: Option<&'a str>,
    pub service_tier: Option<&'a str>,
    /// `None` when the source does not retain it.
    pub speed: Option<&'a str>,
    pub inference_geo: Option<&'a str>,
    pub at: Option<i64>,
    pub input_fresh: Option<u64>,
    /// Input including cached input, for providers that report it that way.
    pub input_total: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_creation: Option<u64>,
    pub cache_creation_tiers: Option<&'a BTreeMap<String, u64>>,
    pub output: Option<u64>,
    pub tool_charges: Option<&'a BTreeMap<String, u64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceStatus {
    Priced,
    PartiallyPriced,
    Unpriced,
}

impl PriceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Priced => "priced",
            Self::PartiallyPriced => "partially_priced",
            Self::Unpriced => "unpriced",
        }
    }
}

/// The estimate of one usage record under one pricing revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Estimate {
    pub status: PriceStatus,
    /// `None` when nothing could be priced: unpriced, not zero.
    pub amount: Option<EstimatedApiCost>,
    pub priced_tokens: u64,
    pub unpriced_tokens: u64,
    /// Why some or all of the record is unpriced.
    pub reasons: Vec<String>,
    /// Conditions the rule requires that the record does not state.
    pub assumptions: Vec<String>,
    /// `at_usage_time`, or `catalog_as_of:<fetched_at>`.
    pub price_basis: Option<String>,
    pub rule_id: Option<String>,
}

fn tokens(u: &PricedUsage<'_>) -> u64 {
    let fresh = fresh_input(u).unwrap_or(0);
    fresh + u.cache_read.unwrap_or(0) + u.cache_creation.unwrap_or(0) + u.output.unwrap_or(0)
}

fn fresh_input(u: &PricedUsage<'_>) -> Option<u64> {
    u.input_fresh.or_else(|| {
        let total = u.input_total?;
        Some(total.saturating_sub(u.cache_read.unwrap_or(0)))
    })
}

fn unpriced(u: &PricedUsage<'_>, reason: String) -> Estimate {
    Estimate {
        status: PriceStatus::Unpriced,
        amount: None,
        priced_tokens: 0,
        unpriced_tokens: tokens(u),
        reasons: vec![reason],
        assumptions: Vec::new(),
        price_basis: None,
        rule_id: None,
    }
}

#[derive(Default)]
struct Acc {
    amount: i128,
    any_priced: bool,
    priced_tokens: u64,
    unpriced_tokens: u64,
    reasons: Vec<String>,
}

impl Acc {
    fn tokens(&mut self, rule: &PriceRule, category: &str, count: Option<u64>) {
        match count {
            None => self
                .reasons
                .push(format!("category_not_reported:{category}")),
            Some(0) => {}
            Some(n) => match rule
                .per_mtok
                .get(category)
                .and_then(|p| parse_scaled(p, TOKEN_PRICE_SCALE))
            {
                Some(nano_per_token) => {
                    self.amount += i128::from(n) * nano_per_token;
                    self.priced_tokens += n;
                    self.any_priced = true;
                }
                None => {
                    self.unpriced_tokens += n;
                    self.reasons.push(format!("no_price_for:{category}"));
                }
            },
        }
    }
}

/// Estimates one record. Never returns a zero for something it cannot price.
pub fn estimate(catalog: &Catalog, u: &PricedUsage<'_>) -> Estimate {
    let Some(model) = u.model else {
        return unpriced(u, "model_unknown".into());
    };
    if !catalog
        .rules
        .iter()
        .any(|r| r.provider == u.provider && r.model == model)
    {
        return unpriced(u, format!("no_price_rule:{model}"));
    }
    let Some(tier) = u.service_tier else {
        return unpriced(u, "service_tier_unknown".into());
    };
    let mut assumptions = Vec::new();
    let speed = u.speed.unwrap_or_else(|| {
        assumptions.push("speed_not_recorded:standard".to_owned());
        "standard"
    });
    let geo = u.inference_geo.unwrap_or_else(|| {
        assumptions.push("inference_geo_not_recorded:global".to_owned());
        "global"
    });
    let Some(rule) = catalog.rules.iter().find(|r| {
        r.provider == u.provider
            && r.model == model
            && r.service_tier == tier
            && r.speed == speed
            && r.inference_geo == geo
    }) else {
        return unpriced(u, format!("no_price_rule:{model}/{tier}/{speed}/{geo}"));
    };

    let mut acc = Acc::default();
    acc.tokens(rule, INPUT, fresh_input(u));
    acc.tokens(rule, CACHE_READ, u.cache_read);
    acc.tokens(rule, OUTPUT, u.output);
    // Cache writes are priced by TTL tier. Tokens of an unreported tier stay unpriced.
    match u.cache_creation {
        None => acc.tokens(rule, "cache_write", None),
        Some(total) => {
            let mut tiered = 0u64;
            for (key, &n) in u.cache_creation_tiers.into_iter().flatten() {
                tiered += n;
                match catalog.tier_categories.get(key) {
                    Some(cat) => acc.tokens(rule, cat, Some(n)),
                    None if n > 0 => {
                        acc.unpriced_tokens += n;
                        acc.reasons.push(format!("cache_tier_unknown:{key}"));
                    }
                    None => {}
                }
            }
            if total > tiered {
                acc.unpriced_tokens += total - tiered;
                acc.reasons.push("cache_write_tier_not_reported".into());
            }
        }
    }
    for (key, &n) in u.tool_charges.into_iter().flatten() {
        if n == 0 {
            continue;
        }
        match rule
            .per_unit
            .get(key)
            .and_then(|p| parse_scaled(p, UNIT_PRICE_SCALE))
        {
            Some(nano_per_unit) => {
                acc.amount += i128::from(n) * nano_per_unit;
                acc.any_priced = true;
            }
            None => acc.reasons.push(format!("no_price_for_tool:{key}")),
        }
    }
    let Acc {
        amount,
        any_priced,
        priced_tokens,
        unpriced_tokens,
        reasons,
    } = acc;

    let status = if reasons.is_empty() {
        PriceStatus::Priced
    } else if any_priced {
        PriceStatus::PartiallyPriced
    } else {
        PriceStatus::Unpriced
    };
    let fetched_at = catalog
        .source(&rule.source)
        .map(|s| s.fetched_at.as_str())
        .unwrap_or("unknown");
    let in_effect = u.at.is_some_and(|at| {
        let from = rule.effective_from.as_deref().and_then(day_ms);
        let to = rule.effective_to.as_deref().and_then(day_ms);
        from.is_some_and(|f| at >= f) && to.is_none_or(|t| at < t)
    });
    Estimate {
        status,
        amount: (status != PriceStatus::Unpriced)
            .then(|| EstimatedApiCost(Money::new(amount, &catalog.currency))),
        priced_tokens,
        unpriced_tokens,
        reasons,
        assumptions,
        price_basis: Some(if in_effect {
            "at_usage_time".to_owned()
        } else {
            format!("catalog_as_of:{fetched_at}")
        }),
        rule_id: Some(rule.id.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        let rule = |id: &str, model: &str, speed: &str, from: Option<&str>| PriceRule {
            id: id.into(),
            provider: "p".into(),
            model: model.into(),
            service_tier: "standard".into(),
            speed: speed.into(),
            inference_geo: "global".into(),
            source: "s".into(),
            effective_from: from.map(str::to_owned),
            effective_to: None,
            per_mtok: [
                ("input", "4"),
                ("cache_write_5m", "5"),
                ("cache_write_1h", "8"),
                ("cache_read", "0.20"),
                ("output", "20"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
            per_unit: [("web_search_requests".to_owned(), "0.01".to_owned())].into(),
        };
        Catalog {
            catalog_version: "t".into(),
            currency: "USD".into(),
            sources: vec![PricingSource {
                id: "s".into(),
                url: "u".into(),
                fetched_at: "2026-10-01".into(),
            }],
            tier_categories: [
                ("ephemeral_5m_input_tokens", "cache_write_5m"),
                ("ephemeral_1h_input_tokens", "cache_write_1h"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
            rules: vec![
                rule("m", "m", "standard", None),
                rule("dated", "dated", "standard", Some("2026-01-01")),
            ],
        }
    }

    fn tiers(t5: u64, t1: u64) -> BTreeMap<String, u64> {
        [
            ("ephemeral_5m_input_tokens".to_owned(), t5),
            ("ephemeral_1h_input_tokens".to_owned(), t1),
        ]
        .into()
    }

    fn base<'a>(t: &'a BTreeMap<String, u64>) -> PricedUsage<'a> {
        PricedUsage {
            provider: "p",
            model: Some("m"),
            service_tier: Some("standard"),
            speed: Some("standard"),
            inference_geo: Some("global"),
            at: Some(1_800_000_000_000),
            input_fresh: Some(1_000_000),
            cache_read: Some(1_000_000),
            cache_creation: Some(2_000_000),
            cache_creation_tiers: Some(t),
            output: Some(1_000_000),
            ..Default::default()
        }
    }

    #[test]
    fn prices_every_category_exactly() {
        let t = tiers(1_000_000, 1_000_000);
        let e = estimate(&catalog(), &base(&t));
        assert_eq!(e.status, PriceStatus::Priced);
        // 4 + 0.20 + 20 + 5 + 8 = 37.20 USD
        assert_eq!(e.amount.unwrap().0.nano, 37_200_000_000);
        assert_eq!(e.priced_tokens, 5_000_000);
        assert_eq!(e.unpriced_tokens, 0);
        assert_eq!(e.price_basis.as_deref(), Some("catalog_as_of:2026-10-01"));
    }

    #[test]
    fn unknown_model_is_unpriced_not_zero() {
        let t = tiers(2_000_000, 0);
        let u = PricedUsage {
            model: Some("m-2"),
            ..base(&t)
        };
        let e = estimate(&catalog(), &u);
        assert_eq!(e.status, PriceStatus::Unpriced);
        assert_eq!(e.amount, None);
        assert_eq!(e.unpriced_tokens, 5_000_000);
        // No family or substring match.
        assert_eq!(e.reasons, ["no_price_rule:m-2"]);
    }

    #[test]
    fn untiered_cache_writes_stay_unpriced() {
        let t = tiers(500_000, 0);
        let e = estimate(&catalog(), &base(&t));
        assert_eq!(e.status, PriceStatus::PartiallyPriced);
        assert_eq!(e.unpriced_tokens, 1_500_000);
        assert!(
            e.reasons
                .contains(&"cache_write_tier_not_reported".to_owned())
        );
    }

    #[test]
    fn missing_category_is_partial_and_named() {
        let t = tiers(2_000_000, 0);
        let u = PricedUsage {
            output: None,
            ..base(&t)
        };
        let e = estimate(&catalog(), &u);
        assert_eq!(e.status, PriceStatus::PartiallyPriced);
        assert!(
            e.reasons
                .contains(&"category_not_reported:output".to_owned())
        );
    }

    #[test]
    fn unrecorded_conditions_are_stated_assumptions() {
        let t = tiers(2_000_000, 0);
        let u = PricedUsage {
            speed: None,
            inference_geo: None,
            ..base(&t)
        };
        let e = estimate(&catalog(), &u);
        assert_eq!(e.status, PriceStatus::Priced);
        assert_eq!(
            e.assumptions,
            [
                "speed_not_recorded:standard",
                "inference_geo_not_recorded:global"
            ]
        );
        // A stated condition without a rule is not assumed away.
        let fast = PricedUsage {
            speed: Some("fast"),
            ..base(&t)
        };
        assert_eq!(estimate(&catalog(), &fast).status, PriceStatus::Unpriced);
    }

    #[test]
    fn tool_charges_priced_or_named() {
        let t = tiers(2_000_000, 0);
        let charges: BTreeMap<String, u64> = [
            ("web_search_requests".to_owned(), 3),
            ("code_execution_requests".to_owned(), 1),
            ("web_fetch_requests".to_owned(), 0),
        ]
        .into();
        let u = PricedUsage {
            tool_charges: Some(&charges),
            ..base(&t)
        };
        let e = estimate(&catalog(), &u);
        assert_eq!(e.status, PriceStatus::PartiallyPriced);
        assert_eq!(e.reasons, ["no_price_for_tool:code_execution_requests"]);
        let tokens_only = estimate(&catalog(), &base(&t)).amount.unwrap().0.nano;
        assert_eq!(e.amount.unwrap().0.nano - tokens_only, 30_000_000);
    }

    #[test]
    fn input_total_includes_cached_input() {
        let none = BTreeMap::new();
        let u = PricedUsage {
            input_fresh: None,
            input_total: Some(1_000_000),
            cache_read: Some(400_000),
            cache_creation: Some(0),
            cache_creation_tiers: None,
            output: Some(0),
            ..base(&none)
        };
        let e = estimate(&catalog(), &u);
        // 0.6 M × 4 + 0.4 M × 0.20 = 2.48 USD
        assert_eq!(e.amount.unwrap().0.nano, 2_480_000_000);
    }

    #[test]
    fn rule_in_effect_prices_at_usage_time() {
        let t = tiers(2_000_000, 0);
        let u = PricedUsage {
            model: Some("dated"),
            ..base(&t)
        };
        let e = estimate(&catalog(), &u);
        assert_eq!(e.price_basis.as_deref(), Some("at_usage_time"));
        let before = PricedUsage {
            at: Some(1_700_000_000_000),
            ..u
        };
        assert_eq!(
            estimate(&catalog(), &before).price_basis.as_deref(),
            Some("catalog_as_of:2026-10-01")
        );
    }

    #[test]
    fn validation_rejects_inexact_prices_and_duplicates() {
        let mut c = catalog();
        c.validate().unwrap();
        c.rules[0].per_mtok.insert("input".into(), "0.0001".into());
        assert!(c.validate().is_err());
        let mut c = catalog();
        c.rules.push(c.rules[0].clone());
        assert!(matches!(c.validate(), Err(CatalogError::Duplicate(_))));
    }
}
