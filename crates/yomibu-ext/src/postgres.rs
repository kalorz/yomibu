mod collection_import;
mod transaction;

use std::{future::Future, pin::Pin, time::Instant};

use sqlx::{
    PgPool, Postgres,
    pool::PoolConnection,
    postgres::{PgDatabaseError, PgSeverity},
};
use yomibu_core::domain::{CollectionId, LearnerId, LearnerName};
use yomibu_core::ports::infrastructure::storage::{
    LearnerCollectionQuery, LearnerWriteError, LearnerWriter, StorageError,
};

/// PostgreSQL implementation of [`LearnerCollectionQuery`] and [`LearnerWriter`].
///
/// Derives collection membership from the retained schema's learner progress sources.
/// Multiple sources for one collection produce one collection ID.
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
/// Writes use the retained schema's UUID default. Confirmed commits and explicitly
/// acknowledged rollbacks return their connections to the pool; interrupted or
/// uncertain transaction cleanup closes the connection.
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
            expired(deadline, StorageError::collection_lookup_deadline_exceeded)?;
            let mut connection = bounded(deadline, self.pool.acquire(), |error| {
                error.map_or_else(
                    StorageError::collection_lookup_deadline_exceeded,
                    storage_error,
                )
            })
            .await?;
            expired(deadline, StorageError::collection_lookup_deadline_exceeded)?;
            let query = sqlx::query_scalar::<_, uuid::Uuid>(
                "SELECT DISTINCT collection_id FROM learner_progress_sources WHERE learner_id = $1",
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
            expired(deadline, StorageError::collection_lookup_deadline_exceeded)?;
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
            let not_created = || LearnerWriteError::NotCreated("creation deadline exceeded".into());
            let connection = bounded(deadline, self.pool.acquire(), |error| {
                error.map_or_else(not_created, |error| {
                    LearnerWriteError::NotCreated(failure_reason(error))
                })
            })
            .await?;
            expired(deadline, not_created)?;
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
            let id = bounded(deadline, insert, |error| {
                error.map_or_else(
                    || LearnerWriteError::CompletionUnknown("creation deadline exceeded".into()),
                    write_error,
                )
            })
            .await?;
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
    // SQLSTATEs such as 40003 (statement_completion_unknown) never qualify.
    let rejected = error
        .as_database_error()
        .and_then(|error| error.try_downcast_ref::<PgDatabaseError>())
        .is_some_and(|error| is_statement_rejection(error.severity(), error.code()));
    let reason = failure_reason(error);
    if rejected {
        LearnerWriteError::NotCreated(reason)
    } else {
        LearnerWriteError::CompletionUnknown(reason)
    }
}

fn is_statement_rejection(severity: PgSeverity, code: &str) -> bool {
    severity == PgSeverity::Error && (code.starts_with("23") || code.starts_with("42"))
}

/// Fails with `error` once the deadline has been reached.
fn expired<E>(deadline: Instant, error: impl FnOnce() -> E) -> Result<(), E> {
    if Instant::now() >= deadline {
        return Err(error());
    }
    Ok(())
}

/// Runs one driver operation under the deadline. `failure` receives the driver
/// error, or `None` when the deadline preceded or interrupted the operation.
async fn bounded<T, E>(
    deadline: Instant,
    operation: impl Future<Output = Result<T, sqlx::Error>>,
    failure: impl Fn(Option<sqlx::Error>) -> E,
) -> Result<T, E> {
    expired(deadline, || failure(None))?;
    match tokio::time::timeout_at(deadline.into(), operation).await {
        Ok(result) => result.map_err(|error| failure(Some(error))),
        Err(_) => Err(failure(None)),
    }
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
mod tests;
