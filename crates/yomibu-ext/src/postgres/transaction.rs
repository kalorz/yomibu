//! Explicit transaction lifecycle. A row returned before COMMIT is not a receipt.
use super::{WriteConnection, bounded, expired, failure_reason};
use sqlx::{Connection, PgPool, Postgres, Transaction};
use std::{
    future::Future,
    time::{Duration, Instant},
};
use yomibu_core::ports::infrastructure::storage::ImportWriteError;

pub(super) fn deadline_error() -> ImportWriteError {
    ImportWriteError::NotSaved("storage deadline exceeded".into())
}
fn write_failure(error: Option<sqlx::Error>) -> ImportWriteError {
    error.map_or_else(deadline_error, |error| {
        ImportWriteError::NotSaved(failure_reason(error))
    })
}
/// One deadline-bounded driver operation with the write ports' failure mapping.
pub(super) async fn until<T>(
    deadline: Instant,
    operation: impl Future<Output = Result<T, sqlx::Error>>,
) -> Result<T, ImportWriteError> {
    bounded(deadline, operation, write_failure).await
}

/// Runs `body` in one explicit READ COMMITTED transaction on a pooled connection.
/// Only a confirmed commit or an acknowledged rollback returns the connection to
/// the pool; interrupted or uncertain cleanup closes it.
/// Receipts require a confirmed COMMIT, including confirmation observed late.
/// Commit errors/timeouts conservatively report unknown completion, including rejections.
pub(super) async fn transact<T>(
    pool: &PgPool,
    deadline: Instant,
    body: impl AsyncFnOnce(&mut Transaction<'_, Postgres>) -> Result<T, ImportWriteError>,
) -> Result<T, ImportWriteError> {
    let connection = until(deadline, pool.acquire()).await?;
    let mut lease = WriteConnection {
        connection,
        confirmed: false,
    };
    let mut tx = until(
        deadline,
        lease
            .connection
            .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED"),
    )
    .await?;
    let result = async {
        // PostgreSQL does not rearm an inherited active timer when its value
        // changes. Disable it before applying this operation's remaining budget.
        until(
            deadline,
            sqlx::raw_sql("SET LOCAL transaction_timeout = 0").execute(&mut *tx),
        )
        .await?;
        // PostgreSQL bounds the whole transaction, including waits after caller
        // cancellation. Individual statements never receive a fresh time budget.
        let millis = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128);
        until(
            deadline,
            sqlx::query("SELECT set_config('transaction_timeout',$1,true)")
                .bind(format!("{millis}ms"))
                .execute(&mut *tx),
        )
        .await?;
        let value = body(&mut tx).await?;
        expired(deadline, deadline_error)?;
        Ok(value)
    }
    .await;
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            // Before COMMIT dispatch no durable write is possible, even if the
            // rollback acknowledgement cannot be obtained. Only an acknowledged
            // rollback allows reuse; timeout/error leaves the lease marked for close.
            lease.confirmed = matches!(
                tokio::time::timeout(Duration::from_millis(250), tx.rollback()).await,
                Ok(Ok(()))
            );
            return Err(error);
        }
    };
    // Polling this future may dispatch COMMIT. No later rollback can establish
    // non-commit after a transport failure, timeout or unknown completion error.
    match tokio::time::timeout_at(deadline.into(), tx.commit()).await {
        Ok(Ok(())) => {
            lease.confirmed = true;
            Ok(value)
        }
        Ok(Err(error)) => Err(ImportWriteError::CompletionUnknown(failure_reason(error))),
        Err(_) => Err(ImportWriteError::CompletionUnknown(
            "storage deadline exceeded during commit".into(),
        )),
    }
}
