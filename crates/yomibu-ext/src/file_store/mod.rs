//! File-backed storage with validated atomic replacement.
pub const COMPONENT: yomibu_core::extension::Component = yomibu_core::extension::Component {
    id: "file-learning-store",
    settings: &[],
};

pub mod cache;

const CACHE_FILE: &str = "wanikani.json";

use crate::wanikani::data::WaniKaniSyncData;
use std::path::PathBuf;
use yomibu_core::{
    domain::knowledge::learner::LearnerData,
    ports::infrastructure::{LearningStore, SourceSyncWriter},
};

/// One WaniKani account per cache file, persisted using cache schema 1.
#[derive(Debug, Clone)]
pub struct FileLearningStore {
    path: PathBuf,
}

impl FileLearningStore {
    /// Configure a path without creating or reading files.
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            path: data_dir.into().join(CACHE_FILE),
        }
    }

    pub fn load_snapshot(&self) -> Result<WaniKaniSyncData, cache::CacheError> {
        cache::load_path(&self.path)
    }
}

impl LearningStore for FileLearningStore {
    type WriteError = cache::WriteError;
    type Writer = FileSyncWriter;

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
    type Data = WaniKaniSyncData;
    type Error = cache::WriteError;

    fn replace(self, data: Self::Data) -> Result<(), Self::Error> {
        self.guard.replace(&data)
    }
}

impl FileLearningStore {
    /// Read normalized data without writes. Only an absent cache returns `None`.
    pub fn read_learner_data(&self) -> Result<Option<LearnerData>, cache::CacheError> {
        let source = match self.load_snapshot() {
            Ok(source) => source,
            Err(cache::CacheError::Missing { .. }) => return Ok(None),
            Err(error) => return Err(error),
        };
        let records = crate::wanikani::map_content_and_progress(&source)
            .map_err(|error| cache::CacheError::Invalid {
                path: self.path.clone(),
                source: error,
            })?
            .collect();
        let subscription = &source.learner.subscription;
        Ok(Some(LearnerData {
            records,
            access_expires_at: subscription
                .active
                .then_some(subscription.period_ends_at)
                .flatten(),
        }))
    }
}
