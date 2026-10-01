//! Monetary kinds (cost model §1, architecture §8.2).
//!
//! Every kind is its own type. No kind implements `Default`, a zero or
//! `Sum`. `Add` exists only within one kind and one currency, and not at all
//! for [`ProviderReportedCost`]: a provider's cumulative figure is never
//! summed. There is deliberately no total across kinds.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Nano-units per currency unit.
pub const NANO: i128 = 1_000_000_000;

/// An amount in nano-units of one currency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub nano: i128,
    pub currency: String,
}

impl Money {
    pub fn new(nano: i128, currency: &str) -> Self {
        Self {
            nano,
            currency: currency.to_owned(),
        }
    }

    /// The same amount rounded half away from zero to `decimals` places, as text.
    pub fn display(&self, decimals: u32) -> String {
        let step = NANO / 10i128.pow(decimals.min(9));
        let rounded = (self.nano.abs() + step / 2) / step * step;
        let sign = if self.nano < 0 && rounded > 0 {
            "-"
        } else {
            ""
        };
        let units = rounded / NANO;
        let frac = (rounded % NANO) / step;
        if decimals == 0 {
            format!("{sign}{units}")
        } else {
            format!("{sign}{units}.{frac:0width$}", width = decimals as usize)
        }
    }
}

/// Two amounts of different currencies are never added.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot add {0} and {1}")]
pub struct CurrencyMismatch(pub String, pub String);

/// Observed usage × a price rule: a counterfactual list-price estimate,
/// never spend (CO-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstimatedApiCost(pub Money);

impl EstimatedApiCost {
    pub fn checked_add(&self, other: &Self) -> Result<Self, CurrencyMismatch> {
        if self.0.currency != other.0.currency {
            return Err(CurrencyMismatch(
                self.0.currency.clone(),
                other.0.currency.clone(),
            ));
        }
        Ok(Self(Money::new(
            self.0.nano + other.0.nano,
            &self.0.currency,
        )))
    }
}

/// A cost figure a provider displays locally. Not billing truth, not spend,
/// and never summed: there is no `Add`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderReportedCost(pub Money);

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.display(2), self.currency)
    }
}

/// Parses a plain decimal (`"12.5"`, `"0.25"`) into units of `10^-scale`.
/// Fails on signs, exponents and more fractional digits than `scale`:
/// a price is never rounded on entry.
pub fn parse_scaled(text: &str, scale: u32) -> Option<i128> {
    let (int, frac) = text.split_once('.').unwrap_or((text, ""));
    if int.is_empty()
        || !int.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
        || frac.len() > scale as usize
    {
        return None;
    }
    let int: i128 = int.parse().ok()?;
    let frac_val: i128 = if frac.is_empty() {
        0
    } else {
        frac.parse().ok()?
    };
    let frac_scaled = frac_val * 10i128.pow(scale - frac.len() as u32);
    int.checked_mul(10i128.pow(scale))?.checked_add(frac_scaled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_decimals_only() {
        assert_eq!(parse_scaled("12.5", 3), Some(12_500));
        assert_eq!(parse_scaled("0.25", 3), Some(250));
        assert_eq!(parse_scaled("4", 3), Some(4_000));
        assert_eq!(parse_scaled("0.0001", 3), None);
        assert_eq!(parse_scaled("-1", 3), None);
        assert_eq!(parse_scaled("1e3", 3), None);
        assert_eq!(parse_scaled("", 3), None);
    }

    #[test]
    fn adds_within_one_currency_only() {
        let a = EstimatedApiCost(Money::new(NANO, "USD"));
        let b = EstimatedApiCost(Money::new(NANO / 2, "USD"));
        assert_eq!(a.checked_add(&b).unwrap().0.nano, 1_500_000_000);
        let e = EstimatedApiCost(Money::new(1, "EUR"));
        assert!(a.checked_add(&e).is_err());
    }

    #[test]
    fn displays_rounded() {
        assert_eq!(Money::new(1_234_567_890, "USD").display(2), "1.23");
        assert_eq!(Money::new(1_235_000_000, "USD").display(2), "1.24");
        assert_eq!(Money::new(4_000_000, "USD").display(2), "0.00");
        assert_eq!(Money::new(5_000_000, "USD").display(2), "0.01");
    }
}
