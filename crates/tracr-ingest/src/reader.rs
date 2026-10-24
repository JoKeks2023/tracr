//! Line reader with byte offsets (§4.4). Only lines ending in `\n` are
//! complete; a trailing partial line is reported, never consumed.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, SeekFrom};

pub enum Line {
    /// A complete line; `bytes` includes the terminator.
    Complete {
        start: u64,
        bytes: Vec<u8>,
    },
    /// Bytes after the last `\n` at end of file.
    Partial {
        start: u64,
        bytes: Vec<u8>,
    },
    Eof,
}

pub struct LineReader {
    reader: BufReader<File>,
    pos: u64,
}

impl LineReader {
    pub fn new(mut file: File, start: u64) -> io::Result<Self> {
        file.seek(SeekFrom::Start(start))?;
        Ok(Self {
            reader: BufReader::with_capacity(256 * 1024, file),
            pos: start,
        })
    }

    pub fn next_line(&mut self) -> io::Result<Line> {
        let start = self.pos;
        let mut bytes = Vec::new();
        let n = self.reader.read_until(b'\n', &mut bytes)?;
        self.pos += n as u64;
        Ok(match n {
            0 => Line::Eof,
            _ if bytes.last() == Some(&b'\n') => Line::Complete { start, bytes },
            _ => Line::Partial { start, bytes },
        })
    }

    /// Bytes consumed so far (including a partial line once it was returned).
    pub fn position(&self) -> u64 {
        self.pos
    }
}

/// A file read as its decompressed content (`*.jsonl.zst`).
pub fn is_compressed(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e == "zst")
}

/// Decompresses a zstd file into an anonymous temporary file, positioned at 0.
/// A frame cut off by a writer that is still running keeps the bytes decoded
/// so far (its last line is then a partial line, as with a plain file); a
/// file that yields nothing before failing is an error.
pub fn decompress(file: File) -> io::Result<File> {
    use std::io::{Read, Write};
    let invalid = |e: ruzstd::decoding::errors::FrameDecoderError| {
        io::Error::new(io::ErrorKind::InvalidData, e.to_string())
    };
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(file).map_err(invalid)?;
    let mut out = tempfile::tempfile()?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut written = 0u64;
    loop {
        match decoder.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                out.write_all(&buf[..n])?;
                written += n as u64;
            }
            Err(e) if written == 0 => return Err(e),
            Err(_) => break,
        }
    }
    out.seek(SeekFrom::Start(0))?;
    Ok(out)
}
