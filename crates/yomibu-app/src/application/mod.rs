use std::time::Instant;

use yomibu_core::domain::{CollectionId, LearnerId, LearnerName};
use yomibu_core::ports::infrastructure::{
    LearnerCollectionQuery, LearnerWriteError, LearnerWriter, StorageError,
};

/// A validated creation request, prepared before configuring storage dependencies.
pub struct CreateLearner {
    name: LearnerName,
}

impl CreateLearner {
    /// Trims and rejects blank input through the core rule, without I/O.
    pub fn new(name: &str) -> Result<Self, CreateLearnerError> {
        let name = LearnerName::new(name).ok_or(CreateLearnerError::BlankName)?;
        Ok(Self { name })
    }

    /// Port: inbound / driving.
    ///
    /// Rejects an expired deadline before dispatch, otherwise invokes the supplied
    /// writer exactly once with the unchanged deadline. Storage bounds I/O and
    /// confirms persistence. Preserves failures without retries; a late confirmed
    /// creation fails with `Created`, retaining its UUID.
    pub async fn execute(
        self,
        storage: &dyn LearnerWriter,
        deadline: Instant,
    ) -> Result<LearnerId, CreateLearnerError> {
        if Instant::now() >= deadline {
            return Err(CreateLearnerError::DeadlineExceeded);
        }
        let result = storage.create_learner(self.name, deadline).await;
        complete_creation(result, deadline, Instant::now())
    }
}

/// Application failures distinguish invalid input from storage outcomes and retain
/// confirmed creation through later failures. Reasons must be credential-safe.
#[derive(Debug, PartialEq, Eq)]
pub enum CreateLearnerError {
    BlankName,
    DeadlineExceeded,
    Storage(LearnerWriteError),
    Created { id: LearnerId, reason: &'static str },
}

impl std::fmt::Display for CreateLearnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlankName => f.write_str("learner name must not be blank"),
            Self::DeadlineExceeded => {
                f.write_str("learner was not created: creation deadline exceeded")
            }
            Self::Storage(error) => error.fmt(f),
            Self::Created { id, reason } => write!(
                f,
                "learner {} was created, but {reason}; do not repeat creation",
                id.0
            ),
        }
    }
}

impl std::error::Error for CreateLearnerError {}

fn complete_creation(
    result: Result<LearnerId, LearnerWriteError>,
    deadline: Instant,
    observed_at: Instant,
) -> Result<LearnerId, CreateLearnerError> {
    let id = result.map_err(CreateLearnerError::Storage)?;
    if observed_at >= deadline {
        return Err(CreateLearnerError::Created {
            id,
            reason: "creation deadline exceeded",
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

    use super::{CreateLearnerError, complete_creation, complete_lookup};
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
                Err(CreateLearnerError::Created {
                    id,
                    reason: "creation deadline exceeded"
                })
            );
            let error = LearnerWriteError::CompletionUnknown("connection lost".into());
            assert_eq!(
                complete_creation(Err(error), deadline, observed_at),
                Err(CreateLearnerError::Storage(
                    LearnerWriteError::CompletionUnknown("connection lost".into())
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
