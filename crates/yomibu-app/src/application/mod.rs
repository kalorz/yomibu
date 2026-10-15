use std::time::Instant;

use yomibu_core::domain::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};

/// Port: inbound / driving.
///
/// Lists the learner's associated collection IDs through the supplied storage.
/// Rejects an expired deadline before dispatch; otherwise forwards the learner ID
/// and unchanged deadline once and awaits storage. Preserves failures and results,
/// including empty results, except that late success is a deadline error.
/// Storage enforces the I/O deadline. Does not check learner existence.
pub async fn list_collection_ids(
    learner_id: LearnerId,
    storage: &dyn LearnerCollectionQuery,
    deadline: Instant,
) -> Result<Vec<CollectionId>, StorageError> {
    if Instant::now() >= deadline {
        return Err(StorageError("collection lookup deadline exceeded".into()));
    }
    let ids = storage
        .collection_ids_for_learner(learner_id, deadline)
        .await?;
    if Instant::now() >= deadline {
        return Err(StorageError("collection lookup deadline exceeded".into()));
    }
    Ok(ids)
}
