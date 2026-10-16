use std::{future::Future, pin::Pin, time::Instant};

use sqlx::PgPool;
use yomibu_core::domain::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};

/// PostgreSQL implementation of [`LearnerCollectionQuery`].
///
/// Uses the retained schema's unique learner/collection association constraint.
/// The composition root supplies a pool backed by a live Tokio runtime with I/O
/// and time enabled. It configures finite acquisition and session statement
/// timeouts, credentials, TLS, and the schema search path.
///
/// The port's deadline covers pool acquisition, including SQLx's connection setup
/// and health checks, and the query itself. A query interrupted by its client
/// timeout closes its connection on drop, avoiding SQLx's normal return-to-pool
/// ping waiting for the abandoned statement. SQLx owns connection cleanup and
/// replacement; a client timeout does not prove immediate restoration of pool
/// capacity.
pub struct PostgresLearnerCollectionQuery {
    pool: PgPool,
}

impl PostgresLearnerCollectionQuery {
    /// Takes a pool handle without performing I/O or changing its configuration.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl LearnerCollectionQuery for PostgresLearnerCollectionQuery {
    fn collection_ids_for_learner(
        &self,
        learner_id: LearnerId,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<CollectionId>, StorageError>> + '_>> {
        Box::pin(async move {
            reject_expired(deadline)?;
            let mut connection = tokio::time::timeout_at(deadline.into(), self.pool.acquire())
                .await
                .map_err(|_| StorageError::collection_lookup_deadline_exceeded())?
                .map_err(storage_error)?;
            reject_expired(deadline)?;
            let query = sqlx::query_scalar::<_, uuid::Uuid>(
                "SELECT collection_id FROM learner_collections WHERE learner_id = $1",
            )
            .bind(learner_id.0)
            .fetch_all(&mut *connection);
            let ids = match tokio::time::timeout_at(deadline.into(), query).await {
                Ok(result) => result.map_err(storage_error)?,
                Err(_) => {
                    connection.close_on_drop();
                    return Err(StorageError::collection_lookup_deadline_exceeded());
                }
            };
            reject_expired(deadline)?;
            Ok(ids.into_iter().map(CollectionId).collect())
        })
    }
}

/// Fails once the deadline has been reached.
fn reject_expired(deadline: Instant) -> Result<(), StorageError> {
    if Instant::now() >= deadline {
        return Err(StorageError::collection_lookup_deadline_exceeded());
    }
    Ok(())
}

fn storage_error(error: sqlx::Error) -> StorageError {
    // Never expose provider messages: they can contain SQL, identifiers or secrets.
    let category = match error {
        sqlx::Error::Database(error) => {
            if let Some(code) = error.code()
                && code.len() == 5
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            {
                return StorageError(format!("collection lookup failed (SQLSTATE {code})"));
            }
            "database"
        }
        sqlx::Error::PoolClosed => "pool closed",
        sqlx::Error::PoolTimedOut => "pool acquisition timed out",
        sqlx::Error::Io(_) => "connection I/O",
        sqlx::Error::Tls(_) => "TLS",
        sqlx::Error::Configuration(_) => "configuration",
        sqlx::Error::Protocol(_) => "protocol",
        sqlx::Error::Decode(_) | sqlx::Error::ColumnDecode { .. } => "decode",
        _ => "driver",
    };
    StorageError(format!("collection lookup failed ({category})"))
}

#[cfg(test)]
mod tests {
    use super::storage_error;
    use yomibu_core::ports::infrastructure::StorageError;

    #[test]
    fn provider_details_are_not_exposed() {
        assert_eq!(
            storage_error(sqlx::Error::Protocol("sensitive provider detail".into())),
            StorageError("collection lookup failed (protocol)".into())
        );
    }
}
