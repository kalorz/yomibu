//! Storage contracts for supplied WaniKani data; no HTTP or traversal ownership.
use super::StorageError;
use std::{future::Future, pin::Pin, time::Instant};

/// Maximum number of supplied IDs in one lookup (before deduplication).
pub const MAX_SUBJECT_LOOKUP_IDS: usize = 1_000;

/// Port: outbound / driven. Role: infrastructure.
///
/// Returns unique requested external subject IDs, sorted numerically, from the
/// shared WaniKani collection. An absent collection means no matches; this query
/// never creates anything. Reject zero IDs or more than [`MAX_SUBJECT_LOOKUP_IDS`]
/// before I/O; an empty valid request returns immediately. Results are advisory:
/// a subsequent save must resolve references under its own database constraints.
///
/// The absolute monotonic deadline covers acquisition, connection setup, locks
/// and statements. Reject expired calls and late read success. Dropping the
/// future abandons delivery; close interrupted connections rather than returning
/// them to a pool while commands remain pending. Cleanup is bounded separately
/// by the adapter; a timeout does not promise immediate server cancellation.
/// Failures are sanitized errors, never partial or successful empty results.
pub trait WaniKaniSubjectQuery {
    fn existing_subject_ids<'a>(
        &'a self,
        ids: &'a [u64],
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u64>, StorageError>> + 'a>>;
}
use crate::domain::{LearnerId, MaterialKind};
use chrono::{DateTime, Utc};
use serde_json::value::RawValue;
use std::collections::HashSet;

/// Per-group record limit, independently for subjects and learner progress.
pub const MAX_GROUP_RECORDS: usize = 1_000;
/// Combined raw UTF-8 input bytes: account ID, stages and all source payloads.
pub const MAX_GROUP_INPUT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum raw JSON byte length of one source payload.
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// A rejected storage request. Messages never contain supplied payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidSaveGroup(&'static str);
impl std::fmt::Display for InvalidSaveGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for InvalidSaveGroup {}

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
            .map_err(|_| InvalidSaveGroup("source payload must be a JSON object"))?;
        if !payload.get().starts_with('{') {
            return Err(InvalidSaveGroup("source payload must be a JSON object"));
        }
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

/// A translated supplied subject. Vocabulary/kana vocabulary use vocabulary;
/// kanji/radicals use no material kind and keep their provider type in source data.
#[derive(Debug, Clone)]
pub struct PreparedSubject {
    pub external_id: u64,
    pub kind: Option<MaterialKind>,
    pub source: SourceRecord,
}

/// Supplied learner progress, including its observation time independently of
/// source freshness. Provider writes do not accept a manual mastery declaration.
#[derive(Debug, Clone)]
pub struct PreparedProgress {
    pub subject_external_id: u64,
    pub progress_stage: Option<String>,
    pub is_hidden: bool,
    pub learning_started_at: Option<DateTime<Utc>>,
    pub first_passed_at: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
    pub source: SourceRecord,
}

/// One finite storage request, not an API page or proof of complete traversal.
/// Construction validates storage-input limits/identity shape without I/O.
/// The caller bounds upstream preparation; the adapter separately validates its
/// encoding and database constraints. References may resolve to stored subjects.
#[derive(Debug, Clone)]
pub struct PreparedSaveGroup {
    learner_id: LearnerId,
    external_account_id: String,
    account: SourceRecord,
    subjects: Vec<PreparedSubject>,
    progress: Vec<PreparedProgress>,
}
impl PreparedSaveGroup {
    pub fn new(
        learner_id: LearnerId,
        external_account_id: String,
        account: SourceRecord,
        subjects: Vec<PreparedSubject>,
        progress: Vec<PreparedProgress>,
    ) -> Result<Self, InvalidSaveGroup> {
        if external_account_id.is_empty() {
            return Err(InvalidSaveGroup("account ID must not be empty"));
        }
        if subjects.len() > MAX_GROUP_RECORDS || progress.len() > MAX_GROUP_RECORDS {
            return Err(InvalidSaveGroup("save group exceeds record limit"));
        }
        let mut subject_ids = HashSet::new();
        let mut progress_ids = HashSet::new();
        if subjects.iter().any(|s| {
            s.external_id == 0
                || s.kind == Some(MaterialKind::Grammar)
                || !subject_ids.insert(s.external_id)
        }) || progress
            .iter()
            .any(|p| p.subject_external_id == 0 || !progress_ids.insert(p.subject_external_id))
        {
            return Err(InvalidSaveGroup(
                "invalid or duplicate subject/progress identity or kind",
            ));
        }
        let mut sizes = [external_account_id.len(), account.payload.get().len()]
            .into_iter()
            .chain(subjects.iter().map(|s| s.source.payload.get().len()))
            .chain(progress.iter().flat_map(|p| {
                [
                    p.source.payload.get().len(),
                    p.progress_stage.as_ref().map_or(0, String::len),
                ]
            }));
        let size = sizes
            .try_fold(0usize, |total, size| total.checked_add(size))
            .ok_or(InvalidSaveGroup("save group exceeds byte limit"))?;
        if size > MAX_GROUP_INPUT_BYTES {
            return Err(InvalidSaveGroup("save group exceeds byte limit"));
        }
        Ok(Self {
            learner_id,
            external_account_id,
            account,
            subjects,
            progress,
        })
    }
    pub fn learner_id(&self) -> LearnerId {
        self.learner_id
    }
    pub fn external_account_id(&self) -> &str {
        &self.external_account_id
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
    pub collection_id: crate::domain::CollectionId,
    pub learner_progress_source_id: crate::domain::LearnerProgressSourceId,
}

/// Knowledge of this transaction's outcome, not the outcome of earlier groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportWriteError {
    /// The transaction cannot have committed. Earlier confirmed groups survive.
    NotSaved(String),
    /// COMMIT may have reached storage; the caller must retain uncertainty.
    CompletionUnknown(String),
}
impl std::fmt::Display for ImportWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSaved(reason) => write!(f, "transaction not saved: {reason}"),
            Self::CompletionUnknown(reason) => {
                write!(f, "transaction completion unknown: {reason}")
            }
        }
    }
}
impl std::error::Error for ImportWriteError {}

/// Port: outbound / driven. Role: infrastructure.
///
/// Atomically saves one finite, prepared group for an existing learner. Shared
/// subjects are identified by collection/external ID; progress references must
/// resolve to supplied or existing subjects in that collection. Never fabricates
/// learners/subjects, publishes entries, deletes omitted records or records sync
/// completion. Empty progress is valid. Account sources remain independent.
///
/// Strictly newer upstream timestamps replace each record's source fields and
/// full payload; equal/older payloads do not. Progress observation time advances
/// independently and monotonically. Newer explicit null dates replace old dates.
/// Existing identities remain stable. Invalid stored freshness fails safely.
///
/// One absolute deadline covers preparation, acquisition, locks, statements and
/// commit. No automatic retry. Receipts require confirmed COMMIT, including a
/// confirmation observed late. A timeout after COMMIT dispatch is unknown, never
/// inferred rollback. Dropping the future abandons delivery and may abandon an
/// unknown commit outcome; the caller must account for this. Interrupted leases
/// are discarded with bounded cleanup. Failures contain no source diagnostics.
/// The adapter validates its encoded-resource bounds and backend representation
/// before I/O, independently of the request's raw-input limits.
pub trait WaniKaniGroupWriter {
    fn save_group<'a>(
        &'a self,
        group: &'a PreparedSaveGroup,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<SaveReceipt, ImportWriteError>> + 'a>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    fn time() -> DateTime<Utc> {
        Utc.timestamp_opt(1_700_000_000, 1).unwrap()
    }
    fn source() -> SourceRecord {
        SourceRecord::new(time(), r#"{"id":1,"nested":{"all":[1,2]}}"#).unwrap()
    }
    fn progress(id: u64) -> PreparedProgress {
        PreparedProgress {
            subject_external_id: id,
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
            LearnerId(uuid::Uuid::from_u128(1)),
            "account-a".into(),
            source(),
            subjects,
            progress,
        )
    }
    #[test]
    fn group_accepts_account_only_and_existing_subject_references() {
        assert!(group(vec![], vec![]).is_ok());
        assert!(group(vec![], vec![progress(99)]).is_ok());
        assert_eq!(source().updated_at(), time()); // Nanoseconds are not a contract rejection.
    }
    #[test]
    fn group_rejects_duplicate_identities() {
        assert!(group(vec![], vec![progress(1), progress(1)]).is_err());
        assert!(group(vec![], vec![progress(0)]).is_err());
        let subject = || PreparedSubject {
            external_id: 1,
            kind: Some(MaterialKind::Vocabulary),
            source: source(),
        };
        assert!(group(vec![subject(), subject()], vec![]).is_err());
        assert!(
            group(
                vec![PreparedSubject {
                    kind: Some(MaterialKind::Grammar),
                    ..subject()
                }],
                vec![]
            )
            .is_err()
        );
        assert!(
            PreparedSaveGroup::new(
                LearnerId(uuid::Uuid::nil()),
                String::new(),
                source(),
                vec![],
                vec![]
            )
            .is_err()
        );
    }
    #[test]
    fn source_record_validates_opaque_json_and_raw_bytes() {
        for raw in ["null", "[]", "123", "{bad}"] {
            assert!(SourceRecord::new(time(), raw).is_err());
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
        let fixed =
            "account-a".len() + source().payload().get().len() + p.source.payload().get().len();
        p.progress_stage = Some("あ".repeat((MAX_GROUP_INPUT_BYTES - fixed) / 3));
        let remaining = MAX_GROUP_INPUT_BYTES - fixed - p.progress_stage.as_ref().unwrap().len();
        p.progress_stage
            .as_mut()
            .unwrap()
            .push_str(&"x".repeat(remaining));
        assert!(group(vec![], vec![p.clone()]).is_ok());
        p.progress_stage.as_mut().unwrap().push('x');
        assert!(group(vec![], vec![p]).is_err());
    }
}
