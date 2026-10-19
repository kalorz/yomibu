//! Finite shared-collection imports for existing learners and external progress sources.
//! No source protocol, payload interpretation or traversal ownership.
use super::StorageError;
use crate::domain::{CollectionId, LearnerId, LearnerProgressSourceId, MaterialKind};
use chrono::{DateTime, Utc};
use serde_json::value::RawValue;
use std::{collections::HashSet, future::Future, pin::Pin, time::Instant};

/// Per-group record limit, independently for subjects and learner progress.
pub const MAX_GROUP_RECORDS: usize = 1_000;
/// Maximum combined raw UTF-8 bytes in one lookup or save-group request.
/// Includes supplied identities, collection name, stages and payloads.
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
/// a subsequent save must resolve references under its own database constraints.
///
/// The absolute monotonic deadline covers acquisition, connection setup, locks
/// and statements. Reject expired calls and late read success. Dropping the
/// future abandons delivery; close interrupted connections rather than returning
/// them to a pool while commands remain pending. Cleanup is bounded separately
/// by the adapter; a timeout does not promise immediate server cancellation.
/// Failures are sanitized errors, never partial or successful empty results.
pub trait SubjectQuery {
    fn existing_subject_ids<'a>(
        &'a self,
        collection: &'a SharedCollection,
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
    collection: &SharedCollection,
    ids: &[String],
) -> Result<(), StorageError> {
    let size = ids
        .iter()
        .try_fold(collection.input_bytes(), |n, id| n.checked_add(id.len()));
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
/// The caller bounds upstream preparation; the adapter separately validates its
/// encoding and database constraints. References may resolve to stored subjects.
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

/// Knowledge of this transaction's outcome, not the outcome of earlier groups.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportWriteError {
    /// The transaction cannot have committed. Earlier confirmed groups survive.
    #[error("transaction not saved: {0}")]
    NotSaved(String),
    /// COMMIT failed or was interrupted; even a rejection may be reported conservatively.
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
/// One absolute deadline covers preparation, acquisition, locks, statements and
/// commit. No automatic retry. Receipts require confirmed COMMIT, including a
/// confirmation observed late. Commit errors/timeouts conservatively report unknown
/// completion, never inferred rollback. Dropping the future may abandon an
/// unknown commit outcome; the caller must account for this. Leases without a
/// confirmed clean transaction boundary are discarded with bounded cleanup.
/// Failures contain no source diagnostics.
/// Before I/O, the adapter validates encoded-resource bounds and lossless
/// timestamp representation independently of the request's raw-input limits.
/// Database-dependent representation checks can reject the transaction atomically
/// after earlier statements have run.
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    fn collection() -> SharedCollection {
        SharedCollection {
            provider: "source".into(),
            key: "lessons".into(),
            name: "Lessons".into(),
        }
    }
    fn time() -> DateTime<Utc> {
        Utc.timestamp_opt(1_700_000_000, 1).unwrap()
    }
    fn source() -> SourceRecord {
        SourceRecord::new(time(), r#"{"id":1,"nested":{"all":[1,2]}}"#).unwrap()
    }
    fn progress(id: u64) -> PreparedProgress {
        PreparedProgress {
            subject_external_id: id.to_string(),
            progress_stage: Some("5".into()),
            is_hidden: false,
            learning_started_at: None,
            first_passed_at: None,
            observed_at: time(),
            source: source(),
        }
    }
    fn group(
        subjects: Vec<PreparedSubject>,
        progress: Vec<PreparedProgress>,
    ) -> Result<PreparedSaveGroup, InvalidSaveGroup> {
        PreparedSaveGroup::new(
            collection(),
            LearnerProgressSourceIdentity {
                learner_id: LearnerId(uuid::Uuid::from_u128(1)),
                external_account_id: "account-a".into(),
            },
            source(),
            subjects,
            progress,
        )
    }
    #[test]
    fn group_accepts_account_only_existing_references_and_both_kinds() {
        assert!(group(vec![], vec![]).is_ok());
        assert!(group(vec![], vec![progress(99)]).is_ok());
        let grammar = PreparedSubject {
            external_id: "lesson:alpha/7".into(),
            kind: Some(MaterialKind::Grammar),
            source: source(),
        };
        let input = group(vec![grammar], vec![]).unwrap();
        assert_eq!(input.subjects()[0].external_id, "lesson:alpha/7"); // No normalization.
        assert_eq!(source().updated_at(), time()); // Nanoseconds are not a contract rejection.
    }
    #[test]
    fn group_rejects_duplicate_identities() {
        assert!(group(vec![], vec![progress(1), progress(1)]).is_err());
        let subject = || PreparedSubject {
            external_id: "1".into(),
            kind: Some(MaterialKind::Vocabulary),
            source: source(),
        };
        assert!(group(vec![subject(), subject()], vec![]).is_err());
    }
    #[test]
    fn lookup_limits_include_identity_bytes() {
        assert!(
            validate_subject_lookup(&collection(), &["lesson:alpha/7".into(), "0".into()]).is_ok()
        );
        assert!(
            validate_subject_lookup(&collection(), &vec!["x".into(); MAX_SUBJECT_LOOKUP_IDS + 1])
                .is_err()
        );
        assert!(
            validate_subject_lookup(&collection(), &["x".repeat(MAX_REQUEST_INPUT_BYTES)]).is_err()
        );
        let mut oversized = collection();
        oversized.key = "x".repeat(MAX_REQUEST_INPUT_BYTES);
        assert!(validate_subject_lookup(&oversized, &[]).is_err());
    }
    #[test]
    fn source_record_validates_opaque_json_and_raw_bytes() {
        assert!(SourceRecord::new(time(), "{bad}").is_err());
        for raw in ["null", "[]", "123"] {
            assert!(SourceRecord::new(time(), raw).is_ok());
        }
        let payload = format!("{{\"x\":\"{}\"}}", "a".repeat(MAX_SOURCE_BYTES - 8));
        assert_eq!(payload.len(), MAX_SOURCE_BYTES);
        assert!(SourceRecord::new(time(), &payload).is_ok());
        assert!(SourceRecord::new(time(), &(payload + " ")).is_err());
        assert!(SourceRecord::new(time(), r#"{"x":"\u0000"}"#).is_ok()); // JSONB rejection belongs to adapter.
    }
    #[test]
    fn group_limits_count_utf8_bytes_and_reject_oversized_input() {
        assert!(
            group(
                vec![],
                (1..=MAX_GROUP_RECORDS as u64).map(progress).collect()
            )
            .is_ok()
        );
        assert!(
            group(
                vec![],
                (1..=MAX_GROUP_RECORDS as u64 + 1).map(progress).collect()
            )
            .is_err()
        );
        let mut p = progress(1);
        let fixed = collection().input_bytes()
            + "account-a".len()
            + p.subject_external_id.len()
            + source().payload().get().len()
            + p.source.payload().get().len();
        p.progress_stage = Some("あ".repeat((MAX_REQUEST_INPUT_BYTES - fixed) / 3));
        let remaining = MAX_REQUEST_INPUT_BYTES - fixed - p.progress_stage.as_ref().unwrap().len();
        p.progress_stage
            .as_mut()
            .unwrap()
            .push_str(&"x".repeat(remaining));
        assert!(group(vec![], vec![p.clone()]).is_ok());
        p.progress_stage.as_mut().unwrap().push('x');
        assert!(group(vec![], vec![p]).is_err());
    }

    #[test]
    fn group_limits_include_every_variable_identity_field() {
        for field in 0..6 {
            let mut collection = collection();
            let mut identity = LearnerProgressSourceIdentity {
                learner_id: LearnerId(uuid::Uuid::nil()),
                external_account_id: "a".into(),
            };
            let mut subject = PreparedSubject {
                external_id: "s".into(),
                kind: None,
                source: source(),
            };
            let mut progress = progress(1);
            let large = "x".repeat(MAX_REQUEST_INPUT_BYTES);
            match field {
                0 => collection.provider = large,
                1 => collection.key = large,
                2 => collection.name = large,
                3 => identity.external_account_id = large,
                4 => subject.external_id = large,
                _ => progress.subject_external_id = large,
            }
            let error = PreparedSaveGroup::new(
                collection,
                identity,
                source(),
                vec![subject],
                vec![progress],
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("byte limit"),
                "field {field}: {error}"
            );
        }
    }
}
