use yomibu_core::domain::identity::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};

/// Port: inbound / driving.
///
/// Return the supplied storage's learner-scoped collection IDs or failure unchanged.
/// The learner ID is forwarded without normalization or an existence check.
pub fn list_collection_ids(
    learner_id: LearnerId,
    storage: &impl LearnerCollectionQuery,
) -> Result<Vec<CollectionId>, StorageError> {
    storage.collection_ids_for_learner(learner_id)
}
