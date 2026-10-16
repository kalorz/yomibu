use std::{future::Future, pin::Pin, time::Instant};

use crate::domain::{CollectionId, LearnerId, LearnerName};

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

/// Creation knowledge must survive subsequent failures. Reasons must be safe to
/// present, without credentials, connection URLs, or raw provider messages.
#[derive(Debug, PartialEq, Eq)]
pub enum LearnerWriteError {
    /// No learner was created by this invocation.
    NotCreated(String),
    /// The write may have committed. This must never be treated as safe to retry.
    CompletionUnknown(String),
    /// Creation was confirmed, but a later deadline, cleanup, or output step failed.
    Created { id: LearnerId, reason: String },
}

impl std::fmt::Display for LearnerWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotCreated(reason) => write!(f, "learner was not created: {reason}"),
            Self::CompletionUnknown(reason) => write!(
                f,
                "learner creation completion is unknown: {reason}; it may have been saved; do not automatically retry"
            ),
            Self::Created { id, reason } => write!(
                f,
                "learner {} was created, but {reason}; do not repeat creation",
                id.0
            ),
        }
    }
}

impl std::error::Error for LearnerWriteError {}

/// Port: outbound / driven. Role: infrastructure.
///
/// Creates exactly one learner with the validated name and returns its assigned
/// UUID only after storage confirms commit. Repeated names create distinct learners;
/// this operation creates no collections or associations. The adapter owns these
/// persistence guarantees; callers do not query again to validate success.
///
/// Reject expired calls before I/O. Bound acquisition, connection establishment,
/// and execution by the supplied absolute monotonic deadline, without retrying
/// the write. Failures before dispatch are `NotCreated`; failures after dispatch
/// are conservatively `CompletionUnknown` unless creation is confirmed. Preserve
/// a confirmed UUID even if observed after expiry; application handles late results.
///
/// Dropping the future abandons its result, not necessarily the server operation.
/// A timeout or cancellation does not prove rollback. Cleanup may continue beyond
/// the cooperative I/O deadline; composition bounds startup/shutdown separately
/// and configures finite server execution limits.
pub trait LearnerWriter {
    fn create_learner(
        &self,
        name: LearnerName,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<LearnerId, LearnerWriteError>> + '_>>;
}
