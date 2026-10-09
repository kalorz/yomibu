//! File-backed storage with validated atomic replacement.
pub const COMPONENT: yomibu_core::capabilities::options::Component =
    yomibu_core::capabilities::options::Component {
        id: "file-learning-store",
        settings: &[],
    };

pub mod cache;

use std::{path::PathBuf, sync::Arc};
use yomibu_core::{
    capabilities::{LearningStore, Persistence, SourceSyncWriter},
    domain::source::WaniKaniSyncData,
};

/// One WaniKani account per cache file, persisted using cache schema 1.
#[derive(Debug, Clone)]
pub struct FileLearningStore {
    path: PathBuf,
}

impl FileLearningStore {
    /// Configure a path without creating or reading files.
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self::at_path(data_dir.into().join("wanikani.json"))
    }

    pub fn at_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn load_snapshot(&self) -> Result<WaniKaniSyncData, cache::CacheError> {
        cache::load_path(&self.path)
    }
}

impl LearningStore for FileLearningStore {
    type ReadError = cache::CacheError;
    type WriteError = cache::WriteError;
    type Writer = FileSyncWriter;

    fn load(&self) -> Result<Arc<WaniKaniSyncData>, Self::ReadError> {
        self.load_snapshot().map(Arc::new)
    }

    fn begin_sync(&self) -> Result<Self::Writer, Self::WriteError> {
        cache::SyncGuard::acquire_path(&self.path).map(|guard| FileSyncWriter { guard })
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
        self.replace_snapshot(&data)
    }
}

impl FileSyncWriter {
    pub fn replace_snapshot(
        self,
        data: &WaniKaniSyncData,
    ) -> Result<Persistence, cache::WriteError> {
        self.guard.replace(data)?;
        Ok(Persistence::Durable)
    }
}
