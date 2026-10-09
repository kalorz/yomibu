//! Volatile storage sharing immutable versions across explicitly cloned handles.

pub const COMPONENT: yomibu_core::capabilities::options::Component =
    yomibu_core::capabilities::options::Component {
        id: "in-memory-learning-store",
        settings: &[],
    };

use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};
use thiserror::Error;
use yomibu_core::{
    capabilities::{LearningStore, Persistence, SourceSyncWriter},
    domain::source::{ValidationError, WaniKaniSyncData},
};

#[derive(Debug, Error)]
pub enum InMemoryStoreError {
    #[error("No synchronized data in this in-memory store.")]
    Missing,
    #[error("The in-memory store is unavailable after a panic during access.")]
    Unavailable,
    #[error("Invalid new sync data; the previous version has not been replaced: {0}")]
    InvalidSyncData(#[from] ValidationError),
    #[error("This store belongs to a different WaniKani account; use a separate store.")]
    AccountMismatch,
    #[error("Another sync holds this in-memory store's writer reservation.")]
    Locked,
}

/// Real volatile storage. Cloning shares data; constructing a new store isolates it.
#[derive(Clone, Default)]
pub struct InMemoryLearningStore {
    state: Arc<State>,
}

#[derive(Default)]
struct State {
    data: RwLock<Option<Arc<WaniKaniSyncData>>>,
    writing: AtomicBool,
}

impl InMemoryLearningStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl LearningStore for InMemoryLearningStore {
    type ReadError = InMemoryStoreError;
    type WriteError = InMemoryStoreError;
    type Writer = InMemorySyncWriter;

    fn load(&self) -> Result<Arc<WaniKaniSyncData>, Self::ReadError> {
        self.state
            .data
            .read()
            .map_err(|_| InMemoryStoreError::Unavailable)?
            .as_ref()
            .map(Arc::clone)
            .ok_or(InMemoryStoreError::Missing)
    }

    fn begin_sync(&self) -> Result<Self::Writer, Self::WriteError> {
        self.state
            .writing
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| InMemoryStoreError::Locked)?;
        Ok(InMemorySyncWriter {
            state: Arc::clone(&self.state),
        })
    }
}

/// Owns the writer reservation without holding a data lock across network I/O.
pub struct InMemorySyncWriter {
    state: Arc<State>,
}

impl Drop for InMemorySyncWriter {
    fn drop(&mut self) {
        self.state.writing.store(false, Ordering::Release);
    }
}

impl SourceSyncWriter for InMemorySyncWriter {
    type Error = InMemoryStoreError;

    fn replace(self, data: WaniKaniSyncData) -> Result<Persistence, Self::Error> {
        data.validate()?;
        let mut current = self
            .state
            .data
            .write()
            .map_err(|_| InMemoryStoreError::Unavailable)?;
        if let Some(previous) = current.as_ref()
            && previous.learner.id != data.learner.id
        {
            return Err(InMemoryStoreError::AccountMismatch);
        }
        *current = Some(Arc::new(data));
        Ok(Persistence::Volatile)
    }
}
