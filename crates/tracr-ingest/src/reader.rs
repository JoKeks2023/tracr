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
