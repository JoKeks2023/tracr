//! Forward-only, additive ledger migrations (§3.1).
//!
//! Migrations are embedded SQL files tracked by `PRAGMA user_version`.
//! Before migrating an existing ledger, a backup is written with
//! `VACUUM INTO` (never a raw file copy: recent commits may sit in the WAL).

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::{Result, StoreError};

#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const LEDGER_MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "init",
    sql: include_str!("../migrations/ledger/0001_init.sql"),
}];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub from_version: i64,
    pub to_version: i64,
    pub backup: Option<PathBuf>,
}

pub fn user_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// Applies the ledger migrations.
pub fn migrate_ledger(
    conn: &mut Connection,
    backup_dir: &Path,
    stamp: &str,
) -> Result<MigrationReport> {
    migrate(conn, LEDGER_MIGRATIONS, backup_dir, stamp)
}

/// Applies `migrations` newer than the current `user_version`, each in its
/// own transaction. An existing, non-empty ledger is backed up first.
pub fn migrate(
    conn: &mut Connection,
    migrations: &[Migration],
    backup_dir: &Path,
    stamp: &str,
) -> Result<MigrationReport> {
    let from = user_version(conn)?;
    let latest = migrations.iter().map(|m| m.version).max().unwrap_or(0);
    if from > latest {
        return Err(StoreError::Invalid(format!(
            "ledger schema v{from} is newer than this tracr build (v{latest})"
        )));
    }
    let pending: Vec<&Migration> = migrations.iter().filter(|m| m.version > from).collect();
    if pending.is_empty() {
        return Ok(MigrationReport {
            from_version: from,
            to_version: from,
            backup: None,
        });
    }
    let backup = if from > 0 {
        std::fs::create_dir_all(backup_dir)?;
        let path = backup_dir.join(format!("ledger-premigration-v{from}-{stamp}.sqlite"));
        conn.execute("VACUUM INTO ?1", [path.to_string_lossy().as_ref()])?;
        Some(path)
    } else {
        None
    };
    for m in pending {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
    }
    Ok(MigrationReport {
        from_version: from,
        to_version: latest,
        backup,
    })
}
