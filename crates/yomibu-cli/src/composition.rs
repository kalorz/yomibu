//! Prepare CLI requests, invoke the application, and bound the database lifecycle.

use std::time::{Duration, Instant};

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use yomibu_app::application::{
    CreateLearner, CreateLearnerError, ListCollectionIds, prepare_learner_name,
};
use yomibu_core::domain::{CollectionId, LearnerId, LearnerName};
use yomibu_ext::PostgresLearnerStorage;

const IO_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

pub(super) fn run_collections(learner: LearnerId) -> Result<Vec<CollectionId>, String> {
    let url = database_url()?;
    block_on_database(collections(learner, url))
}

pub(super) fn run_create_learner(name: &str) -> Result<LearnerId, String> {
    let name = prepare_learner_name(name).map_err(|error| error.to_string())?;
    let url = database_url()?;
    block_on_database(create_learner(name, url))
}

fn creation_after_shutdown(
    result: Result<LearnerId, CreateLearnerError>,
    shutdown: Result<(), &'static str>,
) -> Result<LearnerId, String> {
    let id = result.map_err(|error| error.to_string())?;
    shutdown.map_err(|reason| CreateLearnerError::Created { id, reason }.to_string())?;
    Ok(id)
}

fn database_url() -> Result<String, String> {
    let url = std::env::var("YOMIBU_DATABASE_URL").map_err(|error| match error {
        std::env::VarError::NotPresent => {
            "YOMIBU_DATABASE_URL must be set to a PostgreSQL connection URL"
        }
        std::env::VarError::NotUnicode(_) => "YOMIBU_DATABASE_URL must contain valid Unicode",
    })?;
    Ok(prepare_database_url(&url)?.to_owned())
}

fn prepare_database_url(url: &str) -> Result<&str, &'static str> {
    let url = url.trim();
    if url.is_empty() {
        return Err("YOMIBU_DATABASE_URL must not be empty");
    }
    // SQLx parses URL components but does not validate the scheme.
    if !url.split_once("://").is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("postgres") || scheme.eq_ignore_ascii_case("postgresql")
    }) {
        return Err("YOMIBU_DATABASE_URL must use the postgres or postgresql URL scheme");
    }
    Ok(url)
}

fn block_on_database<T>(operation: impl Future<Output = Result<T, String>>) -> Result<T, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "could not start the database runtime")?;
    let result = runtime.block_on(operation);
    // Abandoned blocking work (for example DNS) must not hold process exit open.
    runtime.shutdown_timeout(SHUTDOWN_TIMEOUT);
    result
}

async fn connect(url: String) -> Result<sqlx::PgPool, String> {
    tokio::time::timeout(IO_TIMEOUT, async {
        // The driver's option parsing may read credential files. Include that
        // blocking work in the startup bound, alongside connection establishment.
        let options = tokio::task::spawn_blocking(move || url.parse::<PgConnectOptions>())
            .await
            .map_err(|_| "database configuration task failed")?
            .map_err(|_| "YOMIBU_DATABASE_URL is not a valid PostgreSQL connection URL")?;
        PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(IO_TIMEOUT)
            .connect_with(
                options.options([("statement_timeout", IO_TIMEOUT.as_millis().to_string())]),
            )
            .await
            // Provider errors can contain connection credentials or server details.
            .map_err(|_| "database startup failed; check connection settings and availability")
    })
    .await
    .map_err(|_| "database startup timed out")?
    .map_err(Into::into)
}

async fn close_pool(pool: &sqlx::PgPool) -> Result<(), &'static str> {
    tokio::time::timeout(SHUTDOWN_TIMEOUT, pool.close())
        .await
        .map_err(|_| "database shutdown timed out")
}

async fn collections(learner: LearnerId, url: String) -> Result<Vec<CollectionId>, String> {
    let pool = connect(url).await?;
    let storage = PostgresLearnerStorage::new(pool.clone());
    let result = ListCollectionIds::new(&storage)
        .execute(learner, Instant::now() + IO_TIMEOUT)
        .await;
    let shutdown = close_pool(&pool).await;
    // Preserve a lookup failure even if cleanup also times out; cleanup failure
    // after a successful lookup must not be reported as successful completion.
    let ids = result.map_err(|error| error.to_string())?;
    shutdown?;
    Ok(ids)
}

async fn create_learner(name: LearnerName, url: String) -> Result<LearnerId, String> {
    let pool = connect(url).await?;
    let storage = PostgresLearnerStorage::new(pool.clone());
    let result = CreateLearner::new(&storage)
        .execute(name, Instant::now() + IO_TIMEOUT)
        .await;
    let shutdown = close_pool(&pool).await;
    creation_after_shutdown(result, shutdown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn database_url_preparation_trims_whitespace_and_accepts_postgres_schemes() {
        for (input, expected) in [
            ("postgres://localhost/db", "postgres://localhost/db"),
            (
                " \tPoStGrEsQl://localhost/db\n\u{3000}",
                "PoStGrEsQl://localhost/db",
            ),
        ] {
            assert_eq!(prepare_database_url(input), Ok(expected));
        }
    }

    #[test]
    fn database_url_preparation_rejects_empty_input_and_other_schemes() {
        for input in ["", " \t\n\u{3000}"] {
            assert_eq!(
                prepare_database_url(input),
                Err("YOMIBU_DATABASE_URL must not be empty")
            );
        }
        for input in [
            "http://user:secret-marker@localhost/db",
            "localhost/db",
            "postgres:db",
            "://localhost/db",
        ] {
            assert_eq!(
                prepare_database_url(input),
                Err("YOMIBU_DATABASE_URL must use the postgres or postgresql URL scheme")
            );
        }
    }

    #[test]
    fn cleanup_failure_preserves_creation_and_prior_errors() {
        let id = LearnerId(Uuid::from_u128(7));
        let shutdown = Err("database shutdown timed out");
        assert_eq!(
            creation_after_shutdown(Ok(id), shutdown),
            Err(CreateLearnerError::Created {
                id,
                reason: "database shutdown timed out"
            }
            .to_string())
        );
        let error = CreateLearnerError::DeadlineExceeded;
        let expected = error.to_string();
        assert_eq!(creation_after_shutdown(Err(error), shutdown), Err(expected));
    }
}
