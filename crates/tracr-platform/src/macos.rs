//! macOS implementations (architecture §10).

use std::path::PathBuf;

use crate::AppPaths;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// `~/Library/Application Support/tracr` and the proposed provider roots (§4.2).
#[derive(Debug, Default, Clone, Copy)]
pub struct MacAppPaths;

impl AppPaths for MacAppPaths {
    fn data_dir(&self) -> Option<PathBuf> {
        home().map(|h| h.join("Library/Application Support/tracr"))
    }

    fn proposed_provider_roots(&self) -> Vec<PathBuf> {
        let Some(h) = home() else { return Vec::new() };
        vec![
            h.join(".claude/projects"),
            h.join(".codex/sessions"),
            h.join(".codex/archived_sessions"),
        ]
    }
}
