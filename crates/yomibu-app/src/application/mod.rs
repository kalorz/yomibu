use yomibu_core::domain::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};

/// Port: inbound / driving.
///
/// Lists the learner's associated collection IDs through the supplied storage.
/// Forwards the learner ID once and returns the storage result unchanged,
/// including empty results and failures. Does not check learner existence.
pub fn list_collection_ids(
    learner_id: LearnerId,
    storage: &dyn LearnerCollectionQuery,
) -> Result<Vec<CollectionId>, StorageError> {
    storage.collection_ids_for_learner(learner_id)
}
