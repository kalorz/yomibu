use std::time::{Duration, Instant};

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, PgPool};
use uuid::Uuid;
use yomibu_core::domain::{CollectionId, LearnerId, LearnerName};
use yomibu_core::ports::infrastructure::{
    LearnerCollectionQuery, LearnerWriteError, LearnerWriter, StorageError,
};
use yomibu_ext::PostgresLearnerStorage;

const SCHEMA: &str = include_str!("../../../docs/plan/schema.postgresql.sql");

async fn connect(options: PgConnectOptions) -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .expect("could not connect to the PostgreSQL test database")
}

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

    let storage = PostgresLearnerStorage::new(pool.clone());
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
    let closed_storage = PostgresLearnerStorage::new(closed);
    assert_eq!(
        closed_storage
            .collection_ids_for_learner(LearnerId(Uuid::from_u128(1)), Instant::now(),)
            .await,
        Err(StorageError::collection_lookup_deadline_exceeded())
    );
    assert_eq!(
        lookup(&closed_storage, 1).await,
        Err(StorageError(
            "collection lookup failed (pool closed)".into()
        ))
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
    // The abandoned statement is still blocked. It must not retain the sole slot.
    tokio::time::timeout(
        Duration::from_secs(1),
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(pool),
    )
    .await
    .expect("a timed-out query must release its pool slot without draining the statement")
    .unwrap();
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
        Err(StorageError(
            "collection lookup failed (SQLSTATE 57014)".into()
        ))
    );
    lock.rollback().await.unwrap();
    sqlx::query("RESET statement_timeout")
        .execute(pool)
        .await
        .unwrap();
    let restored: String = sqlx::query_scalar("SHOW statement_timeout")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(restored, "3s");
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
        Err(StorageError::collection_lookup_deadline_exceeded())
    );
}

#[tokio::test]
#[ignore = "requires YOMIBU_TEST_DATABASE_URL: disposable PostgreSQL 18+ database with vector installed"]
async fn postgres_learner_storage_contract() {
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
    let options = options.options([("search_path", format!("{schema},public"))]);
    let control = connect(options.clone()).await;
    sqlx::raw_sql(SCHEMA).execute(&control).await.unwrap();
    let pool = connect(options).await;
    check_contract(&pool, &control).await;
    check_write_contract(&pool, &control).await;
    pool.close().await;
    control.close().await;
    sqlx::query(AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

async fn create(
    storage: &dyn LearnerWriter,
    name: &str,
    budget: Duration,
) -> Result<LearnerId, LearnerWriteError> {
    storage
        .create_learner(LearnerName::new(name).unwrap(), Instant::now() + budget)
        .await
}

async fn check_write_contract(pool: &PgPool, control: &PgPool) {
    let storage = PostgresLearnerStorage::new(pool.clone());
    let budget = Duration::from_secs(5);
    let name = "O'Reilly 東京";
    let backend: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(pool)
        .await
        .unwrap();
    let first = create(&storage, name, budget).await.unwrap();
    let after: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        after, backend,
        "confirmed writes should return their connection to the pool"
    );
    let saved: String = sqlx::query_scalar("SELECT name FROM learners WHERE id = $1")
        .bind(first.0)
        .fetch_one(control)
        .await
        .unwrap();
    assert_eq!(saved, name);
    assert_eq!(first.0.get_version_num(), 7);

    // Test-only deferred constraint: the RETURNING row precedes commit failure.
    sqlx::query("ALTER TABLE learners ADD CONSTRAINT test_name_unique UNIQUE (name) DEFERRABLE INITIALLY DEFERRED")
        .execute(control).await.unwrap();
    let error = create(&storage, name, budget).await.unwrap_err();
    assert!(matches!(error, LearnerWriteError::NotCreated(_)));
    assert!(error.to_string().contains("SQLSTATE 23505"));
    assert!(!error.to_string().contains(name));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM learners WHERE name = $1")
        .bind(name)
        .fetch_one(control)
        .await
        .unwrap();
    assert_eq!(count, 1);
    sqlx::query("ALTER TABLE learners DROP CONSTRAINT test_name_unique")
        .execute(control)
        .await
        .unwrap();
    let second = create(&storage, name, budget).await.unwrap();
    assert_ne!(first, second);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM learners WHERE name = $1")
        .bind(name)
        .fetch_one(control)
        .await
        .unwrap();
    assert_eq!(count, 2);

    sqlx::query("ALTER TABLE learners RENAME TO hidden_learners")
        .execute(control)
        .await
        .unwrap();
    let error = create(&storage, "missing table", budget).await.unwrap_err();
    assert!(matches!(error, LearnerWriteError::NotCreated(_)));
    assert!(error.to_string().contains("SQLSTATE 42P01"));
    assert!(!error.to_string().contains("hidden_learners"));
    sqlx::query("ALTER TABLE hidden_learners RENAME TO learners")
        .execute(control)
        .await
        .unwrap();

    // Expiry and pool acquisition failure happen before any insert is dispatched.
    let closed = PgPoolOptions::new().connect_lazy_with(pool.connect_options().as_ref().clone());
    closed.close().await;
    let closed = PostgresLearnerStorage::new(closed);
    let error = create(&closed, "not dispatched", Duration::ZERO)
        .await
        .unwrap_err();
    assert!(matches!(error, LearnerWriteError::NotCreated(_)));
    assert!(error.to_string().contains("deadline"));
    let error = create(&closed, "not dispatched", budget).await.unwrap_err();
    assert!(matches!(error, LearnerWriteError::NotCreated(_)));
    assert!(error.to_string().contains("pool closed"));
    let held = pool.acquire().await.unwrap();
    let error = create(&storage, "not dispatched", Duration::from_millis(100))
        .await
        .unwrap_err();
    assert!(matches!(error, LearnerWriteError::NotCreated(_)));
    drop(held);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM learners WHERE name = 'not dispatched'")
            .fetch_one(control)
            .await
            .unwrap();
    assert_eq!(count, 0);

    let mut lock = control.begin().await.unwrap();
    sqlx::query("LOCK TABLE learners IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let error = tokio::time::timeout(
        Duration::from_secs(1),
        create(&storage, "timeout", Duration::from_millis(100)),
    )
    .await
    .expect("write deadline must beat watchdog")
    .unwrap_err();
    assert!(matches!(error, LearnerWriteError::CompletionUnknown(_)));
    // The abandoned write must not retain the only connection slot.
    tokio::time::timeout(
        Duration::from_secs(1),
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(pool),
    )
    .await
    .unwrap()
    .unwrap();
    lock.rollback().await.unwrap();

    // Terminate a write while it is blocked, then cancel another caller future.
    // Use a separate observer connection while control holds the table lock.
    for cancel in [false, true] {
        let backend: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(pool)
            .await
            .unwrap();
        let mut lock = control.begin().await.unwrap();
        sqlx::query("LOCK TABLE learners IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *lock)
            .await
            .unwrap();
        let observer = connect(control.connect_options().as_ref().clone()).await;
        let mut write = Box::pin(create(&storage, "interrupted", budget));
        tokio::select! {
            result = &mut write => panic!("locked write completed early: {result:?}"),
            () = wait_for_blocked_write(&observer, backend) => {}
        }
        if cancel {
            drop(write);
        } else {
            sqlx::query("SELECT pg_terminate_backend($1, 1000)")
                .bind(backend)
                .execute(&observer)
                .await
                .unwrap();
            let error = write.await.unwrap_err();
            assert!(matches!(error, LearnerWriteError::CompletionUnknown(_)));
        }
        let replacement: i32 = tokio::time::timeout(
            Duration::from_secs(1),
            sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(pool),
        )
        .await
        .unwrap()
        .unwrap();
        assert_ne!(replacement, backend);
        lock.rollback().await.unwrap();
        observer.close().await;
    }
}

async fn wait_for_blocked_write(control: &PgPool, backend: i32) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS (SELECT FROM pg_stat_activity WHERE pid = $1 AND state = 'active' AND wait_event_type = 'Lock')")
                .bind(backend).fetch_one(control).await.unwrap();
            if blocked { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("write must reach PostgreSQL before interruption");
}
