use crate::domain::{CollectionId, LearnerId};

/// An adapter's provider-independent storage failure description.
/// Adapters must supply a presentable message without credentials or raw
/// connection strings.
#[derive(Debug, PartialEq, Eq)]
pub struct StorageError(pub String);

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StorageError {}

/// Port: outbound / driven. Role: infrastructure.
///
/// Read-only lookup of all collection IDs associated with the supplied learner:
/// `learner_collections.collection_id` where `learner_id` matches the input.
/// Ownership alone does not establish an association; unrelated shared or owned
/// collections must not be included.
///
/// Results have no specified order or pagination. No associations, including for
/// an unknown learner, yield an empty vector; learner existence is not checked.
/// Storage failures must return `Err`, never partial or successful empty results.
///
/// Calls are synchronous, with no per-call deadline or cancellation guarantee.
/// Before introducing an I/O adapter, define how it bounds I/O and receives any
/// request-scoped execution limits through composition.
pub trait LearnerCollectionQuery {
    fn collection_ids_for_learner(
        &self,
        learner_id: LearnerId,
    ) -> Result<Vec<CollectionId>, StorageError>;
}
