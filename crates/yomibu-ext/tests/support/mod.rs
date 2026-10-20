//! PostgreSQL test harness shared by several test targets; each uses a subset.
#![allow(dead_code)]
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, PgPool, Postgres, Transaction};
use std::time::Duration;
use uuid::Uuid;
use yomibu_ext::PostgresLearnerStorage;
const SCHEMA: &str = include_str!("../../../../docs/plan/schema.postgresql.sql");

pub(crate) async fn connect(options: PgConnectOptions) -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .expect("could not connect to the PostgreSQL test database")
}

/// An independent single-connection pool on the same test schema.
pub(crate) async fn another_session(pool: &PgPool) -> PgPool {
    connect((*pool.connect_options()).clone()).await
}

/// Storage over a closed pool: acquisition always fails, so any outcome proves
/// the adapter decided before I/O.
pub(crate) async fn closed_storage() -> PostgresLearnerStorage {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    pool.close().await;
    PostgresLearnerStorage::new(pool)
}

pub(crate) async fn backend_pid(pool: &PgPool) -> i32 {
    sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Locks `table` in the given mode until the returned transaction ends.
pub(crate) async fn lock_table(
    pool: &PgPool,
    table: &str,
    mode: &str,
) -> Transaction<'static, Postgres> {
    let mut lock = pool.begin().await.unwrap();
    sqlx::query(AssertSqlSafe(format!("LOCK TABLE {table} IN {mode} MODE")))
        .execute(&mut *lock)
        .await
        .unwrap();
    lock
}

/// Waits until every listed backend is waiting for a lock.
pub(crate) async fn blocked(observer: &PgPool, pids: &[i32]) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity WHERE pid=ANY($1) AND wait_event_type='Lock'",
            )
            .bind(pids)
            .fetch_one(observer)
            .await
            .unwrap();
            if waiting == pids.len() as i64 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("backends must reach PostgreSQL and block on the held lock");
}

/// The sole pool slot is usable again after an interrupted or failed operation.
pub(crate) async fn recover(pool: &PgPool) {
    tokio::time::timeout(Duration::from_secs(8), async {
        assert_eq!(
            sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(pool)
                .await
                .unwrap(),
            1
        );
    })
    .await
    .expect("sole pool slot did not recover");
}

/// Every import-owned row, for before/after equality checks.
pub(crate) async fn snapshot(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT jsonb_build_object('collections',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM collections c), 'sources',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM learner_progress_sources s),'subjects',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM subjects s),'progress',(SELECT jsonb_agg(to_jsonb(p) ORDER BY learner_progress_source_id,subject_id) FROM learner_progress p))::text").fetch_one(pool).await.unwrap()
}

pub(crate) async fn with_postgres(check: impl AsyncFnOnce(&PgPool, &PgPool)) {
    tokio::time::timeout(Duration::from_secs(30), run_postgres_contract(check))
        .await
        .expect("PostgreSQL setup, contract checks or shutdown exceeded the watchdog timeout");
}

async fn run_postgres_contract(check: impl AsyncFnOnce(&PgPool, &PgPool)) {
    let url = std::env::var("YOMIBU_TEST_DATABASE_URL")
        .expect("set YOMIBU_TEST_DATABASE_URL to a disposable PostgreSQL 18+ database with vector installed");
    let options: PgConnectOptions = url
        .parse()
        .expect("invalid test database connection options");
    let options = options.options([("statement_timeout", "3s")]);
    let admin = connect(options.clone()).await;
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert!(
        version.parse::<u32>().unwrap() >= 180000,
        "retained schema requires PostgreSQL 18+"
    );
    let vector: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT FROM pg_extension WHERE extname = 'vector')")
            .fetch_one(&admin)
            .await
            .unwrap();
    assert!(
        vector,
        "install the vector extension in the disposable database before running this test"
    );

    // PostgreSQL 18 supplies a unique suffix; only hex digits enter the identifier.
    let suffix: Uuid = sqlx::query_scalar("SELECT uuidv7()")
        .fetch_one(&admin)
        .await
        .unwrap();
    let schema = format!("yomibu_query_{}", suffix.simple());
    sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let options = options
        .application_name(&schema)
        .options([("search_path", format!("{schema},public"))]);
    // Keep setup and inspection separate from the adapter pool so tests can hold
    // locks or interrupt its session through an independent database connection.
    let control_pool = connect(options.clone()).await;
    sqlx::raw_sql(SCHEMA).execute(&control_pool).await.unwrap();
    let pool = connect(options).await;
    check(&pool, &control_pool).await;
    pool.close().await;
    control_pool.close().await;
    // A closed client socket can precede backend exit after an interrupted
    // statement. Avoid racing DROP locks with that still-draining session.
    loop {
        let active: i64 =
            sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE application_name=$1")
                .bind(&schema)
                .fetch_one(&admin)
                .await
                .unwrap();
        if active == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    sqlx::query(AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
