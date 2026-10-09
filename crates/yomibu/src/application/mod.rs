//! Explicit synchronization and offline status for one account-scoped store.

mod assessment;
mod construction;
mod credentials;
pub use credentials::{CredentialError, Credentials, Secret};
mod embeddings;
mod inputs;
mod local;
pub use local::{ApplicationError, LocalApp, ServiceEndpoints, SetupIssue};
pub mod input_file;
pub mod progress;
pub mod selection;
mod source;
pub mod story;

use crate::reports::summary::Summary;
use thiserror::Error;
use yomibu_core::{
    capabilities::{LearningSource, LearningStore, Persistence, SourceSyncWriter},
    domain::source::ValidationError,
};

#[derive(Debug)]
pub enum Operation {
    Story,
    Preview,
    Retrieval,
    Analyze(std::path::PathBuf),
    Import(std::path::PathBuf),
    Verify,
    Sync,
    Status,
}

impl Operation {
    pub(crate) fn uses_credential(
        &self,
        requirement: crate::configuration::components::CredentialRequirement,
    ) -> bool {
        use crate::configuration::components;
        match self {
            Self::Story => true,
            Self::Sync => requirement.name == components::SOURCE_KEY.name,
            Self::Retrieval => requirement.name == components::EMBEDDING_KEY.name,
            _ => false,
        }
    }

    pub(crate) fn uses_setting(&self, name: &str) -> bool {
        use Operation::*;
        match name {
            "pipeline.components.source" => matches!(self, Story | Sync),
            "pipeline.components.learning_store" | "application.wanikani_cache" => {
                matches!(self, Story | Preview | Retrieval | Sync | Status)
            }
            "pipeline.components.analysis" | "application.dictionary_dir" => {
                matches!(self, Story | Analyze(_) | Import(_) | Verify)
            }
            "pipeline.components.assessment" => matches!(self, Story | Analyze(_)),
            "pipeline.components.preparation"
            | "pipeline.components.generation"
            | "pipeline.model"
            | "pipeline.options.openai-story-generation.model"
            | "story.format"
            | "story.candidates"
            | "story.seed"
            | "enable"
            | "disable"
            | "application.sync"
            | "pipeline.selection.embeddings"
            | "pipeline.assessment.enabled"
            | "pipeline.selection.steps"
            | "pipeline.selection.embedding_steps" => matches!(self, Story | Preview),
            "application.cache_max_age_seconds" => matches!(self, Story),
            _ => matches!(self, Story | Preview | Retrieval),
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

impl<Store: LearningStore, Source> App<Store, Source> {
    /// Read and summarize local data without network access or writes.
    /// The returned summary owns its data and can outlive the application.
    pub fn status(&self) -> Result<Summary, StatusError<Store::ReadError>> {
        crate::reports::summary::summarize(self.store.load().map_err(StatusError::Read)?.as_ref())
            .map_err(StatusError::InvalidData)
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
        let persistence = writer.replace(data).map_err(SyncError::Write)?;
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
pub enum StatusError<ReadError> {
    #[error(transparent)]
    Read(ReadError),
    #[error("Invalid stored source data: {0}")]
    InvalidData(#[source] ValidationError),
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
