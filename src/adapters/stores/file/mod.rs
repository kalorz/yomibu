//! File-backed storage with validated atomic replacement.
pub mod cache;

use crate::{
    domain::WaniKaniSyncData,
    ports::{LearningStore, Persistence, SourceSyncWriter},
};
use std::{path::PathBuf, sync::Arc};

/// One WaniKani account per directory, persisted using cache schema 1.
#[derive(Debug, Clone)]
pub struct FileLearningStore {
    data_dir: PathBuf,
}

impl FileLearningStore {
    /// Configure a path without creating or reading files.
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }
}

impl LearningStore for FileLearningStore {
    type ReadError = cache::CacheError;
    type WriteError = cache::WriteError;
    type Writer = FileSyncWriter;

    fn load(&self) -> Result<Arc<WaniKaniSyncData>, Self::ReadError> {
        cache::load(&self.data_dir).map(Arc::new)
    }

    fn begin_sync(&self) -> Result<Self::Writer, Self::WriteError> {
        cache::SyncGuard::acquire(&self.data_dir).map(|guard| FileSyncWriter { guard })
    }
}

/// Owns the file lock until publication, failure, or cancellation.
#[derive(Debug)]
pub struct FileSyncWriter {
    guard: cache::SyncGuard,
}

impl SourceSyncWriter for FileSyncWriter {
    type Error = cache::WriteError;

    fn replace(self, data: WaniKaniSyncData) -> Result<Persistence, Self::Error> {
        self.guard.replace(&data)?;
        Ok(Persistence::Durable)
    }
}
