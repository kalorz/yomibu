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
        return Err(StorageError::collection_lookup_deadline_exceeded());
    }
    let result = storage
        .collection_ids_for_learner(learner_id, deadline)
        .await;
    complete_lookup(result, deadline, Instant::now())
}

fn complete_lookup(
    result: Result<Vec<CollectionId>, StorageError>,
    deadline: Instant,
    observed_at: Instant,
) -> Result<Vec<CollectionId>, StorageError> {
    let ids = result?;
    if observed_at >= deadline {
        return Err(StorageError::collection_lookup_deadline_exceeded());
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::complete_lookup;
    use yomibu_core::ports::infrastructure::StorageError;

    #[test]
    fn completion_rejects_late_success_but_preserves_failure() {
        let deadline = Instant::now();
        let before = deadline - Duration::from_secs(1);
        let after = deadline + Duration::from_secs(1);
        assert_eq!(complete_lookup(Ok(vec![]), deadline, before), Ok(vec![]));
        for observed_at in [deadline, after] {
            assert_eq!(
                complete_lookup(Ok(vec![]), deadline, observed_at),
                Err(StorageError::collection_lookup_deadline_exceeded())
            );
        }
        for observed_at in [before, deadline, after] {
            assert_eq!(
                complete_lookup(
                    Err(StorageError("storage unavailable".into())),
                    deadline,
                    observed_at,
                ),
                Err(StorageError("storage unavailable".into()))
            );
        }
    }
}
