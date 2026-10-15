use std::time::{Duration, Instant};

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, PgPool};
use uuid::Uuid;
use yomibu_core::domain::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};
use yomibu_ext::PostgresLearnerCollectionQuery;

const SCHEMA: &str = include_str!("../../../docs/plan/schema.postgresql.sql");

async fn lookup(
    storage: &dyn LearnerCollectionQuery,
    learner: u128,
) -> Result<Vec<CollectionId>, StorageError> {
    storage
        .collection_ids_for_learner(
            LearnerId(Uuid::from_u128(learner)),
            Instant::now() + Duration::from_secs(5),
        )
        .await
}

async fn check_contract(pool: &PgPool, control: &PgPool) {
    sqlx::raw_sql(
        "INSERT INTO learners (id, name) VALUES
            ('00000000-0000-0000-0000-000000000001', 'A'),
            ('00000000-0000-0000-0000-000000000002', 'B'),
            ('00000000-0000-0000-0000-000000000003', 'Unassociated');
         INSERT INTO collections (id, owner_learner_id, provider, name) VALUES
            ('00000000-0000-0000-0000-000000000010', NULL, 'manual', 'Shared'),
            ('00000000-0000-0000-0000-000000000020', '00000000-0000-0000-0000-000000000001', 'manual', 'A owned'),
            ('00000000-0000-0000-0000-000000000030', '00000000-0000-0000-0000-000000000002', 'manual', 'B owned'),
            ('00000000-0000-0000-0000-000000000040', '00000000-0000-0000-0000-000000000001', 'manual', 'Owned but unassociated'),
            ('00000000-0000-0000-0000-000000000050', NULL, 'manual', 'Shared but unassociated');
         INSERT INTO learner_collections (learner_id, collection_id) VALUES
            ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000010'),
            ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000020'),
            ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000010'),
            ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000030');"
    ).execute(control).await.unwrap();

    let storage = PostgresLearnerCollectionQuery::new(pool.clone());
    for (learner, expected) in [(1, vec![0x10, 0x20]), (2, vec![0x10, 0x30])] {
        let mut actual: Vec<_> = lookup(&storage, learner)
            .await
            .unwrap()
            .into_iter()
            .map(|id| id.0.as_u128())
            .collect();
        actual.sort_unstable(); // Compare membership and multiplicity, not database order.
        assert_eq!(actual, expected);
    }
    let duplicate = sqlx::query(
        "INSERT INTO learner_collections (learner_id, collection_id)
         VALUES ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000010')",
    )
    .execute(control)
    .await
    .unwrap_err();
    assert_eq!(
        duplicate.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
    assert_eq!(lookup(&storage, 3).await, Ok(vec![]));
    assert_eq!(lookup(&storage, 99).await, Ok(vec![]));

    // An expired call must win even when the pool cannot execute a query.
    let closed = PgPoolOptions::new().connect_lazy_with(pool.connect_options().as_ref().clone());
    closed.close().await;
    let closed_storage = PostgresLearnerCollectionQuery::new(closed);
    assert_eq!(
        closed_storage
            .collection_ids_for_learner(LearnerId(Uuid::from_u128(1)), Instant::now(),)
            .await,
        Err(StorageError("collection lookup deadline exceeded".into()))
    );
    assert_eq!(
        lookup(&closed_storage, 1).await,
        Err(StorageError("collection lookup failed".into()))
    );

    // Acquiring the sole connection is inside the request deadline.
    let held = pool.acquire().await.unwrap();
    assert_deadline(&storage).await;
    drop(held);
    assert_eq!(lookup(&storage, 1).await.unwrap().len(), 2);

    // Client deadline is shorter than the session's 3s statement timeout.
    let mut lock = control.begin().await.unwrap();
    sqlx::query("LOCK TABLE learner_collections IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    assert_deadline(&storage).await;
    lock.rollback().await.unwrap();
    assert_eq!(lookup(&storage, 1).await.unwrap().len(), 2);

    // A server-side execution error is sanitized, not converted to empty success.
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query("SET statement_timeout = '50ms'")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    let mut lock = control.begin().await.unwrap();
    sqlx::query("LOCK TABLE learner_collections IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    assert_eq!(
        lookup(&storage, 1).await,
        Err(StorageError("collection lookup failed".into()))
    );
    lock.rollback().await.unwrap();
    assert_eq!(lookup(&storage, 1).await.unwrap().len(), 2);

    // Break the pooled connection; the next independent call can recover.
    let backend: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(pool)
        .await
        .unwrap();
    let terminated: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1, 1000)")
        .bind(backend)
        .fetch_one(control)
        .await
        .unwrap();
    assert!(terminated);
    assert_eq!(lookup(&storage, 1).await.unwrap().len(), 2);
}

async fn assert_deadline(storage: &dyn LearnerCollectionQuery) {
    let deadline = Instant::now() + Duration::from_millis(100);
    let result = tokio::time::timeout(
        Duration::from_secs(1),
        storage.collection_ids_for_learner(LearnerId(Uuid::from_u128(1)), deadline),
    )
    .await
    .expect("adapter must finish before the longer watchdog timeout");
    assert_eq!(
        result,
        Err(StorageError("collection lookup deadline exceeded".into()))
    );
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires YOMIBU_TEST_DATABASE_URL: disposable PostgreSQL 18+ database with vector installed"]
async fn postgres_collection_query_contract() {
    tokio::time::timeout(Duration::from_secs(30), run_postgres_contract())
        .await
        .expect("PostgreSQL setup, contract checks or shutdown exceeded the watchdog timeout");
}

async fn run_postgres_contract() {
    let url = std::env::var("YOMIBU_TEST_DATABASE_URL")
        .expect("set YOMIBU_TEST_DATABASE_URL to a disposable PostgreSQL 18+ database with vector installed");
    let options: PgConnectOptions = url
        .parse()
        .expect("invalid test database connection options");
    let options = options.options([("statement_timeout", "3s")]);
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options.clone())
        .await
        .unwrap_or_else(|_| panic!("could not connect to the PostgreSQL test database"));
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

    // The only dynamic SQL identifier consists of a fixed prefix and numeric PID.
    let schema = format!("yomibu_query_{}", std::process::id());
    sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let options = options.options([("search_path", format!("{schema},public"))]);
    let control = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options.clone())
        .await
        .unwrap();
    sqlx::raw_sql(SCHEMA).execute(&control).await.unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .unwrap();
    check_contract(&pool, &control).await;
    pool.close().await;
    control.close().await;
    sqlx::query(AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
