//! Instants and half-open intervals (architecture §3.1 "Time").

use std::fmt;

use serde::{Deserialize, Serialize};

/// A UTC instant in epoch milliseconds, the precision the sources provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UtcMillis(pub i64);

impl UtcMillis {
    /// Parses an RFC 3339 timestamp such as `2026-09-28T14:05:00.123Z`.
    ///
    /// Returns `None` for anything it cannot parse. A missing time is unknown,
    /// never zero (spec concepts §3).
    pub fn parse_rfc3339(text: &str) -> Option<Self> {
        text.parse::<jiff::Timestamp>()
            .ok()
            .map(|t| Self(t.as_millisecond()))
    }

    pub fn as_millis(self) -> i64 {
        self.0
    }

    pub fn to_rfc3339(self) -> String {
        jiff::Timestamp::from_millisecond(self.0)
            .map(|t| t.to_string())
            .unwrap_or_else(|_| format!("{}ms", self.0))
    }
}

impl fmt::Display for UtcMillis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

/// Half-open interval `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HalfOpen {
    pub start: UtcMillis,
    pub end: UtcMillis,
}

impl HalfOpen {
    /// Returns `None` when `end < start`.
    pub fn new(start: UtcMillis, end: UtcMillis) -> Option<Self> {
        (start <= end).then_some(Self { start, end })
    }

    pub fn duration_ms(&self) -> i64 {
        self.end.0 - self.start.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3339_with_millis() {
        let t = UtcMillis::parse_rfc3339("2026-09-28T14:05:00.123Z").unwrap();
        assert_eq!(t.0 % 1000, 123);
        assert_eq!(t.to_rfc3339(), "2026-09-28T14:05:00.123Z");
    }

    #[test]
    fn unparseable_time_is_unknown_not_zero() {
        assert_eq!(UtcMillis::parse_rfc3339("yesterday"), None);
        assert_eq!(UtcMillis::parse_rfc3339(""), None);
    }

    #[test]
    fn half_open_rejects_reversed_bounds() {
        assert!(HalfOpen::new(UtcMillis(10), UtcMillis(5)).is_none());
        assert_eq!(
            HalfOpen::new(UtcMillis(5), UtcMillis(10))
                .unwrap()
                .duration_ms(),
            5
        );
    }
}
