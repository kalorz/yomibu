use super::{PostgresLearnerStorage, WriteConnection, failure_reason};
use std::{future::Future, pin::Pin, time::Instant};
use yomibu_core::ports::infrastructure::{
    StorageError,
    wanikani::{MAX_SUBJECT_LOOKUP_IDS, WaniKaniSubjectQuery},
};

fn lookup_expired(deadline: Instant) -> Result<(), StorageError> {
    if Instant::now() >= deadline {
        Err(lookup_deadline())
    } else {
        Ok(())
    }
}
fn lookup_deadline() -> StorageError {
    StorageError("subject lookup deadline exceeded".into())
}
fn lookup_error(error: sqlx::Error) -> StorageError {
    StorageError(format!("subject lookup failed ({})", failure_reason(error)))
}

impl WaniKaniSubjectQuery for PostgresLearnerStorage {
    fn existing_subject_ids<'a>(
        &'a self,
        ids: &'a [u64],
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u64>, StorageError>> + 'a>> {
        Box::pin(async move {
            lookup_expired(deadline)?;
            if ids.len() > MAX_SUBJECT_LOOKUP_IDS || ids.contains(&0) {
                return Err(StorageError(
                    "invalid subject lookup: require at most 1000 nonzero IDs".into(),
                ));
            }
            if ids.is_empty() {
                return Ok(vec![]);
            }
            let ids: Vec<String> = ids.iter().map(u64::to_string).collect();
            let connection = tokio::time::timeout_at(deadline.into(), self.pool.acquire())
                .await
                .map_err(|_| lookup_deadline())?
                .map_err(lookup_error)?;
            let mut lease = WriteConnection {
                connection,
                confirmed: false,
            };
            lookup_expired(deadline)?;
            let rows = tokio::time::timeout_at(deadline.into(), sqlx::query_scalar::<_,String>(
                "SELECT s.external_id FROM subjects s JOIN collections c ON c.id=s.collection_id
                 WHERE c.provider='wanikani' AND c.key='subjects' AND c.owner_learner_id IS NULL
                 AND s.external_id = ANY($1)")
                 .bind(ids).fetch_all(&mut *lease.connection)).await
                 .map_err(|_|lookup_deadline())?.map_err(lookup_error)?;
            lease.confirmed = true;
            lookup_expired(deadline)?;
            let mut ids: Vec<u64> = rows
                .iter()
                .map(|id| id.parse())
                .collect::<Result<_, _>>()
                .map_err(|_| StorageError("invalid stored subject identity".into()))?;
            ids.sort_unstable();
            Ok(ids)
        })
    }
}
