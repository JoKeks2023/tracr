//! Test kit (dev only, architecture §12.1): schema-faithful provider lines,
//! scenario time and file manipulations for ingestion scenarios.

pub mod claude;
pub mod codex;
pub mod collector;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Creates or truncates `path` and writes `lines`, each terminated by `\n`.
pub fn write_lines(path: &Path, lines: &[String]) {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).unwrap();
    }
    let mut text = String::new();
    for l in lines {
        text.push_str(l);
        text.push('\n');
    }
    fs::write(path, text).unwrap();
}

/// Appends complete lines, the way a provider appends to its transcript.
pub fn append_lines(path: &Path, lines: &[String]) {
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .unwrap();
    for l in lines {
        f.write_all(l.as_bytes()).unwrap();
        f.write_all(b"\n").unwrap();
    }
}

/// Appends raw bytes (e.g. a partially flushed line).
pub fn append_raw(path: &Path, bytes: &[u8]) {
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .unwrap();
    f.write_all(bytes).unwrap();
}

/// Atomic replacement: write a temporary file, rename it over `path`
/// (walkthrough F2). The result has a new file identity.
pub fn atomic_replace(path: &Path, content: &[u8]) {
    let tmp = path.with_extension("jsonl.tmp");
    fs::write(&tmp, content).unwrap();
    fs::rename(&tmp, path).unwrap();
}

/// Rotation (walkthrough F3): the old file is renamed away, a new file is
/// created at `path`, then the old one is deleted. The old file still exists
/// while the new one is created, so the new file cannot reuse its inode.
pub fn rotate(path: &Path, new_lines: &[String]) {
    let old = path.with_extension("jsonl.rotated");
    fs::rename(path, &old).unwrap();
    write_lines(path, new_lines);
    fs::remove_file(&old).unwrap();
}
