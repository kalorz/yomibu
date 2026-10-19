//! Explicit transaction lifecycle. A row returned before COMMIT is not a receipt.
use super::{WriteConnection, bounded, expired, failure_reason, rejected_by};
use sqlx::{Connection, PgPool, Postgres, Transaction, postgres::PgSeverity};
use std::{
    future::Future,
    time::{Duration, Instant},
};
use yomibu_core::ports::infrastructure::collection_import::ImportWriteError;

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
    let result = body(&mut tx).await;
    let (result, confirmed) = finish(tx, result, deadline).await;
    lease.confirmed = confirmed;
    result
}

// Recompute limits before each statement; a chunk never gets a fresh budget.
pub(super) async fn limits(
    tx: &mut Transaction<'_, Postgres>,
    deadline: Instant,
) -> Result<(), ImportWriteError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    let millis = remaining
        .as_millis()
        .saturating_add(1)
        .min(i32::MAX as u128);
    until(
        deadline,
        sqlx::query(
            "SELECT set_config('statement_timeout',$1,true), set_config('lock_timeout',$1,true)",
        )
        .bind(format!("{millis}ms"))
        .execute(&mut **tx),
    )
    .await?;
    Ok(())
}

// The boolean separately reports a confirmed clean transaction boundary. A
// NotSaved result alone cannot establish that an interrupted connection is reusable.
async fn finish<T>(
    mut tx: Transaction<'_, Postgres>,
    result: Result<T, ImportWriteError>,
    deadline: Instant,
) -> (Result<T, ImportWriteError>, bool) {
    let result = match result {
        Ok(value) => limits(&mut tx, deadline)
            .await
            .and_then(|()| expired(deadline, deadline_error))
            .map(|()| value),
        Err(error) => Err(error),
    };
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            // Before COMMIT dispatch no durable write is possible, even if the
            // rollback acknowledgement cannot be obtained. Only an acknowledged
            // rollback allows reuse; timeout/error leaves the lease marked for close.
            let rolled_back = matches!(
                tokio::time::timeout(Duration::from_millis(250), tx.rollback()).await,
                Ok(Ok(()))
            );
            return (Err(error), rolled_back);
        }
    };
    // Polling this future may dispatch COMMIT. No later rollback can establish
    // non-commit after a transport failure, timeout or unknown completion error.
    let result = match tokio::time::timeout_at(deadline.into(), tx.commit()).await {
        Ok(Ok(())) => return (Ok(value), true),
        Ok(Err(error)) => {
            let rejected = rejected_by(&error, commit_rejected);
            let reason = failure_reason(error);
            if rejected {
                Err(ImportWriteError::NotSaved(reason))
            } else {
                Err(ImportWriteError::CompletionUnknown(reason))
            }
        }
        Err(_) => Err(ImportWriteError::CompletionUnknown(
            "storage deadline exceeded during commit".into(),
        )),
    };
    (result, false)
}

fn commit_rejected(severity: PgSeverity, code: &str) -> bool {
    severity == PgSeverity::Error
        && (code.starts_with("23") || matches!(code, "40001" | "40P01" | "25P02"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_positive_commit_rejection_evidence_means_not_saved() {
        for code in ["23503", "23505", "23514", "40001", "40P01", "25P02"] {
            assert!(commit_rejected(PgSeverity::Error, code));
            assert!(!commit_rejected(PgSeverity::Fatal, code));
        }
        for code in ["40003", "08006", "57P01", "57014", "XX000", "P0001"] {
            assert!(!commit_rejected(PgSeverity::Error, code));
        }
    }
}
