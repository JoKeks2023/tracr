//! The bundled pricing catalog (architecture §8.3).
//!
//! `pricing/catalog.toml` is embedded at build time. Its revision is the
//! catalog version plus a content-hash prefix; the ledger keeps a copy of
//! every revision that was used, so an estimate's prices stay resolvable.

use std::sync::OnceLock;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::Serialize;
use tracr_core::pricing::Catalog;
use tracr_core::time::UtcMillis;
use tracr_store::ledger;

const BUNDLED: &str = include_str!("../../../pricing/catalog.toml");

pub struct Pricing {
    pub catalog: Catalog,
    pub revision: String,
    pub content_hash: String,
}

/// What a view shows next to every estimate (OQ-A-10 default).
#[derive(Debug, Clone, Serialize)]
pub struct PricingInfo {
    pub revision: String,
    pub catalog_version: String,
    pub currency: String,
    pub source_urls: Vec<String>,
    pub oldest_fetched_at: Option<String>,
    pub label: &'static str,
}

pub const ESTIMATE_LABEL: &str = "API-equivalent estimate, not spend";

pub fn parse(text: &str) -> Result<Pricing> {
    let catalog: Catalog = toml::from_str(text).context("parse pricing catalog")?;
    catalog.validate().context("validate pricing catalog")?;
    let content_hash = blake3::hash(text.as_bytes()).to_hex().to_string();
    Ok(Pricing {
        revision: format!("{}+{}", catalog.catalog_version, &content_hash[..12]),
        catalog,
        content_hash,
    })
}

/// The catalog this build carries. A broken bundled catalog is a build bug.
pub fn bundled() -> &'static Pricing {
    static P: OnceLock<Pricing> = OnceLock::new();
    P.get_or_init(|| parse(BUNDLED).expect("bundled pricing catalog is valid"))
}

/// The bundled catalog text.
pub fn bundled_text() -> &'static str {
    BUNDLED
}

impl Pricing {
    pub fn info(&self) -> PricingInfo {
        PricingInfo {
            revision: self.revision.clone(),
            catalog_version: self.catalog.catalog_version.clone(),
            currency: self.catalog.currency.clone(),
            source_urls: self.catalog.sources.iter().map(|s| s.url.clone()).collect(),
            oldest_fetched_at: self.catalog.oldest_fetched_at().map(str::to_owned),
            label: ESTIMATE_LABEL,
        }
    }

    /// Copies this catalog into the ledger on first use.
    pub fn record(&self, conn: &Connection, at: UtcMillis) -> Result<()> {
        ledger::ensure_pricing_catalog(
            conn,
            &self.revision,
            &self.catalog.catalog_version,
            &self.content_hash,
            BUNDLED,
            at,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_is_valid_and_exact() {
        let p = bundled();
        assert!(p.revision.starts_with(&p.catalog.catalog_version));
        assert_eq!(p.catalog.currency, "USD");
        // Every rule prices input, cache reads and output. Claude rules also
        // price both cache-write tiers; Codex records no cache writes as a
        // category (cache writes are a marker there), so its rules have none.
        for r in &p.catalog.rules {
            let mut cats = vec!["input", "cache_read", "output"];
            if r.provider == "claude_code" {
                cats.extend(["cache_write_5m", "cache_write_1h"]);
            }
            for c in cats {
                assert!(r.per_mtok.contains_key(c), "{} lacks {c}", r.id);
            }
        }
    }
}
