//! WaniKani synchronization, validated cache persistence, and offline summaries.
//!
//! [`wanikani::Client::fetch`] returns owned [`domain::Snapshot`] data. Hold a
//! [`cache::SyncGuard`] across retrieval and call [`cache::SyncGuard::replace`]
//! only after a successful fetch. Cache operations, validation, and summaries
//! are synchronous; only retrieval needs a Tokio runtime with I/O and time enabled.
//!
//! [`cache::load`] and [`domain::Snapshot::summarize`] support offline inspection.
//! Public domain fields allow callers to construct data; loading, replacement,
//! and summarization each validate it. This library does not read credentials or
//! paths from the environment, start a runtime, or print output.

pub mod cache;
pub mod domain;
pub mod summary;
pub mod wanikani;
