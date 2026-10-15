use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::domain::identity::{CollectionId, LearnerId};

/// A storage failure with a message safe to expose to callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError {
    /// Implementations must omit credentials and private backend diagnostics.
    pub message: String,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for StorageError {}

/// Port: outbound / driven. Role: infrastructure.
pub trait LearnerCollectionQuery {
    /// Return all collection IDs associated with `learner_id` through
    /// `learner_collections`, each once, in unspecified order.
    /// Ownership alone or a collection being shared does not establish association.
    ///
    /// Returns an empty vector when there are no associations, including for an
    /// unknown learner; this query does not establish learner existence or access
    /// permission. This read has no result limit, does not change associations,
    /// and does not synchronize concurrent updates.
    ///
    /// Storage failures must return `Err` with a safe message, never empty success
    /// or partial results. The absolute monotonic `deadline` bounds the entire
    /// call. Implementations must bound backend I/O by the remaining time and
    /// honor `cancelled`, including while I/O is in progress, returning `Err`
    /// on cancellation or expiry. Callers may set the flag but must not reset it
    /// during the operation. No retries are requested.
    ///
    /// The call is synchronous: the application cannot forcibly interrupt an
    /// implementation that violates these requirements.
    fn collection_ids_for_learner(
        &self,
        learner_id: LearnerId,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<CollectionId>, StorageError>;
}
