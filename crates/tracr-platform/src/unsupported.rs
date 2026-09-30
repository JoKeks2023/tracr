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

/// No device sensor on this platform yet: every signal is unavailable,
/// which the device lane shows as `unknown` (§10), never as idle or active.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedDeviceSensor;

impl crate::DeviceSensor for UnsupportedDeviceSensor {
    fn name(&self) -> &'static str {
        "unsupported"
    }

    fn sample(&self, now: tracr_core::time::UtcMillis) -> tracr_core::device::DeviceSample {
        tracr_core::device::DeviceSample {
            at: now,
            idle_ms: None,
            locked: None,
            last_sleep_wake: None,
        }
    }
}
