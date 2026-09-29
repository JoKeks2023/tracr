//! Identifiers, content hashes and record keys (architecture §3.2, §6.1).
//!
//! Evidence and user records carry opaque UUIDv7 identifiers. Generating
//! them needs a clock, so generation lives outside this crate.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// Opaque UUIDv7 of an observation.
    ObservationId
);
string_id!(
    /// Opaque UUIDv7 of a source (one logical evidence stream).
    SourceId
);
string_id!(
    /// Installation identity of the machine that collects evidence.
    MachineId
);

/// BLAKE3 hash of a record's bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    pub fn of(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// Hash of one line without its terminator: CRLF and LF lines hash the same (§3.2).
    pub fn of_line(line: &[u8]) -> Self {
        Self::of(strip_line_terminator(line))
    }

    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        <[u8; 32]>::try_from(bytes).ok().map(Self)
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({})", self.to_hex())
    }
}

/// Removes a trailing `\n` and a `\r` before it.
pub fn strip_line_terminator(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// Key of an observation within its source (§3.2): the native record ID
/// where the source provides one, else `h:<content hash>#<occurrence>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordKey(String);

impl RecordKey {
    /// A key from a native record ID. A native ID that happens to start with
    /// the content prefix is escaped so the two namespaces never collide.
    pub fn native(id: &str) -> Self {
        if id.starts_with("h:") || id.starts_with("n:") {
            Self(format!("n:{id}"))
        } else {
            Self(id.to_owned())
        }
    }

    /// A key for a record without native identity: the n-th occurrence
    /// (0-based, in file order) of this exact content in the source.
    pub fn content_derived(hash: &ContentHash, occurrence: u32) -> Self {
        Self(format!("h:{}#{occurrence}", hash.to_hex()))
    }

    pub fn from_stored(key: String) -> Self {
        Self(key)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_content_derived(&self) -> bool {
        self.0.starts_with("h:")
    }
}

/// Deterministic ID of a derived record (AD-09, §6.1): `prefix` followed by
/// the hex of the first 128 bits of BLAKE3 over the key parts. Parts are
/// length-prefixed, so `["ab", "c"]` and `["a", "bc"]` never collide.
pub fn derived_id(prefix: &str, parts: &[&str]) -> String {
    let mut h = blake3::Hasher::new();
    for p in parts {
        h.update(&(p.len() as u64).to_le_bytes());
        h.update(p.as_bytes());
    }
    let hex: String = h.finalize().as_bytes()[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{prefix}{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_ids_are_deterministic_and_unambiguous() {
        assert_eq!(
            derived_id("ses_", &["a", "b"]),
            derived_id("ses_", &["a", "b"])
        );
        assert_ne!(
            derived_id("ses_", &["ab", "c"]),
            derived_id("ses_", &["a", "bc"])
        );
        let id = derived_id("trn_", &["x"]);
        assert!(id.starts_with("trn_") && id.len() == 4 + 32);
    }

    #[test]
    fn crlf_and_lf_lines_hash_the_same() {
        assert_eq!(
            ContentHash::of_line(b"{\"a\":1}\r\n"),
            ContentHash::of_line(b"{\"a\":1}\n")
        );
        assert_eq!(
            ContentHash::of_line(b"{\"a\":1}\n"),
            ContentHash::of(b"{\"a\":1}")
        );
    }

    #[test]
    fn native_and_content_keys_never_collide() {
        let h = ContentHash::of(b"x");
        let content = RecordKey::content_derived(&h, 0);
        assert!(content.is_content_derived());
        let sneaky = RecordKey::native(content.as_str());
        assert_ne!(sneaky, content);
        assert!(!sneaky.is_content_derived());
    }
}
