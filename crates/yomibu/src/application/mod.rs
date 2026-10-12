//! Explicit synchronization for one account-scoped store.

mod construction;
mod credentials;
pub use credentials::{CredentialError, Credentials, Secret};
mod inputs;
mod local;
pub use local::{ApplicationError, LocalApp, ServiceEndpoints, SetupIssue};
pub mod input_file;
pub mod progress;
pub mod selection;
mod source;
pub use source::cache_path as source_cache_path;
mod story_workflow;
pub use story_workflow::{StoryInputs, StoryResources, run_story};

use crate::reports::summary::Summary;
use thiserror::Error;
use yomibu_core::{
    capabilities::{LearningSource, LearningStore, Persistence, SourceSyncWriter},
    domain::source::ValidationError,
};

#[derive(Debug)]
pub enum Operation {
    Auth,
    Story,
    Sync,
}

impl Operation {
    pub(crate) fn uses_setting(&self, name: &str) -> bool {
        use Operation::*;
        if matches!(self, Auth) {
            return matches!(
                name,
                "enable"
                    | "disable"
                    | "application.sync"
                    | "pipeline.components.source"
                    | "pipeline.components.generation"
            );
        }
        if let Some(option) = name.strip_prefix("pipeline.options.") {
            return matches!(
                option.split_once('.').map(|(component, _)| component),
                Some(yomibu_components::openai_story_generation::ID)
            ) && matches!(self, Story);
        }
        match name {
            "pipeline.components.source"
            | "pipeline.components.learning_store"
            | "application.wanikani_cache" => matches!(self, Story | Sync),
            _ => matches!(self, Story),
        }
    }
}

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
        let persistence = writer.replace(data.into()).map_err(SyncError::Write)?;
        Ok(SyncReport {
            summary,
            persistence,
        })
    }
}

/// A successful, complete publication; retention depends on the selected store.
#[derive(Debug, PartialEq, serde::Serialize)]
pub struct SyncReport {
    pub summary: Summary,
    pub persistence: Persistence,
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
