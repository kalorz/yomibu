//! Storage contracts for learners, collections, subjects and learner progress.

use crate::domain::{CollectionId, LearnerId, LearnerName, MaterialKind};
use chrono::{DateTime, Utc};
use serde_json::value::RawValue;
use std::{future::Future, pin::Pin, time::Instant};

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
/// Read-only lookup of the supplied learner's collection IDs.
/// Collection membership is derived from `learner_progress_sources.collection_id`
/// where `learner_id` matches the input, even when a source has no learner progress yet.
/// Ownership alone does not establish membership; unrelated shared or owned
/// collections must not be included.
///
/// Every collection ID is returned exactly once, in unspecified order,
/// with no pagination. Uniqueness is an adapter obligation, verified by each
/// adapter's contract tests. An unknown learner or one with no progress sources
/// yields an empty vector; learner existence is not checked.
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

/// Storage's knowledge of a failed write. Reasons must be safe to present,
/// without credentials, connection URLs, or raw provider messages.
#[derive(Debug, PartialEq, Eq)]
pub enum LearnerWriteError {
    /// No learner was created by this invocation.
    NotCreated(String),
    /// The write may have committed. This must never be treated as safe to retry.
    CompletionUnknown(String),
}

impl std::fmt::Display for LearnerWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotCreated(reason) => write!(f, "learner was not created: {reason}"),
            Self::CompletionUnknown(reason) => write!(
                f,
                "learner creation completion is unknown: {reason}; it may have been saved; do not automatically retry"
            ),
        }
    }
}

impl std::error::Error for LearnerWriteError {}

/// Port: outbound / driven. Role: infrastructure.
///
/// Creates exactly one learner with the validated name and returns its assigned
/// UUID only after storage confirms commit. Repeated names create distinct learners;
/// this operation creates no collections or learner progress sources. The adapter
/// owns these persistence guarantees; callers do not query again to validate success.
///
/// Reject expired calls before I/O. Bound acquisition, connection establishment,
/// and execution by the supplied absolute monotonic deadline, without retrying
/// the write. Failures before dispatch and proven server rejections are
/// `NotCreated`; ambiguous failures after dispatch are `CompletionUnknown`. Preserve
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

/// Maximum raw JSON byte length of one source payload.
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// Shared collection to create or reuse, identified by opaque provider/key values.
/// The name is used only on creation; this operation does not refresh collection metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedCollection {
    pub provider: String,
    pub key: String,
    pub name: String,
}
impl SharedCollection {
    /// Raw UTF-8 bytes counted toward the request input limits.
    pub fn input_bytes(&self) -> usize {
        self.provider.len() + self.key.len() + self.name.len()
    }
}

/// External progress-source identity within the group's supplied collection.
/// The existing learner and opaque external account ID distinguish independent sources.
/// Manual sources without an external account are outside this import operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnerProgressSourceIdentity {
    pub learner_id: LearnerId,
    pub external_account_id: String,
}

/// A rejected storage request. Messages never contain supplied payloads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct InvalidSaveGroup(&'static str);

/// Opaque source content and its own upstream update time, supplied after
/// translation. This storage-contract value does not prescribe a database JSON
/// envelope or timestamp representation. The payload must not contain credentials.
#[derive(Debug, Clone)]
pub struct SourceRecord {
    updated_at: DateTime<Utc>,
    payload: Box<RawValue>,
}
impl SourceRecord {
    pub fn new(updated_at: DateTime<Utc>, raw_json: &str) -> Result<Self, InvalidSaveGroup> {
        if raw_json.len() > MAX_SOURCE_BYTES {
            return Err(InvalidSaveGroup("source payload exceeds byte limit"));
        }
        let payload: Box<RawValue> = serde_json::from_str(raw_json)
            .map_err(|_| InvalidSaveGroup("source payload must be valid JSON"))?;
        Ok(Self {
            updated_at,
            payload,
        })
    }
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
    pub fn payload(&self) -> &RawValue {
        &self.payload
    }
}

/// A supplied subject with an opaque collection-local external ID.
/// Both material kinds and unresolved/out-of-model kinds (`None`) are supported.
#[derive(Debug, Clone)]
pub struct PreparedSubject {
    pub external_id: String,
    pub kind: Option<MaterialKind>,
    pub source: SourceRecord,
}

/// Supplied learner progress, including its observation time independently of
/// source freshness. Provider writes do not accept a manual mastery declaration.
#[derive(Debug, Clone)]
pub struct PreparedProgress {
    pub subject_external_id: String,
    pub progress_stage: Option<String>,
    pub is_hidden: bool,
    pub learning_started_at: Option<DateTime<Utc>>,
    pub first_passed_at: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
    pub source: SourceRecord,
}
