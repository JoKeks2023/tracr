//! Honest fallbacks for platforms without an implementation yet. They report
//! "unavailable" instead of guessing; this is not a Windows implementation.

use std::fs::Metadata;
use std::path::PathBuf;

use crate::{AppPaths, FileId, FileIdentity};

#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedFileIdentity;

impl FileIdentity for UnsupportedFileIdentity {
    fn identity(&self, _meta: &Metadata) -> Option<FileId> {
        None
    }
}

/// No default data directory: callers must pass one explicitly.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedAppPaths;

impl AppPaths for UnsupportedAppPaths {
    fn data_dir(&self) -> Option<PathBuf> {
        None
    }

    fn proposed_provider_roots(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}
