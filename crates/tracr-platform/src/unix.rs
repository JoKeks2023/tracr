//! Unix implementations (macOS in 0.9; Linux for CI).

use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;

use crate::{FileId, FileIdentity};

/// File identity = `(st_dev, st_ino)` (§4.3).
#[derive(Debug, Default, Clone, Copy)]
pub struct UnixFileIdentity;

impl FileIdentity for UnixFileIdentity {
    fn identity(&self, meta: &Metadata) -> Option<FileId> {
        Some(FileId(format!("{}:{}", meta.dev(), meta.ino())))
    }
}
