//! File snapshots and the continuity proof (§4.3).
//!
//! Continuity holds when: same file identity, size ≥ offset, the same hash
//! over the first `head_len` bytes, and the complete line just before the
//! offset hashes to `last_line_fp`.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use tracr_core::ids::ContentHash;
use tracr_core::time::UtcMillis;
use tracr_platform::{FileId, FileIdentity, created_at, modified_at};
use tracr_store::ledger::SourceCursor;

/// Bytes covered by the head fingerprint (§4.3): the first min(4 KiB, size).
pub const HEAD_LEN: u64 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSnapshot {
    pub path: PathBuf,
    pub identity: Option<FileId>,
    pub size: u64,
    pub mtime: Option<UtcMillis>,
    pub birth: Option<UtcMillis>,
}

/// `None` if nothing (or no regular file) exists at `path`.
pub fn snapshot(path: &Path, fid: &dyn FileIdentity) -> io::Result<Option<FileSnapshot>> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => Ok(Some(FileSnapshot {
            path: path.to_path_buf(),
            identity: fid.identity(&meta),
            size: meta.len(),
            mtime: modified_at(&meta),
            birth: created_at(&meta),
        })),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Snapshot of an already-open file, from its handle. Reconcile opens a file
/// once and judges identity, size and mtime on that handle, so a rename-over
/// between checks cannot mix two files.
pub fn snapshot_of_open(
    path: &Path,
    file: &File,
    fid: &dyn FileIdentity,
) -> io::Result<FileSnapshot> {
    let meta = file.metadata()?;
    Ok(FileSnapshot {
        path: path.to_path_buf(),
        identity: fid.identity(&meta),
        size: meta.len(),
        mtime: modified_at(&meta),
        birth: created_at(&meta),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Break {
    IdentityChanged,
    Truncated,
    HeadChanged,
    LastLineChanged,
}

impl Break {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::IdentityChanged => "identity_changed",
            Self::Truncated => "truncated",
            Self::HeadChanged => "head_changed",
            Self::LastLineChanged => "last_line_changed",
        }
    }

    /// Gap reason if the re-read shows that history was not preserved (§4.3).
    pub fn gap_reason(&self) -> &'static str {
        match self {
            Self::IdentityChanged => "rotated_before_read",
            Self::Truncated => "truncated",
            Self::HeadChanged | Self::LastLineChanged => "replaced_without_history",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continuity {
    Holds,
    Broken(Break),
}

/// Hash of the first min(`HEAD_LEN`, `size`) bytes, and that length.
pub fn head_fingerprint(file: &mut File, size: u64) -> io::Result<(ContentHash, u64)> {
    let len = size.min(HEAD_LEN);
    let mut buf = vec![0u8; len as usize];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut buf)?;
    Ok((ContentHash::of(&buf), len))
}

/// Hash of the complete line that ends exactly at `offset` (terminator
/// included), or `None` if the byte before `offset` is not a `\n`.
pub fn line_fingerprint_ending_at(file: &mut File, offset: u64) -> io::Result<Option<ContentHash>> {
    if offset == 0 {
        return Ok(None);
    }
    let mut last = [0u8; 1];
    file.seek(SeekFrom::Start(offset - 1))?;
    file.read_exact(&mut last)?;
    if last[0] != b'\n' {
        return Ok(None);
    }
    // Scan backwards for the previous line terminator.
    let mut start = 0u64;
    let mut end = offset - 1;
    const CHUNK: u64 = 64 * 1024;
    'scan: while end > 0 {
        let from = end.saturating_sub(CHUNK);
        let mut buf = vec![0u8; (end - from) as usize];
        file.seek(SeekFrom::Start(from))?;
        file.read_exact(&mut buf)?;
        if let Some(i) = buf.iter().rposition(|b| *b == b'\n') {
            start = from + i as u64 + 1;
            break 'scan;
        }
        end = from;
    }
    let mut line = vec![0u8; (offset - start) as usize];
    file.seek(SeekFrom::Start(start))?;
    file.read_exact(&mut line)?;
    Ok(Some(ContentHash::of(&line)))
}

pub fn check_continuity(
    c: &SourceCursor,
    snap: &FileSnapshot,
    file: &mut File,
) -> io::Result<Continuity> {
    if let (Some(before), Some(now)) = (c.file_identity.as_deref(), snap.identity.as_ref())
        && before != now.0
    {
        return Ok(Continuity::Broken(Break::IdentityChanged));
    }
    if snap.size < c.byte_offset {
        return Ok(Continuity::Broken(Break::Truncated));
    }
    if let (Some(fp), Some(len)) = (c.head_fp, c.head_len) {
        if snap.size < len {
            return Ok(Continuity::Broken(Break::HeadChanged));
        }
        let mut buf = vec![0u8; len as usize];
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut buf)?;
        if ContentHash::of(&buf) != fp {
            return Ok(Continuity::Broken(Break::HeadChanged));
        }
    }
    if c.byte_offset > 0 {
        let now = line_fingerprint_ending_at(file, c.byte_offset)?;
        if now.is_none() || now != c.last_line_fp {
            return Ok(Continuity::Broken(Break::LastLineChanged));
        }
    }
    Ok(Continuity::Holds)
}
