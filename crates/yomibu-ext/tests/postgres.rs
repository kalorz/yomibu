use std::time::{Duration, Instant};

use sqlx::PgPool;
use sqlx::postgres::{PgPoolOptions, PgQueryResult};
use uuid::Uuid;
use yomibu_core::domain::{CollectionId, LearnerId, LearnerName};
use yomibu_core::ports::infrastructure::storage::{
    LearnerCollectionQuery, LearnerWriteError, LearnerWriter, StorageError,
};
use yomibu_ext::PostgresLearnerStorage;

mod support;
use support::{backend_pid, blocked, connect, lock_table, with_postgres};

async fn lookup(
    storage: &dyn LearnerCollectionQuery,
    learner_id: u128,
) -> Result<Vec<CollectionId>, StorageError> {
    storage
        .collection_ids_for_learner(
            LearnerId(Uuid::from_u128(learner_id)),
            Instant::now() + Duration::from_secs(5),
        )
        .await
}

async fn check_query_contract(pool: &PgPool, control_pool: &PgPool) {
    sqlx::raw_sql(
        "INSERT INTO learners (id, name) VALUES
            ('00000000-0000-0000-0000-000000000001', 'A'),
            ('00000000-0000-0000-0000-000000000002', 'B'),
            ('00000000-0000-0000-0000-000000000003', 'No progress sources');
         INSERT INTO collections (id, owner_learner_id, provider, name) VALUES
            ('00000000-0000-0000-0000-000000000010', NULL, 'manual', 'Shared'),
            ('00000000-0000-0000-0000-000000000020', '00000000-0000-0000-0000-000000000001', 'manual', 'A owned'),
            ('00000000-0000-0000-0000-000000000030', '00000000-0000-0000-0000-000000000002', 'manual', 'B owned'),
            ('00000000-0000-0000-0000-000000000040', '00000000-0000-0000-0000-000000000001', 'manual', 'Owned without a progress source'),
            ('00000000-0000-0000-0000-000000000050', NULL, 'manual', 'Shared without a progress source');
         INSERT INTO learner_progress_sources (learner_id, collection_id) VALUES
            ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000010'),
            ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000020'),
            ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000010'),
            ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000030');
         INSERT INTO learner_progress_sources (learner_id, collection_id, external_account_id) VALUES
            ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000010', 'account-a'),
            ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000010', 'account-b');"
    ).execute(control_pool).await.unwrap();

    let storage = PostgresLearnerStorage::new(pool.clone());
    for (learner_id, expected_collection_ids) in [(1, vec![0x10, 0x20]), (2, vec![0x10, 0x30])] {
        let mut actual_collection_ids: Vec<_> = lookup(&storage, learner_id)
            .await
            .unwrap()
            .into_iter()
            .map(|id| id.0.as_u128())
            .collect();
        actual_collection_ids.sort_unstable(); // Compare IDs and multiplicity, not database order.
        assert_eq!(actual_collection_ids, expected_collection_ids);
    }
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
    let lock = lock_table(control_pool, "learner_progress_sources", "ACCESS EXCLUSIVE").await;
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
    let lock = lock_table(control_pool, "learner_progress_sources", "ACCESS EXCLUSIVE").await;
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
    let backend = backend_pid(pool).await;
    let terminated: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1, 1000)")
        .bind(backend)
        .fetch_one(control_pool)
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
async fn postgres_collection_query_contract() {
    with_postgres(check_query_contract).await;
}

async fn seed_progress_sources(control_pool: &PgPool) {
    sqlx::raw_sql(
        r#"INSERT INTO learners (id, name) VALUES
            ('00000000-0000-0000-0000-000000000001', 'Alice'),
            ('00000000-0000-0000-0000-000000000002', 'Bob');
         INSERT INTO collections (id, provider, name) VALUES
            ('00000000-0000-0000-0000-000000000010', 'wanikani', 'WaniKani'),
            ('00000000-0000-0000-0000-000000000020', 'manual', 'Other');
         INSERT INTO subjects (id, collection_id, external_id, kind, source_data) VALUES
            ('00000000-0000-0000-0000-000000001000', '00000000-0000-0000-0000-000000000010', '42', 'vocabulary', '{"characters":"食べる"}'),
            ('00000000-0000-0000-0000-000000002000', '00000000-0000-0000-0000-000000000020', NULL, NULL, NULL);
         INSERT INTO learner_progress_sources (id, learner_id, collection_id, external_account_id, source_data) VALUES
            ('00000000-0000-0000-0000-000000000100', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000010', 'account-a', '{"max_level_granted":10}'),
            ('00000000-0000-0000-0000-000000000101', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000010', 'account-b', '{"max_level_granted":3,"expires_at":"2026-11-01T00:00:00Z"}'),
            ('00000000-0000-0000-0000-000000000200', '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000010', 'account-a', '{"max_level_granted":20}');"#,
    ).execute(control_pool).await.unwrap();
}

async fn insert_source(
    control_pool: &PgPool,
    learner_id: u128,
    collection_id: u128,
    external_account_id: Option<&str>,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query("INSERT INTO learner_progress_sources (learner_id, collection_id, external_account_id) VALUES ($1, $2, $3)")
        .bind(Uuid::from_u128(learner_id))
        .bind(Uuid::from_u128(collection_id))
        .bind(external_account_id)
        .execute(control_pool)
        .await
}

async fn insert_progress(
    control_pool: &PgPool,
    source_id: u128,
    subject_id: u128,
    collection_id: u128,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query("INSERT INTO learner_progress (learner_progress_source_id, subject_id, collection_id, recorded_at) VALUES ($1, $2, $3, '2026-10-18T00:00:00Z')")
        .bind(Uuid::from_u128(source_id))
        .bind(Uuid::from_u128(subject_id))
        .bind(Uuid::from_u128(collection_id))
        .execute(control_pool)
        .await
}

fn assert_sqlstate(result: Result<PgQueryResult, sqlx::Error>, code: &str) {
    let error = result.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some(code),
        "{error}"
    );
}

#[tokio::test]
#[ignore = "requires YOMIBU_TEST_DATABASE_URL: disposable PostgreSQL 18+ database with vector installed"]
async fn postgres_progress_source_identity() {
    with_postgres(async |_, control_pool| {
        seed_progress_sources(control_pool).await;
        // A null account can coexist with named accounts; scope is learner/collection.
        for (learner_id, collection_id, external_account_id) in [
            (1, 0x10, None),
            (2, 0x10, None),
            (1, 0x20, Some("account-a")),
            (1, 0x20, None),
        ] {
            insert_source(control_pool, learner_id, collection_id, external_account_id)
                .await
                .unwrap();
        }
        for external_account_id in [Some("account-a"), None] {
            assert_sqlstate(
                insert_source(control_pool, 1, 0x10, external_account_id).await,
                "23505",
            );
        }
    })
    .await;
}

async fn seed_progress(control_pool: &PgPool) {
    seed_progress_sources(control_pool).await;
    sqlx::raw_sql(
        r#"INSERT INTO learner_progress (learner_progress_source_id, subject_id, collection_id, progress_stage, is_hidden, learning_started_at, first_passed_at, recorded_at, source_data) VALUES
            ('00000000-0000-0000-0000-000000000100', '00000000-0000-0000-0000-000000001000', '00000000-0000-0000-0000-000000000010', '5', FALSE, '2026-10-01T00:00:00Z', '2026-10-08T00:00:00Z', '2026-10-18T00:00:00Z', '{"source_updated_at":"2026-10-17T00:00:00Z"}'),
            ('00000000-0000-0000-0000-000000000101', '00000000-0000-0000-0000-000000001000', '00000000-0000-0000-0000-000000000010', '1', TRUE, '2026-10-17T00:00:00Z', NULL, '2026-10-18T00:00:00Z', '{"excluded":true}'),
            ('00000000-0000-0000-0000-000000000200', '00000000-0000-0000-0000-000000001000', '00000000-0000-0000-0000-000000000010', '7', FALSE, NULL, NULL, '2026-10-18T00:00:00Z', '{"marker":"bob"}');"#,
    ).execute(control_pool).await.unwrap();
}

async fn source_snapshot(control_pool: &PgPool, id: u128) -> String {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('source', to_jsonb(s), 'progress',
            (SELECT jsonb_agg(to_jsonb(p) ORDER BY p.subject_id) FROM learner_progress p WHERE p.learner_progress_source_id = s.id))::text
         FROM learner_progress_sources s WHERE s.id = $1",
    ).bind(Uuid::from_u128(id)).fetch_one(control_pool).await.unwrap()
}

#[tokio::test]
#[ignore = "requires YOMIBU_TEST_DATABASE_URL: disposable PostgreSQL 18+ database with vector installed"]
async fn postgres_progress_is_source_scoped() {
    with_postgres(async |_, control_pool| {
        seed_progress(control_pool).await;
        let b_before = source_snapshot(control_pool, 0x101).await;
        let bob_before = source_snapshot(control_pool, 0x200).await;

        sqlx::query("UPDATE learner_progress SET progress_stage = '7' WHERE learner_progress_source_id = $1")
            .bind(Uuid::from_u128(0x100)).execute(control_pool).await.unwrap();
        sqlx::query(r#"UPDATE learner_progress_sources SET source_data = '{"max_level_granted":60}' WHERE id = $1"#)
            .bind(Uuid::from_u128(0x100)).execute(control_pool).await.unwrap();
        let updated: (String, i32) = sqlx::query_as(
            "SELECT p.progress_stage, (s.source_data->>'max_level_granted')::integer
             FROM learner_progress p JOIN learner_progress_sources s ON s.id = p.learner_progress_source_id
             WHERE s.id = $1 AND p.subject_id = $2",
        ).bind(Uuid::from_u128(0x100)).bind(Uuid::from_u128(0x1000)).fetch_one(control_pool).await.unwrap();
        assert_eq!(updated, ("7".into(), 60));
        assert_eq!(source_snapshot(control_pool, 0x101).await, b_before);
        assert_eq!(source_snapshot(control_pool, 0x200).await, bob_before);
        assert_sqlstate(insert_progress(control_pool, 0x100, 0x1000, 0x10).await, "23505");
    }).await;
}

#[tokio::test]
#[ignore = "requires YOMIBU_TEST_DATABASE_URL: disposable PostgreSQL 18+ database with vector installed"]
async fn postgres_progress_source_references() {
    with_postgres(async |_, control_pool| {
        seed_progress(control_pool).await;
        for (learner_id, collection_id) in [(0x999, 0x10), (1, 0x999)] {
            assert_sqlstate(
                insert_source(control_pool, learner_id, collection_id, None).await,
                "23503",
            );
        }
        // Missing source/subject, wrong source collection, then wrong subject collection.
        for (source_id, subject_id, collection_id) in [
            (0x999, 0x1000, 0x10), (0x100, 0x999, 0x10),
            (0x100, 0x2000, 0x20), (0x100, 0x2000, 0x10),
        ] {
            assert_sqlstate(
                insert_progress(control_pool, source_id, subject_id, collection_id).await,
                "23503",
            );
        }
        // Only a progress source references this collection, isolating its RESTRICT rule.
        sqlx::raw_sql(
            "INSERT INTO collections (id, provider, name) VALUES
                ('00000000-0000-0000-0000-000000000030', 'manual', 'No subjects');
             INSERT INTO learner_progress_sources (learner_id, collection_id) VALUES
                ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000030');",
        ).execute(control_pool).await.unwrap();
        // PostgreSQL 18 reports RESTRICT violations as 23001, not 23503.
        for (query, id) in [
            ("DELETE FROM learners WHERE id = $1", 1),
            ("DELETE FROM collections WHERE id = $1", 0x30),
            ("DELETE FROM subjects WHERE id = $1", 0x1000),
        ] {
            assert_sqlstate(sqlx::query(query).bind(Uuid::from_u128(id)).execute(control_pool).await, "23001");
        }
        sqlx::query("DELETE FROM learner_progress_sources WHERE id = $1")
            .bind(Uuid::from_u128(0x100)).execute(control_pool).await.unwrap();
        let remaining: Vec<Uuid> = sqlx::query_scalar("SELECT learner_progress_source_id FROM learner_progress ORDER BY learner_progress_source_id")
            .fetch_all(control_pool).await.unwrap();
        assert_eq!(remaining, [Uuid::from_u128(0x101), Uuid::from_u128(0x200)]);
    }).await;
}

#[tokio::test]
#[ignore = "requires YOMIBU_TEST_DATABASE_URL: disposable PostgreSQL 18+ database with vector installed"]
async fn postgres_learner_write_contract() {
    with_postgres(check_write_contract).await;
}

#[tokio::test]
#[ignore = "requires YOMIBU_TEST_DATABASE_URL: disposable PostgreSQL 18+ database with vector installed"]
async fn postgres_write_disconnection_releases_connection() {
    with_postgres(async |pool, control_pool| {
        check_interrupted_write(pool, control_pool, false).await
    })
    .await;
}

#[tokio::test]
#[ignore = "requires YOMIBU_TEST_DATABASE_URL: disposable PostgreSQL 18+ database with vector installed"]
async fn postgres_write_cancellation_releases_connection() {
    with_postgres(async |pool, control_pool| {
        check_interrupted_write(pool, control_pool, true).await
    })
    .await;
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

async fn learner_count(control_pool: &PgPool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM learners WHERE name = $1")
        .bind(name)
        .fetch_one(control_pool)
        .await
        .unwrap()
}

async fn check_write_contract(pool: &PgPool, control_pool: &PgPool) {
    let storage = PostgresLearnerStorage::new(pool.clone());
    let budget = Duration::from_secs(5);
    let name = "O'Reilly 東京";
    let backend = backend_pid(pool).await;
    let first = create(&storage, name, budget).await.unwrap();
    assert_eq!(
        backend_pid(pool).await,
        backend,
        "confirmed writes should return their connection to the pool"
    );
    let saved: String = sqlx::query_scalar("SELECT name FROM learners WHERE id = $1")
        .bind(first.0)
        .fetch_one(control_pool)
        .await
        .unwrap();
    assert_eq!(saved, name);
    assert_eq!(first.0.get_version_num(), 7);

    // Test-only deferred constraint: the RETURNING row precedes commit failure.
    sqlx::query("ALTER TABLE learners ADD CONSTRAINT test_name_unique UNIQUE (name) DEFERRABLE INITIALLY DEFERRED")
        .execute(control_pool).await.unwrap();
    let error = create(&storage, name, budget).await.unwrap_err();
    assert!(matches!(error, LearnerWriteError::NotCreated(_)));
    assert!(error.to_string().contains("SQLSTATE 23505"));
    assert!(!error.to_string().contains(name));
    assert_eq!(learner_count(control_pool, name).await, 1);
    sqlx::query("ALTER TABLE learners DROP CONSTRAINT test_name_unique")
        .execute(control_pool)
        .await
        .unwrap();
    let second = create(&storage, name, budget).await.unwrap();
    assert_ne!(first, second);
    assert_eq!(learner_count(control_pool, name).await, 2);

    sqlx::query("ALTER TABLE learners RENAME TO hidden_learners")
        .execute(control_pool)
        .await
        .unwrap();
    let error = create(&storage, "missing table", budget).await.unwrap_err();
    assert!(matches!(error, LearnerWriteError::NotCreated(_)));
    assert!(error.to_string().contains("SQLSTATE 42P01"));
    assert!(!error.to_string().contains("hidden_learners"));
    sqlx::query("ALTER TABLE hidden_learners RENAME TO learners")
        .execute(control_pool)
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
    assert_eq!(learner_count(control_pool, "not dispatched").await, 0);

    let lock = lock_table(control_pool, "learners", "ACCESS EXCLUSIVE").await;
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
}

async fn check_interrupted_write(pool: &PgPool, control_pool: &PgPool, cancel: bool) {
    let storage = PostgresLearnerStorage::new(pool.clone());
    let budget = Duration::from_secs(5);
    // Observe a blocked write before interrupting it, using a separate connection
    // while control_pool holds the table lock.
    let observer_pool = connect(control_pool.connect_options().as_ref().clone()).await;
    let backend = backend_pid(pool).await;
    let lock = lock_table(control_pool, "learners", "ACCESS EXCLUSIVE").await;
    let mut write = Box::pin(create(&storage, "interrupted", budget));
    let pids = [backend];
    tokio::select! {
        result = &mut write => panic!("locked write completed early: {result:?}"),
        () = blocked(&observer_pool, &pids) => {}
    }
    if cancel {
        drop(write);
    } else {
        sqlx::query("SELECT pg_terminate_backend($1, 1000)")
            .bind(backend)
            .execute(&observer_pool)
            .await
            .unwrap();
        let error = write.await.unwrap_err();
        assert!(matches!(error, LearnerWriteError::CompletionUnknown(_)));
    }
    let replacement = tokio::time::timeout(Duration::from_secs(1), backend_pid(pool))
        .await
        .unwrap();
    assert_ne!(replacement, backend);
    lock.rollback().await.unwrap();
    observer_pool.close().await;
}
