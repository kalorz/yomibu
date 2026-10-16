use std::{future::Future, pin::Pin, time::Instant};

use crate::domain::{CollectionId, LearnerId};

/// A storage port's provider-independent failure description.
/// Adapters must supply a presentable message without credentials or raw
/// connection strings.
#[derive(Debug, PartialEq, Eq)]
pub struct StorageError(pub String);

impl StorageError {
    /// The shared failure for an expired collection lookup deadline.
    pub fn collection_lookup_deadline_exceeded() -> Self {
        Self("collection lookup deadline exceeded".into())
    }
}

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
/// Every associated collection ID is returned exactly once, in unspecified order,
/// with no pagination. Uniqueness is an adapter obligation, verified by each
/// adapter's contract tests. No associations, including for an unknown learner,
/// yield an empty vector; learner existence is not checked.
/// Storage failures must return `Err`, never partial or successful empty results.
///
/// Application supplies an absolute monotonic deadline, shared with any parent
/// operation. Adapters must reject expired calls before I/O, bound all request
/// waits (including connection acquisition and establishment) by that deadline,
/// and reject success observed after expiry. Deadlines are cooperative I/O bounds,
/// not hard real-time CPU or memory limits.
/// Dropping the future abandons the result, without guaranteeing immediate server
/// cancellation. Provider cleanup may continue outside the request deadline.
/// Composition separately bounds startup/shutdown and configures finite backend
/// execution limits. No retry or separate cancellation-token contract is provided.
pub trait LearnerCollectionQuery {
    fn collection_ids_for_learner(
        &self,
        learner_id: LearnerId,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<CollectionId>, StorageError>> + '_>>;
}
