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
