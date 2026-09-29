//! Persistence for tracr (architecture §3).
//!
//! `ledger.sqlite` holds evidence and user records and is never rebuilt.
//! Immutability is enforced by triggers in the schema, not by convention.

pub mod connection;
pub mod ledger;
pub mod migrate;

pub use rusqlite;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;
