//! Focused contracts for explicitly wired components.
//!
//! Sync preserves source-specific state; story reads normalized learner data.

use crate::domain::source::WaniKaniSyncData;
use std::{error::Error, sync::Arc};

mod story;
pub use story::{CandidateGenerator, StoryPreparer};

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

/// One exclusive synchronization attempt; dropping it releases the writer slot.
pub trait SourceSyncWriter {
    type Error: Error + Send + Sync + 'static;

    /// Validate and atomically publish a complete source version for the same
    /// account. Backend errors must distinguish failure before replacement from
    /// uncertain durability after replacement; an error need not mean rollback.
    /// Success means durability operations, including directory sync, completed.
    fn replace(self, data: Arc<WaniKaniSyncData>) -> Result<(), Self::Error>;
}

/// A physical store of coherent source material and learner progress.
///
/// The writer excludes other writers for the entire fetch-and-replace operation;
/// readers keep the prior version.
/// These synchronous operations suit memory and local files. Future SQL/remote
/// contracts must account for asynchronous I/O rather than block this interface.
pub trait LearningStore {
    type WriteError: Error + Send + Sync + 'static;
    type Writer: SourceSyncWriter<Error = Self::WriteError>;

    /// Reserve a writer without waiting. Files may create a directory and lock
    /// file here. A failure must not replace existing source data.
    fn begin_sync(&self) -> Result<Self::Writer, Self::WriteError>;
}

/// Synchronous candidate transformation. Prepare external evidence before calling.
/// Keep word identity and targets; update scores and reasons together.
pub trait SelectionStep {
    fn apply<'a>(
        &self,
        input: crate::domain::story::SelectionInput<'a>,
        candidates: crate::domain::story::SelectionCandidates<'a>,
    ) -> Result<crate::domain::story::SelectionCandidates<'a>, crate::domain::story::StoryError>;
}
