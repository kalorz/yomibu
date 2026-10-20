mod support;
use std::time::{Duration, Instant};
use support::{closed_storage, lock_table, recover, with_postgres};
use yomibu_core::ports::infrastructure::storage::{
    MAX_REQUEST_INPUT_BYTES, SharedCollection, SubjectQuery, validate_subject_lookup,
};
use yomibu_ext::{PostgresLearnerStorage, wanikani};

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

fn shared_collection(provider: &str, key: &str) -> SharedCollection {
    SharedCollection {
        provider: provider.into(),
        key: key.into(),
        name: "Supplied lessons".into(),
    }
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn lookup_is_grouped_read_only_and_collection_scoped() {
    let collection = wanikani::collection();
    with_postgres(async |pool, control| {
        let storage = PostgresLearnerStorage::new(pool.clone());
        assert!(storage.existing_subject_ids(&collection.provider, &collection.key, &["2".into(), "10".into()], deadline()).await.unwrap().is_empty());
        sqlx::raw_sql(
            "INSERT INTO collections (provider,key,name) VALUES ('wanikani','subjects','WaniKani'), ('manual','other','Other');
          INSERT INTO subjects (collection_id, external_id) SELECT id, '2' FROM collections;
          INSERT INTO subjects (collection_id, external_id) SELECT id, '10' FROM collections WHERE provider='wanikani';
          INSERT INTO subjects (collection_id, external_id) SELECT id, '3' FROM collections WHERE provider='manual';",
        )
        .execute(control)
        .await
        .unwrap();
        assert_eq!(storage.existing_subject_ids(&collection.provider, &collection.key, &["10".into(), "2".into(), "2".into(), "3".into(), "99".into()], deadline()).await.unwrap(), ["10", "2"]);
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM collections").fetch_one(control).await.unwrap(), 2);
        let ids: Vec<String> = (1..=1000).map(|id| id.to_string()).collect();
        assert_eq!(storage.existing_subject_ids(&collection.provider, &collection.key, &ids, deadline()).await.unwrap(), ["10", "2"]);
    })
    .await;
}

#[tokio::test]
async fn lookup_checks_contract_limits_before_acquisition() {
    let collection = wanikani::collection();
    let storage = closed_storage().await;
    assert!(
        storage
            .existing_subject_ids(&collection.provider, &collection.key, &[], deadline())
            .await
            .unwrap()
            .is_empty()
    );
    // Detailed count/byte cases live in Core; this checks adapter enforcement
    // before attempting to acquire the closed pool.
    let ids = ["x".repeat(MAX_REQUEST_INPUT_BYTES)];
    let error = storage
        .existing_subject_ids(&collection.provider, &collection.key, &ids, deadline())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("input limits"));
}

#[tokio::test]
async fn lookup_encoding_rejection_precedes_acquisition() {
    let collection = shared_collection("source", "lessons");
    let ids =
        ["x".repeat(MAX_REQUEST_INPUT_BYTES - collection.provider.len() - collection.key.len())];
    validate_subject_lookup(&collection.provider, &collection.key, &ids).unwrap();
    let error = closed_storage()
        .await
        .existing_subject_ids(&collection.provider, &collection.key, &ids, deadline())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("encoded argument limit"));
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn lookup_obeys_deadline() {
    let collection = wanikani::collection();
    with_postgres(async |pool, control| {
        let storage = PostgresLearnerStorage::new(pool.clone());
        let error = storage
            .existing_subject_ids(
                &collection.provider,
                &collection.key,
                &["1".into()],
                Instant::now(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("deadline"));
        let held = pool.acquire().await.unwrap();
        assert_lookup_deadline(&storage).await;
        drop(held);
        let lock = lock_table(control, "subjects", "ACCESS EXCLUSIVE").await;
        assert_lookup_deadline(&storage).await;
        recover(pool).await;
        lock.rollback().await.unwrap();
    })
    .await;
}

async fn assert_lookup_deadline(storage: &PostgresLearnerStorage) {
    let collection = wanikani::collection();
    let error = tokio::time::timeout(
        Duration::from_secs(1),
        storage.existing_subject_ids(
            &collection.provider,
            &collection.key,
            &["1".into()],
            Instant::now() + Duration::from_millis(50),
        ),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(error.to_string().contains("deadline"));
}
