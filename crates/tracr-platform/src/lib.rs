//! Platform-specific adapters behind small traits (architecture §10).
//!
//! Only this crate may call OS-specific APIs. The core and the ingestion
//! layer use the traits below. Windows implementations come later; until
//! then non-Unix targets compile against the `unsupported` implementations.

use std::fs::Metadata;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use tracr_core::time::UtcMillis;

pub mod clock;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(unix)]
pub mod unix;
pub mod unsupported;

pub use clock::{Clock, ManualClock, SystemClock};

/// Stable identity of a file across renames (§4.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileId(pub String);

/// Resolves the identity of a file from its metadata.
pub trait FileIdentity: Send + Sync {
    /// `None` when the platform cannot provide a stable identity.
    fn identity(&self, meta: &Metadata) -> Option<FileId>;
}

/// Platform paths (§3.1, §4.2).
pub trait AppPaths: Send + Sync {
    /// Where tracr keeps `ledger.sqlite`, `tracr.lock`, `backups/` and `logs/`.
    fn data_dir(&self) -> Option<PathBuf>;
    /// Provider roots proposed at setup. They are only proposals: discovery
    /// reads a root only after the user approves it (PS-02).
    fn proposed_provider_roots(&self) -> Vec<PathBuf>;
}

pub fn file_identity() -> Box<dyn FileIdentity> {
    #[cfg(unix)]
    {
        Box::new(unix::UnixFileIdentity)
    }
    #[cfg(not(unix))]
    {
        Box::new(unsupported::UnsupportedFileIdentity)
    }
}

pub fn app_paths() -> Box<dyn AppPaths> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacAppPaths)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(unsupported::UnsupportedAppPaths)
    }
}

fn to_millis(time: std::time::SystemTime) -> Option<UtcMillis> {
    let d = time.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(d.as_millis()).ok().map(UtcMillis)
}

/// Last modification time.
pub fn modified_at(meta: &Metadata) -> Option<UtcMillis> {
    meta.modified().ok().and_then(to_millis)
}

/// File creation time where the OS provides it (used for copy election, §8.1).
pub fn created_at(meta: &Metadata) -> Option<UtcMillis> {
    meta.created().ok().and_then(to_millis)
}
