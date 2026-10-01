//! Provider-neutral vocabulary of tracr.
//!
//! This crate performs no IO, reads no clock, runs no SQL and names no
//! provider (architecture AD-01, §1.2). Adapters implement the traits in
//! [`adapter`]; the ingestion layer stores what they extract.

pub mod adapter;
pub mod agent;
pub mod copy;
pub mod counting;
pub mod coverage;
pub mod device;
pub mod events;
pub mod evidence;
pub mod human;
pub mod ids;
pub mod manifest;
pub mod normalize;
pub mod time;
