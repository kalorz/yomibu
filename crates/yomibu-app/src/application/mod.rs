use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use yomibu_core::domain::identity::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};

/// Port: inbound / driving.
///
/// Return valid learner-scoped collection IDs or the storage failure unchanged.
/// The learner ID is forwarded without normalization or an existence check.
/// Duplicate collection IDs are rejected as a storage contract violation.
/// The supplied deadline and cancellation flag are forwarded unchanged. Calls
/// already expired or cancelled are rejected before dispatch; successful results
/// are accepted only while the operation is still active. Storage failures take
/// precedence over cancellation or expiry observed after dispatch.
/// Storage must honor the execution limits during its synchronous call; this
/// function cannot forcibly interrupt a noncompliant implementation.
pub fn list_collection_ids(
    learner_id: LearnerId,
    storage: &(impl LearnerCollectionQuery + ?Sized),
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<CollectionId>, StorageError> {
    check_execution_limits(deadline, cancelled)?;
    let collection_ids = storage.collection_ids_for_learner(learner_id, deadline, cancelled)?;
    let mut seen = HashSet::new();
    if collection_ids.iter().any(|id| !seen.insert(id.0)) {
        return Err(StorageError {
            message: "Storage returned duplicate collection IDs".into(),
        });
    }
    check_execution_limits(deadline, cancelled)?;
    Ok(collection_ids)
}

fn check_execution_limits(deadline: Instant, cancelled: &AtomicBool) -> Result<(), StorageError> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(StorageError {
            message: "Collection query was cancelled".into(),
        });
    }
    if Instant::now() >= deadline {
        return Err(StorageError {
            message: "Collection query deadline expired".into(),
        });
    }
    Ok(())
}
