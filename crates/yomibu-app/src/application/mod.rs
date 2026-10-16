use std::time::Instant;

use yomibu_core::domain::{CollectionId, LearnerId, LearnerName};
use yomibu_core::ports::infrastructure::{
    LearnerCollectionQuery, LearnerWriteError, LearnerWriter, StorageError,
};

/// Port: inbound / driving.
///
/// Trims and rejects a blank name through the core rule before invoking storage.
/// Rejects an expired deadline before dispatch, otherwise calls the supplied writer
/// exactly once with the unchanged deadline. The writer bounds I/O and confirms
/// persistence. Preserves failures, including uncertain completion, without retries.
/// A confirmed creation observed after expiry fails with `Created`, retaining its UUID.
pub async fn create_learner(
    name: &str,
    storage: &dyn LearnerWriter,
    deadline: Instant,
) -> Result<LearnerId, LearnerWriteError> {
    let name = LearnerName::new(name)
        .ok_or_else(|| LearnerWriteError::NotCreated("learner name must not be blank".into()))?;
    if Instant::now() >= deadline {
        return Err(LearnerWriteError::NotCreated(
            "creation deadline exceeded".into(),
        ));
    }
    let result = storage.create_learner(name, deadline).await;
    complete_creation(result, deadline, Instant::now())
}

fn complete_creation(
    result: Result<LearnerId, LearnerWriteError>,
    deadline: Instant,
    observed_at: Instant,
) -> Result<LearnerId, LearnerWriteError> {
    let id = result?;
    if observed_at >= deadline {
        return Err(LearnerWriteError::Created {
            id,
            reason: "creation deadline exceeded".into(),
        });
    }
    Ok(id)
}

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

    use super::{complete_creation, complete_lookup};
    use yomibu_core::domain::LearnerId;
    use yomibu_core::ports::infrastructure::{LearnerWriteError, StorageError};

    #[test]
    fn late_creation_retains_confirmed_uuid() {
        let id = LearnerId(uuid::Uuid::from_u128(7));
        let deadline = Instant::now();
        assert_eq!(
            complete_creation(Ok(id), deadline, deadline - Duration::from_secs(1)),
            Ok(id)
        );
        for observed_at in [deadline, deadline + Duration::from_secs(1)] {
            assert_eq!(
                complete_creation(Ok(id), deadline, observed_at),
                Err(LearnerWriteError::Created {
                    id,
                    reason: "creation deadline exceeded".into()
                })
            );
            let error = LearnerWriteError::CompletionUnknown("connection lost".into());
            assert_eq!(
                complete_creation(Err(error), deadline, observed_at),
                Err(LearnerWriteError::CompletionUnknown(
                    "connection lost".into()
                ))
            );
        }
    }

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
