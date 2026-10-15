use std::{future::Future, pin::Pin, time::Instant};

use sqlx::PgPool;
use yomibu_core::domain::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};

/// PostgreSQL implementation of [`LearnerCollectionQuery`].
///
/// Uses the retained schema's unique learner/collection association constraint.
/// The composition root supplies a pool backed by a live Tokio runtime with I/O
/// and time enabled. It configures finite acquisition and session statement
/// timeouts, credentials, TLS, and the schema search path. Startup and graceful
/// shutdown must be separately bounded by that root.
///
/// The request deadline covers pool acquisition (including connection setup and
/// health checks), query execution and result reception. SQLx owns connection
/// cleanup/replacement; cleanup may outlive the request. A client timeout does
/// not prove server cancellation or immediate restoration of pool capacity.
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
            if Instant::now() >= deadline {
                return Err(StorageError("collection lookup deadline exceeded".into()));
            }
            let query = sqlx::query_scalar::<_, uuid::Uuid>(
                "SELECT collection_id FROM learner_collections WHERE learner_id = $1",
            )
            .bind(learner_id.0)
            .fetch_all(&self.pool);
            let ids = tokio::time::timeout_at(deadline.into(), query)
                .await
                .map_err(|_| StorageError("collection lookup deadline exceeded".into()))?
                .map_err(|_| StorageError("collection lookup failed".into()))?;
            let ids = ids.into_iter().map(CollectionId).collect();
            if Instant::now() >= deadline {
                return Err(StorageError("collection lookup deadline exceeded".into()));
            }
            Ok(ids)
        })
    }
}
