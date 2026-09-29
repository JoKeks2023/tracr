//! The single ledger connection factory (§3.1, §3.3).
//!
//! Every ledger connection gets WAL, `synchronous=FULL`, `foreign_keys=ON`
//! and `recursive_triggers=ON`. The last one matters for immutability:
//! without it, `REPLACE` deletes rows without firing DELETE triggers.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::{Result, StoreError};

pub const LEDGER_FILE: &str = "ledger.sqlite";

/// Opens (creating if needed) a ledger file with the required pragmas.
pub fn open_ledger(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    configure_ledger(&conn)?;
    Ok(conn)
}

/// Opens an existing ledger read-only. Readers never create or migrate.
pub fn open_ledger_read_only(path: &Path) -> Result<Connection> {
    if !path.exists() {
        return Err(StoreError::Invalid(format!(
            "no ledger at {}; run `tracr import` first",
            path.display()
        )));
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_secs(10))?;
    Ok(conn)
}

pub fn configure_ledger(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_secs(10))?;
    let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(StoreError::Invalid(format!(
            "ledger journal_mode is {mode}, expected wal"
        )));
    }
    conn.execute_batch(
        "PRAGMA synchronous = FULL;
         PRAGMA foreign_keys = ON;
         PRAGMA recursive_triggers = ON;",
    )?;
    Ok(())
}

/// The pragmas in force on a connection (for tests and diagnostics).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerPragmas {
    pub journal_mode: String,
    pub synchronous: i64,
    pub foreign_keys: bool,
    pub recursive_triggers: bool,
}

pub fn ledger_pragmas(conn: &Connection) -> Result<LedgerPragmas> {
    Ok(LedgerPragmas {
        journal_mode: conn.query_row("PRAGMA journal_mode", [], |r| r.get(0))?,
        synchronous: conn.query_row("PRAGMA synchronous", [], |r| r.get(0))?,
        foreign_keys: conn.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0))? == 1,
        recursive_triggers: conn
            .query_row("PRAGMA recursive_triggers", [], |r| r.get::<_, i64>(0))?
            == 1,
    })
}
