use sqlx::postgres::PgPoolOptions;
use std::time::{Duration, Instant};
use yomibu_core::ports::infrastructure::wanikani::WaniKaniSubjectQuery;
use yomibu_ext::PostgresLearnerStorage;
mod support;
use support::with_postgres;

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn lookup_is_grouped_read_only_and_collection_scoped() {
    with_postgres(async |pool, control| {
        let storage = PostgresLearnerStorage::new(pool.clone());
        assert_eq!(storage.existing_subject_ids(&[2, 10], deadline()).await.unwrap(), []);
        sqlx::raw_sql("INSERT INTO collections (provider,key,name) VALUES ('wanikani','subjects','WaniKani'), ('manual','other','Other');
          INSERT INTO subjects (collection_id, external_id) SELECT id, '2' FROM collections;
          INSERT INTO subjects (collection_id, external_id) SELECT id, '10' FROM collections WHERE provider='wanikani';
          INSERT INTO subjects (collection_id, external_id) SELECT id, '3' FROM collections WHERE provider='manual';")
          .execute(control).await.unwrap();
        assert_eq!(storage.existing_subject_ids(&[10,2,2,3,99], deadline()).await.unwrap(), [2,10]);
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM collections").fetch_one(control).await.unwrap(), 2);
        let ids: Vec<u64> = (1..=1000).collect();
        assert_eq!(storage.existing_subject_ids(&ids, deadline()).await.unwrap(), [2,10]);
    }).await;
}

#[tokio::test]
async fn lookup_rejects_invalid_or_over_limit_input() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    pool.close().await;
    let storage = PostgresLearnerStorage::new(pool);
    assert!(
        storage
            .existing_subject_ids(&[], deadline())
            .await
            .unwrap()
            .is_empty()
    );
    for ids in [vec![0], vec![1; 1001]] {
        let error = storage
            .existing_subject_ids(&ids, deadline())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("invalid subject lookup"));
    }
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn lookup_obeys_deadline() {
    with_postgres(async |pool, control| {
        let storage = PostgresLearnerStorage::new(pool.clone());
        let error = storage
            .existing_subject_ids(&[1], Instant::now())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("deadline"));
        let held = pool.acquire().await.unwrap();
        assert_lookup_deadline(&storage).await;
        drop(held);
        let mut lock = control.begin().await.unwrap();
        sqlx::query("LOCK TABLE subjects IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *lock)
            .await
            .unwrap();
        assert_lookup_deadline(&storage).await;
        tokio::time::timeout(
            Duration::from_secs(7),
            sqlx::query("SELECT 1").execute(pool),
        )
        .await
        .unwrap()
        .unwrap();
        lock.rollback().await.unwrap();
    })
    .await;
}
async fn assert_lookup_deadline(storage: &PostgresLearnerStorage) {
    let error = tokio::time::timeout(
        Duration::from_secs(1),
        storage.existing_subject_ids(&[1], Instant::now() + Duration::from_millis(50)),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(error.to_string().contains("deadline"));
}
