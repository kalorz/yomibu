//! Focused contracts for explicitly wired components.
//!
//! The stored unit includes related source material and learner progress. It is
//! currently WaniKani-specific; this is not a general learner or material catalog.

use crate::domain::source::WaniKaniSyncData;
use std::{error::Error, sync::Arc};

pub mod options;
mod story;
pub use story::{CandidateGenerator, SentenceAnalyzer, StoryAssessor, StoryPreparer};

/// Retrieves and normalizes one complete WaniKani account refresh.
///
/// Retrieval may perform network I/O; it must not persist data. This current
/// slice preserves provider-specific state rather than promising a generic
/// provider ontology. Implementations document deadlines and retry behavior.
pub trait LearningSource {
    type Error: Error + Send + Sync + 'static;

    fn fetch(&mut self) -> impl Future<Output = Result<WaniKaniSyncData, Self::Error>> + Send;
}

impl<Source: LearningSource + ?Sized> LearningSource for &mut Source {
    type Error = Source::Error;

    fn fetch(&mut self) -> impl Future<Output = Result<WaniKaniSyncData, Self::Error>> + Send {
        (**self).fetch()
    }
}

/// What a successful replacement promises about the new version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Persistence {
    /// Retained only while the in-memory store has live handles.
    Volatile,
    /// The backend completed its durability operations, including directory sync.
    Durable,
}

/// One exclusive synchronization attempt; dropping it releases the writer slot.
pub trait SourceSyncWriter {
    type Error: Error + Send + Sync + 'static;

    /// Validate and atomically publish a complete source version for the same
    /// account. Backend errors must distinguish failure before replacement from
    /// uncertain durability after replacement; an error need not mean rollback.
    fn replace(self, data: WaniKaniSyncData) -> Result<Persistence, Self::Error>;
}

/// A physical store of coherent source material and learner progress.
///
/// Reads never synchronize or mutate data. The writer excludes other writers
/// for the entire fetch-and-replace operation; readers keep the prior version.
/// These synchronous operations suit memory and local files. Future SQL/remote
/// contracts must account for asynchronous I/O rather than block this interface.
pub trait LearningStore {
    type ReadError: Error + Send + Sync + 'static;
    type WriteError: Error + Send + Sync + 'static;
    type Writer: SourceSyncWriter<Error = Self::WriteError>;

    /// Read one immutable, validated version, independent of later replacements.
    fn load(&self) -> Result<Arc<WaniKaniSyncData>, Self::ReadError>;

    /// Reserve a writer without waiting. Files may create a directory and lock
    /// file here. A failure must not replace existing source data.
    fn begin_sync(&self) -> Result<Self::Writer, Self::WriteError>;
}

/// Encodes explicit lexical documents/briefs; adapters own no process environment
/// or runtime. Purpose distinguishes asymmetric document/query encoders.
pub trait Embedder {
    fn model_identity(&self) -> &crate::domain::embedding::EmbeddingModelIdentity;
    fn embed(
        &self,
        inputs: &[crate::domain::embedding::EmbeddingInput],
    ) -> impl Future<Output = Result<Vec<Vec<f32>>, crate::domain::embedding::EmbeddingError>> + Send;
}

/// Synchronous candidate transformation. Prepare external evidence before calling.
/// Keep word identity and targets; update scores, reasons and embedding provenance together.
pub trait SelectionStep {
    fn apply<'a>(
        &self,
        input: crate::domain::story::SelectionInput<'a>,
        candidates: crate::domain::story::SelectionCandidates<'a>,
    ) -> Result<crate::domain::story::SelectionCandidates<'a>, crate::domain::story::StoryError>;
}
