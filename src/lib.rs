//! WaniKani synchronization, offline summaries, and manual candidate preview.
//!
//! [`preview::preview`] synchronously selects structured word entries and checks
//! membership/count. It borrows explicit inputs and needs no account, store, or
//! runtime. Grammar and linguistic correctness remain explicitly unassessed.
//!
//! [`App`] coordinates explicit synchronization and offline status for a single
//! account-scoped store. Supply a file or in-memory store and, for synchronization,
//! a source. There is no implicit sync or current-user selection.
//!
//! ```no_run
//! use yomibu::{App, adapters::{sources::wanikani::Client, stores::FileLearningStore}};
//!
//! # async fn example(token: &str) -> Result<(), Box<dyn std::error::Error>> {
//! let mut app = App::new(FileLearningStore::new("/path/to/yomibu-data"))
//!     .with_source(Client::new(token)?);
//! let report = app.sync().await?;
//! let summary = app.status()?;
//! assert_eq!(report.summary, summary);
//! # Ok(())
//! # }
//! ```
//!
//! Retrieval needs a caller-owned Tokio runtime with I/O and time enabled. File
//! operations, validation, and summaries are synchronous. Read [`App::sync`]
//! for cancellation and persistence outcomes; backend errors remain typed.
//! [`app::SyncReport`] distinguishes volatile retention from durable persistence.
//!
//! Lower-level access remains available through [`wanikani::Client::fetch`],
//! [`cache::SyncGuard`], and [`cache::load`]. Direct callers must keep their writer
//! guard alive across retrieval and replacement. Root `cache` and `wanikani`
//! module paths re-export the adapters for existing callers.
//!
//! [`cache::load`] and [`domain::WaniKaniSyncData::summarize`] support offline inspection.
//! Public domain fields allow callers to construct data; loading, replacement,
//! and summarization each validate it. This library does not read credentials or
//! paths from the environment, start a runtime, or print output.

pub mod adapters;
pub mod app;
pub use adapters::stores::file::cache;
pub use app::App;
pub mod domain;
pub mod grammar;
pub mod ports;
pub mod preview;
pub mod summary;
pub use adapters::sources::wanikani;
