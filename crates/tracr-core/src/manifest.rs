//! Capability manifests and producer-version gating (architecture AD-19).
//!
//! A manifest declares the producer-version ranges its rules were
//! fixture-validated for. A record from another version is processed by the
//! nearest validated range (the nearest lower one, else the lowest) and is
//! flagged as not fixture-validated. Nothing is silently treated as validated.

use std::cmp::Ordering;
use std::fmt;

/// A dotted numeric producer version such as `2.1.281`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProducerVersion(Vec<u32>);

impl ProducerVersion {
    /// Parses `major.minor.patch…`. A version with any non-numeric part is
    /// not parseable and therefore unvalidated.
    pub fn parse(text: &str) -> Option<Self> {
        let parts: Option<Vec<u32>> = text.split('.').map(|p| p.parse().ok()).collect();
        parts.filter(|p| !p.is_empty()).map(Self)
    }
}

impl Ord for ProducerVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let len = self.0.len().max(other.0.len());
        for i in 0..len {
            let a = self.0.get(i).copied().unwrap_or(0);
            let b = other.0.get(i).copied().unwrap_or(0);
            match a.cmp(&b) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for ProducerVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for ProducerVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text: Vec<String> = self.0.iter().map(u32::to_string).collect();
        f.write_str(&text.join("."))
    }
}

/// An inclusive producer-version range the rules were fixture-validated for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRange {
    pub min: ProducerVersion,
    pub max: ProducerVersion,
}

impl VersionRange {
    /// Panics on unparseable bounds: ranges are compile-time manifest data.
    pub fn new(min: &str, max: &str) -> Self {
        let min = ProducerVersion::parse(min).expect("manifest range min");
        let max = ProducerVersion::parse(max).expect("manifest range max");
        assert!(min <= max, "manifest range min must not exceed max");
        Self { min, max }
    }

    pub fn contains(&self, version: &ProducerVersion) -> bool {
        &self.min <= version && version <= &self.max
    }

    pub fn label(&self) -> String {
        format!("{}..={}", self.min, self.max)
    }
}

/// A capability that is off for unvalidated producer versions, with its fallback (AD-19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatedCapability {
    pub name: &'static str,
    pub fallback: &'static str,
}

/// What an adapter declares about its rules and their validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityManifest {
    /// Adapter identity, e.g. a transcript adapter name.
    pub adapter: &'static str,
    /// Version of the extraction rules; stored with every retained fact row.
    pub extractor_version: &'static str,
    /// Ranges with committed fixtures that pass the extraction tests.
    pub validated_ranges: Vec<VersionRange>,
    /// Values of the provider's interactive-source field that may yield
    /// human engagement (AD-19). Any other value never does.
    pub interactive_values: Vec<&'static str>,
    /// Record types that are irrelevant and produce no observation (§4.4).
    pub ignored_record_types: Vec<&'static str>,
    /// Record types the extractor knows; any other type is quarantined.
    pub known_record_types: Vec<&'static str>,
    pub fixture_gated: Vec<GatedCapability>,
    /// Whether documented provider semantics identify the main agent (AD-21).
    pub main_actor_documented: bool,
}

/// Which validated range applies to a record (AD-19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestMatch {
    Validated(VersionRange),
    /// Processed by `applied`, but the producer version is not fixture-validated.
    Unvalidated {
        applied: VersionRange,
        version: Option<String>,
    },
    /// The manifest declares no validated range at all.
    NoValidatedRange {
        version: Option<String>,
    },
}

impl ManifestMatch {
    pub fn is_validated(&self) -> bool {
        matches!(self, Self::Validated(_))
    }

    /// Stored in `observation.manifest_range`.
    pub fn label(&self) -> String {
        match self {
            Self::Validated(r) => r.label(),
            Self::Unvalidated { applied, version } => format!(
                "unvalidated({})->{}",
                version.as_deref().unwrap_or("none"),
                applied.label()
            ),
            Self::NoValidatedRange { version } => {
                format!(
                    "unvalidated({})->none",
                    version.as_deref().unwrap_or("none")
                )
            }
        }
    }
}

impl CapabilityManifest {
    pub fn select(&self, version: Option<&str>) -> ManifestMatch {
        let parsed = version.and_then(ProducerVersion::parse);
        let owned = version.map(str::to_owned);
        let Some(lowest) = self
            .validated_ranges
            .iter()
            .min_by(|a, b| a.min.cmp(&b.min))
        else {
            return ManifestMatch::NoValidatedRange { version: owned };
        };
        let Some(v) = parsed else {
            return ManifestMatch::Unvalidated {
                applied: lowest.clone(),
                version: owned,
            };
        };
        if let Some(r) = self.validated_ranges.iter().find(|r| r.contains(&v)) {
            return ManifestMatch::Validated(r.clone());
        }
        let nearest_lower = self
            .validated_ranges
            .iter()
            .filter(|r| r.max < v)
            .max_by(|a, b| a.max.cmp(&b.max));
        ManifestMatch::Unvalidated {
            applied: nearest_lower.unwrap_or(lowest).clone(),
            version: owned,
        }
    }

    pub fn is_interactive(&self, value: Option<&str>) -> bool {
        value.is_some_and(|v| self.interactive_values.contains(&v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(ranges: Vec<VersionRange>) -> CapabilityManifest {
        CapabilityManifest {
            adapter: "test",
            extractor_version: "test/1",
            validated_ranges: ranges,
            interactive_values: vec!["cli"],
            ignored_record_types: vec![],
            known_record_types: vec![],
            fixture_gated: vec![],
            main_actor_documented: false,
        }
    }

    #[test]
    fn versions_compare_numerically() {
        let a = ProducerVersion::parse("2.1.9").unwrap();
        let b = ProducerVersion::parse("2.1.10").unwrap();
        assert!(a < b);
        assert_eq!(ProducerVersion::parse("2.1.x"), None);
    }

    #[test]
    fn selection_follows_ad19() {
        let m = manifest(vec![
            VersionRange::new("1.0.0", "1.2.0"),
            VersionRange::new("2.0.0", "2.3.0"),
        ]);
        assert!(m.select(Some("2.1.5")).is_validated());
        match m.select(Some("2.9.0")) {
            ManifestMatch::Unvalidated { applied, .. } => {
                assert_eq!(applied.label(), "2.0.0..=2.3.0")
            }
            other => panic!("{other:?}"),
        }
        match m.select(Some("1.5.0")) {
            ManifestMatch::Unvalidated { applied, .. } => {
                assert_eq!(applied.label(), "1.0.0..=1.2.0")
            }
            other => panic!("{other:?}"),
        }
        match m.select(Some("0.1.0")) {
            ManifestMatch::Unvalidated { applied, .. } => {
                assert_eq!(applied.label(), "1.0.0..=1.2.0")
            }
            other => panic!("{other:?}"),
        }
        let none = m.select(None);
        assert!(!none.is_validated());
        assert_eq!(none.label(), "unvalidated(none)->1.0.0..=1.2.0");
    }

    #[test]
    fn interactive_requires_a_listed_value() {
        let m = manifest(vec![]);
        assert!(m.is_interactive(Some("cli")));
        assert!(!m.is_interactive(Some("sdk")));
        assert!(!m.is_interactive(None));
    }
}
