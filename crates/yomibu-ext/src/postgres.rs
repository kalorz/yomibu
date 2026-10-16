use std::{future::Future, pin::Pin, time::Instant};

use sqlx::{PgPool, Postgres, pool::PoolConnection};
use yomibu_core::domain::{CollectionId, LearnerId, LearnerName};
use yomibu_core::ports::infrastructure::{
    LearnerCollectionQuery, LearnerWriteError, LearnerWriter, StorageError,
};

/// PostgreSQL implementation of [`LearnerCollectionQuery`] and [`LearnerWriter`].
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
///
/// Writes use the retained schema's UUID default. A successfully completed write
/// returns its connection to the pool; failed or abandoned writes close it.
/// Syntax/access and integrity-constraint rejections establish `NotCreated`;
/// other post-dispatch failures are conservatively `CompletionUnknown`.
pub struct PostgresLearnerStorage {
    pool: PgPool,
}

impl PostgresLearnerStorage {
    /// Takes a pool handle without performing I/O or changing its configuration.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl LearnerCollectionQuery for PostgresLearnerStorage {
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

impl LearnerWriter for PostgresLearnerStorage {
    fn create_learner(
        &self,
        name: LearnerName,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<LearnerId, LearnerWriteError>> + '_>> {
        Box::pin(async move {
            let expired = || LearnerWriteError::NotCreated("creation deadline exceeded".into());
            if Instant::now() >= deadline {
                return Err(expired());
            }
            let connection = tokio::time::timeout_at(deadline.into(), self.pool.acquire())
                .await
                .map_err(|_| expired())?
                .map_err(|error| LearnerWriteError::NotCreated(failure_reason(error)))?;
            if Instant::now() >= deadline {
                return Err(expired());
            }
            let mut connection = WriteConnection {
                connection,
                confirmed: false,
            };
            // SQLx PostgreSQL fetch_one drains through ReadyForQuery, including
            // deferred-constraint errors. A RETURNING row alone is not success.
            let insert = sqlx::query_scalar::<_, uuid::Uuid>(
                "INSERT INTO learners (name) VALUES ($1) RETURNING id",
            )
            .bind(name.as_str())
            .fetch_one(&mut *connection.connection);
            let id = tokio::time::timeout_at(deadline.into(), insert)
                .await
                .map_err(|_| {
                    LearnerWriteError::CompletionUnknown("creation deadline exceeded".into())
                })?
                .map_err(write_error)?;
            connection.confirmed = true;
            // Preserve confirmed creation even if observed after the deadline.
            // Application classifies late completion while retaining the UUID.
            Ok(LearnerId(id))
        })
    }
}

// SQLx cannot disarm close_on_drop. Keep the lease reusable only after confirmed
// completion, while still closing if the caller drops the future at any await.
struct WriteConnection {
    connection: PoolConnection<Postgres>,
    confirmed: bool,
}

impl Drop for WriteConnection {
    fn drop(&mut self) {
        if !self.confirmed {
            self.connection.close_on_drop();
        }
    }
}

fn write_error(error: sqlx::Error) -> LearnerWriteError {
    use sqlx::postgres::{PgDatabaseError, PgSeverity};

    // Only classify explicit statement rejections, not fatal connection errors
    // or SQLSTATEs such as 40003 (statement_completion_unknown). No inference from
    // provider messages or from the mere presence of an ErrorResponse is safe.
    let rejected = error
        .as_database_error()
        .and_then(|error| error.try_downcast_ref::<PgDatabaseError>())
        .is_some_and(|error| {
            error.severity() == PgSeverity::Error
                && (error.code().starts_with("23") || error.code().starts_with("42"))
        });
    let reason = failure_reason(error);
    if rejected {
        LearnerWriteError::NotCreated(reason)
    } else {
        LearnerWriteError::CompletionUnknown(reason)
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
    StorageError(format!(
        "collection lookup failed ({})",
        failure_reason(error)
    ))
}

fn failure_reason(error: sqlx::Error) -> String {
    // Never expose provider messages: they can contain SQL, identifiers or secrets.
    let category = match error {
        sqlx::Error::Database(error) => {
            if let Some(code) = error.code()
                && code.len() == 5
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            {
                return format!("SQLSTATE {code}");
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
    category.into()
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
