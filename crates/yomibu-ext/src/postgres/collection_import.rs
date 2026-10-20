use super::{PostgresLearnerStorage, WriteConnection, bounded, expired, failure_reason};
use std::{future::Future, pin::Pin, time::Instant};
use yomibu_core::ports::infrastructure::storage::{
    StorageError, SubjectQuery, validate_subject_lookup,
};
const MAX_ENCODED_BYTES: usize = 8 * 1024 * 1024;
const ROW_OVERHEAD: usize = 512;

fn lookup_deadline() -> StorageError {
    StorageError("subject lookup deadline exceeded".into())
}
fn lookup_failure(error: Option<sqlx::Error>) -> StorageError {
    error.map_or_else(lookup_deadline, |error| {
        StorageError(format!("subject lookup failed ({})", failure_reason(error)))
    })
}

impl SubjectQuery for PostgresLearnerStorage {
    fn existing_subject_ids<'a>(
        &'a self,
        provider: &'a str,
        key: &'a str,
        ids: &'a [String],
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, StorageError>> + 'a>> {
        Box::pin(async move {
            expired(deadline, lookup_deadline)?;
            validate_subject_lookup(provider, key, ids)?;
            // The contract bounds raw bytes; this adapter also bounds array and
            // parameter encoding before SQLx allocates argument buffers.
            if provider.len() + key.len() + array_bytes(ids) > MAX_ENCODED_BYTES {
                return Err(StorageError(
                    "subject lookup exceeds encoded argument limit".into(),
                ));
            }
            if ids.is_empty() {
                return Ok(vec![]);
            }
            let connection = bounded(deadline, self.pool.acquire(), lookup_failure).await?;
            let mut lease = WriteConnection {
                connection,
                confirmed: false,
            };
            let query = sqlx::query_scalar::<_, String>(
                "SELECT s.external_id FROM subjects s JOIN collections c ON c.id=s.collection_id
                 WHERE c.provider=$1 AND c.key=$2 AND c.owner_learner_id IS NULL
                 AND s.external_id = ANY($3)",
            )
            .bind(provider)
            .bind(key)
            .bind(ids)
            .fetch_all(&mut *lease.connection);
            let rows = bounded(deadline, query, lookup_failure).await?;
            lease.confirmed = true;
            expired(deadline, lookup_deadline)?;
            finish_lookup(rows, deadline)
        })
    }
}

// Sorting is part of the read operation and can exhaust its remaining budget.
fn finish_lookup(mut rows: Vec<String>, deadline: Instant) -> Result<Vec<String>, StorageError> {
    rows.sort_unstable();
    expired(deadline, lookup_deadline)?;
    Ok(rows)
}

/// Encoded size of one text-array parameter holding the given IDs.
fn array_bytes<'a>(ids: impl IntoIterator<Item = &'a String>) -> usize {
    ids.into_iter().map(|id| id.len() + 4).sum::<usize>() + ROW_OVERHEAD
}

#[cfg(test)]
mod tests;
