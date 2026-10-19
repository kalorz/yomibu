//! Explicit transaction lifecycle. A row returned before COMMIT is not a receipt.
use super::failure_reason;
use sqlx::{
    Postgres, Transaction,
    postgres::{PgDatabaseError, PgSeverity},
};
use std::{
    future::Future,
    time::{Duration, Instant},
};
use yomibu_core::ports::infrastructure::collection_import::ImportWriteError;

pub(super) fn expired(deadline: Instant) -> Result<(), ImportWriteError> {
    if Instant::now() >= deadline {
        Err(deadline_error())
    } else {
        Ok(())
    }
}
fn deadline_error() -> ImportWriteError {
    ImportWriteError::NotSaved("storage deadline exceeded".into())
}
pub(super) async fn until<T>(
    deadline: Instant,
    operation: impl Future<Output = Result<T, sqlx::Error>>,
) -> Result<T, ImportWriteError> {
    expired(deadline)?;
    tokio::time::timeout_at(deadline.into(), operation)
        .await
        .map_err(|_| deadline_error())?
        .map_err(|error| ImportWriteError::NotSaved(failure_reason(error)))
}

// Recompute limits before each statement; a chunk never gets a fresh budget.
pub(super) async fn limits(
    tx: &mut Transaction<'_, Postgres>,
    deadline: Instant,
) -> Result<(), ImportWriteError> {
    expired(deadline)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    let millis = remaining
        .as_millis()
        .saturating_add(1)
        .min(i32::MAX as u128);
    let timeout = format!("{millis}ms");
    until(
        deadline,
        sqlx::query(
            "SELECT set_config('statement_timeout',$1,true), set_config('lock_timeout',$1,true)",
        )
        .bind(timeout)
        .execute(&mut **tx),
    )
    .await?;
    Ok(())
}

pub(super) async fn finish<T>(
    mut tx: Transaction<'_, Postgres>,
    result: Result<T, ImportWriteError>,
    deadline: Instant,
) -> Result<T, ImportWriteError> {
    let result = match result {
        Ok(value) => limits(&mut tx, deadline)
            .await
            .and_then(|()| expired(deadline))
            .map(|()| value),
        Err(error) => Err(error),
    };
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            // Before COMMIT dispatch no durable write is possible, even if the
            // rollback acknowledgement cannot be obtained. The lease is closed.
            let _ = tokio::time::timeout(Duration::from_millis(250), tx.rollback()).await;
            return Err(error);
        }
    };
    // Polling this future may dispatch COMMIT. No later rollback can establish
    // non-commit after a transport failure, timeout or unknown completion error.
    match tokio::time::timeout_at(deadline.into(), tx.commit()).await {
        Ok(Ok(())) => Ok(value),
        Ok(Err(error)) => {
            let rejected = error
                .as_database_error()
                .and_then(|e| e.try_downcast_ref::<PgDatabaseError>())
                .is_some_and(|e| commit_rejected(e.severity(), e.code()));
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
    }
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
