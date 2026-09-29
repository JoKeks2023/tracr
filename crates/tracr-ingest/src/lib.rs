//! Ingestion: the single read path (architecture §4).
//!
//! Every read of a file source is [`reconcile`]: a read from the durable
//! cursor after a continuity check. Filesystem events, when they arrive in
//! a later task, only schedule reconciles; they never read files themselves.
//! Backfill is a reconcile from offset 0.

pub mod cursor;
pub mod discovery;
pub mod reader;
pub mod reconcile;
pub mod source;
pub mod sweep;

pub use discovery::{DiscoveryReport, discover};
pub use reconcile::{
    GapInfo, ReadMode, ReconcileOptions, ReconcileReport, reconcile, record_failed_run,
};
pub use source::{Registry, RunContext};
pub use sweep::{SweepReport, sweep};

use tracr_platform::{Clock, FileIdentity};

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error(transparent)]
    Store(#[from] tracr_store::StoreError),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("no adapter registered for source kind {0}")]
    NoAdapter(String),
    #[error("source {0} has no readable stream metadata")]
    StreamMeta(String),
    /// Test hook: a crash after `n` committed batches (§4.3 crash test).
    #[error("simulated crash after {0} committed batch(es)")]
    SimulatedCrash(usize),
}

pub type Result<T> = std::result::Result<T, IngestError>;

/// Platform services ingestion needs. Injected so tests control time.
#[derive(Clone, Copy)]
pub struct IngestEnv<'a> {
    pub clock: &'a dyn Clock,
    pub file_identity: &'a dyn FileIdentity,
}
