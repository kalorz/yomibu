use crate::reports::summary::Summary;
use thiserror::Error;
use yomibu_core::{
    domain::knowledge::source::ValidationError,
    ports::infrastructure::{LearningSource, LearningStore, SourceSyncWriter},
};

/// Owns the selected store and optional source. Construction performs no I/O.
///
/// No implicit current learner, environment access, runtime, or background work.
/// The current account scope is the store itself, not a mutable user selection.
pub struct App<Store, Source = ()> {
    store: Store,
    source: Source,
}

impl<Store> App<Store> {
    pub fn new(store: Store) -> Self {
        Self { store, source: () }
    }

    /// Supply an explicit source; the client can be reused for successive syncs.
    pub fn with_source<Source>(self, source: Source) -> App<Store, Source> {
        App {
            store: self.store,
            source,
        }
    }
}

impl<Store: LearningStore, Source: LearningSource> App<Store, Source> {
    /// Reserve a writer, fetch, validate, then atomically publish one version.
    ///
    /// Source/validation failures preserve the previous data. A write failure
    /// may mean uncertain durability after replacement: inspect the backend's
    /// typed error. A successful result has no remaining fallible post-write work.
    /// Dropping this future releases the writer. Synchronous store operations
    /// cannot be interrupted mid-call; a drop is not a rollback guarantee.
    ///
    /// File operations currently run on the caller's thread. A future busy server
    /// needs storage contracts suited to async I/O or a bounded blocking executor.
    pub async fn sync(
        &mut self,
    ) -> Result<SyncReport, SyncError<Source::Error, Store::WriteError>> {
        let writer = self.store.begin_sync().map_err(SyncError::Write)?;
        let data = self.source.fetch().await.map_err(SyncError::Source)?;
        let summary = crate::reports::summary::summarize(&data).map_err(SyncError::InvalidData)?;
        writer.replace(data.into()).map_err(SyncError::Write)?;
        Ok(SyncReport { summary })
    }
}

/// A successful, complete publication; retention depends on the selected store.
#[derive(Debug, PartialEq, serde::Serialize)]
pub struct SyncReport {
    pub summary: Summary,
}

#[derive(Debug, Error)]
pub enum SyncError<SourceError, WriteError> {
    #[error(transparent)]
    Source(SourceError),
    #[error(transparent)]
    Write(WriteError),
    #[error("Invalid source data; the previous version has not been replaced: {0}")]
    InvalidData(#[source] ValidationError),
}
