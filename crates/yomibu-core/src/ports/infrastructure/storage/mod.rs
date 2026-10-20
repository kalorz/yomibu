//! Storage contracts for learners, collections, subjects and learner progress.

use crate::domain::{CollectionId, LearnerId, LearnerName, LearnerProgressSourceId, MaterialKind};
use chrono::{DateTime, Utc};
use serde_json::value::RawValue;
use std::{collections::HashSet, future::Future, pin::Pin, time::Instant};

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

/// Per-group record limit, independently for subjects and learner progress.
pub const MAX_GROUP_RECORDS: usize = 1_000;
/// Maximum combined raw UTF-8 bytes in one lookup or save-group request.
/// Lookups count provider, key and IDs; saves also count names, stages and payloads.
pub const MAX_REQUEST_INPUT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum raw JSON byte length of one source payload.
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// Maximum number of supplied IDs in one lookup (before deduplication).
pub const MAX_SUBJECT_LOOKUP_IDS: usize = 1_000;

/// Port: outbound / driven. Role: infrastructure.
///
/// Returns unique requested external subject IDs, sorted lexicographically, from the
/// supplied shared collection (provider/key). An absent collection means no matches;
/// this query never creates anything. Enforce [`validate_subject_lookup`] before I/O;
/// an empty valid request returns immediately. Results are advisory:
/// a subsequent save must independently enforce its reference guarantees.
///
/// One absolute monotonic deadline covers the entire lookup, including resource
/// acquisition and waits. Reject expired calls and late read success. Dropping the
/// future abandons delivery; a timeout does not promise immediate cancellation of
/// underlying work. Cleanup is separately bounded so abandoned work cannot
/// indefinitely retain shared resources.
/// Failures are sanitized errors, never partial or successful empty results.
pub trait SubjectQuery {
    fn existing_subject_ids<'a>(
        &'a self,
        provider: &'a str,
        key: &'a str,
        ids: &'a [String],
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, StorageError>> + 'a>>;
}

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

/// Storage-contract resource validation for a lookup, before any adapter I/O.
/// IDs are opaque and may repeat; no provider ID parsing or normalization occurs.
pub fn validate_subject_lookup(
    provider: &str,
    key: &str,
    ids: &[String],
) -> Result<(), StorageError> {
    let size = [provider.len(), key.len()]
        .into_iter()
        .chain(ids.iter().map(String::len))
        .try_fold(0usize, usize::checked_add);
    if ids.len() > MAX_SUBJECT_LOOKUP_IDS || size.is_none_or(|n| n > MAX_REQUEST_INPUT_BYTES) {
        return Err(StorageError("subject lookup exceeds input limits".into()));
    }
    Ok(())
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

/// One finite storage request, not an API page or proof of complete traversal.
/// Construction validates duplicate identities and storage-input limits without I/O.
/// External IDs are exact opaque text; this contract performs no normalization.
/// The caller bounds upstream preparation; the adapter separately enforces its
/// representation and resource limits. References may resolve to stored subjects.
#[derive(Debug, Clone)]
pub struct PreparedSaveGroup {
    collection: SharedCollection,
    learner_progress_source: LearnerProgressSourceIdentity,
    account: SourceRecord,
    subjects: Vec<PreparedSubject>,
    progress: Vec<PreparedProgress>,
}
impl PreparedSaveGroup {
    pub fn new(
        collection: SharedCollection,
        learner_progress_source: LearnerProgressSourceIdentity,
        account: SourceRecord,
        subjects: Vec<PreparedSubject>,
        progress: Vec<PreparedProgress>,
    ) -> Result<Self, InvalidSaveGroup> {
        if subjects.len() > MAX_GROUP_RECORDS || progress.len() > MAX_GROUP_RECORDS {
            return Err(InvalidSaveGroup("save group exceeds record limit"));
        }
        let mut subject_ids = HashSet::new();
        let mut progress_ids = HashSet::new();
        if subjects.iter().any(|s| !subject_ids.insert(&s.external_id))
            || progress
                .iter()
                .any(|p| !progress_ids.insert(&p.subject_external_id))
        {
            return Err(InvalidSaveGroup("duplicate subject/progress identity"));
        }
        let group = Self {
            collection,
            learner_progress_source,
            account,
            subjects,
            progress,
        };
        let size = group
            .input_sizes()
            .try_fold(0usize, |total, size| total.checked_add(size));
        if size.is_none_or(|n| n > MAX_REQUEST_INPUT_BYTES) {
            return Err(InvalidSaveGroup("save group exceeds byte limit"));
        }
        Ok(group)
    }
    fn input_sizes(&self) -> impl Iterator<Item = usize> + '_ {
        [
            self.collection.input_bytes(),
            self.learner_progress_source.external_account_id.len(),
            self.account.payload.get().len(),
        ]
        .into_iter()
        .chain(
            self.subjects
                .iter()
                .flat_map(|s| [s.external_id.len(), s.source.payload.get().len()]),
        )
        .chain(self.progress.iter().flat_map(|p| {
            [
                p.subject_external_id.len(),
                p.source.payload.get().len(),
                p.progress_stage.as_ref().map_or(0, String::len),
            ]
        }))
    }
    pub fn collection(&self) -> &SharedCollection {
        &self.collection
    }
    pub fn learner_progress_source(&self) -> &LearnerProgressSourceIdentity {
        &self.learner_progress_source
    }
    pub fn account(&self) -> &SourceRecord {
        &self.account
    }
    pub fn subjects(&self) -> &[PreparedSubject] {
        &self.subjects
    }
    pub fn progress(&self) -> &[PreparedProgress] {
        &self.progress
    }
}

/// Stable identities from one confirmed atomic save; no traversal/completion claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveReceipt {
    pub collection_id: CollectionId,
    pub learner_progress_source_id: LearnerProgressSourceId,
}

/// The stored completion timestamp after a confirmed explicit completion write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionReceipt {
    pub learner_progress_source_id: LearnerProgressSourceId,
    pub last_import_completed_at: DateTime<Utc>,
}

/// Knowledge of this atomic save's outcome, not the outcome of earlier groups.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportWriteError {
    /// This save made no durable changes. Earlier confirmed groups survive.
    #[error("transaction not saved: {0}")]
    NotSaved(String),
    /// Persistence was not confirmed; the supplied data may or may not have been saved.
    #[error("transaction completion unknown: {0}")]
    CompletionUnknown(String),
}

/// Port: outbound / driven. Role: infrastructure.
///
/// Atomically saves the supplied shared collection, external progress source and
/// finite prepared records for an existing learner. Shared subjects are identified by collection/external ID; progress references must
/// resolve to supplied or existing subjects in that collection. Never fabricates
/// learners/subjects, publishes entries, deletes omitted records or records sync
/// completion. Empty progress is valid. Account sources remain independent.
///
/// With known stored freshness, only strictly newer upstream timestamps replace
/// each record's source fields and full payload; equal/older payloads do not.
/// Progress observation time advances independently and monotonically. Newer
/// explicit null dates replace old dates.
/// Existing identities remain stable. For subjects with unavailable stored freshness,
/// supplied timestamped content initializes the source fields, full payload and
/// upstream timestamp. Subsequent replacement requires a strictly newer timestamp.
/// Malformed stored freshness metadata fails safely.
///
/// One absolute deadline covers the entire save, including preparation, resource
/// acquisition, waits and confirmation. No automatic retry. Receipts require
/// confirmed persistence, including confirmation observed late. Failed or
/// interrupted finalization conservatively reports unknown completion. Dropping
/// the future may leave an unknown save outcome; the caller must account for this.
/// Cleanup is separately bounded so abandoned work cannot indefinitely retain
/// shared resources. Failures contain no source diagnostics.
/// Adapters bound internal resource use independently of the raw-input limits.
/// Unsupported representations fail without partial saves or silent data loss.
pub trait LearnerProgressImportWriter {
    fn save_group<'a>(
        &'a self,
        group: &'a PreparedSaveGroup,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<SaveReceipt, ImportWriteError>> + 'a>>;

    /// Explicit successful-sync bookkeeping. The caller has established complete
    /// traversal; source existence or previous group receipts do not prove it.
    /// Monotonically advances only `last_import_completed_at` for an existing
    /// external source in the supplied shared collection, returning the stored
    /// maximum. Account-only syncs are valid. Unknown/wrong-collection sources
    /// fail without insertion. Does not change payloads or ordinary group rows.
    /// Uses the same atomic outcome/deadline/drop guarantees as `save_group`.
    fn record_import_completion(
        &self,
        collection: CollectionId,
        source: LearnerProgressSourceId,
        completed_at: DateTime<Utc>,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<CompletionReceipt, ImportWriteError>> + '_>>;
}
