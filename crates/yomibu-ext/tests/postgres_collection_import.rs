use sqlx::postgres::PgPoolOptions;
use std::time::{Duration, Instant};
use yomibu_core::ports::infrastructure::collection_import::SubjectQuery;
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
        assert!(storage.existing_subject_ids(&yomibu_ext::wanikani::collection(), &["2".into(), "10".into()], deadline()).await.unwrap().is_empty());
        sqlx::raw_sql(
            "INSERT INTO collections (provider,key,name) VALUES ('wanikani','subjects','WaniKani'), ('manual','other','Other');
          INSERT INTO subjects (collection_id, external_id) SELECT id, '2' FROM collections;
          INSERT INTO subjects (collection_id, external_id) SELECT id, '10' FROM collections WHERE provider='wanikani';
          INSERT INTO subjects (collection_id, external_id) SELECT id, '3' FROM collections WHERE provider='manual';",
        )
        .execute(control)
        .await
        .unwrap();
        assert_eq!(storage.existing_subject_ids(&yomibu_ext::wanikani::collection(), &["10".into(), "2".into(), "2".into(), "3".into(), "99".into()], deadline()).await.unwrap(), ["10", "2"]);
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM collections").fetch_one(control).await.unwrap(), 2);
        let ids: Vec<String> = (1..=1000).map(|id| id.to_string()).collect();
        assert_eq!(storage.existing_subject_ids(&yomibu_ext::wanikani::collection(), &ids, deadline()).await.unwrap(), ["10", "2"]);
    })
    .await;
}

#[tokio::test]
async fn lookup_checks_contract_limits_before_acquisition() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    pool.close().await;
    let storage = PostgresLearnerStorage::new(pool);
    assert!(
        storage
            .existing_subject_ids(&yomibu_ext::wanikani::collection(), &[], deadline())
            .await
            .unwrap()
            .is_empty()
    );
    // Detailed count/byte cases live in Core; this checks adapter enforcement
    // before attempting to acquire the closed pool.
    let ids = ["x".repeat(MAX_GROUP_INPUT_BYTES)];
    let error = storage
        .existing_subject_ids(&yomibu_ext::wanikani::collection(), &ids, deadline())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("input limits"));
}

#[tokio::test]
async fn lookup_encoding_rejection_precedes_acquisition() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    pool.close().await;
    let collection = shared_collection("source", "lessons");
    let size = collection.provider.len() + collection.key.len() + collection.name.len();
    let ids = ["x".repeat(MAX_GROUP_INPUT_BYTES - size)];
    yomibu_core::ports::infrastructure::collection_import::validate_subject_lookup(
        &collection,
        &ids,
    )
    .unwrap();
    let error = PostgresLearnerStorage::new(pool)
        .existing_subject_ids(&collection, &ids, deadline())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("encoded argument limit"));
}

#[tokio::test]
async fn reference_resolution_encoding_is_bounded_before_acquisition() {
    use yomibu_core::ports::infrastructure::collection_import::{
        MAX_GROUP_RECORDS, PreparedSubject,
    };
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    pool.close().await;
    let collection = shared_collection("source", "lessons");
    let payload = SourceRecord::new(at(10), "{}").unwrap();
    let fixed = collection.provider.len()
        + collection.key.len()
        + collection.name.len()
        + 1
        + 2
        + MAX_GROUP_RECORDS * 2;
    let identity_bytes = MAX_GROUP_INPUT_BYTES - fixed;
    let subjects = (0..MAX_GROUP_RECORDS)
        .map(|i| {
            let size = identity_bytes / MAX_GROUP_RECORDS
                + usize::from(i < identity_bytes % MAX_GROUP_RECORDS);
            PreparedSubject {
                external_id: format!("{i:04}{}", "x".repeat(size - 4)),
                kind: None,
                source: payload.clone(),
            }
        })
        .collect();
    let input = yomibu_core::ports::infrastructure::collection_import::PreparedSaveGroup::new(
        collection,
        yomibu_core::ports::infrastructure::collection_import::ProgressSourceIdentity {
            learner_id: yomibu_core::domain::LearnerId(uuid::Uuid::nil()),
            external_account_id: "a".into(),
        },
        payload,
        subjects,
        vec![],
    )
    .unwrap();
    let error = PostgresLearnerStorage::new(pool)
        .save_group(&input, deadline())
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("encoded argument limit"),
        "{error}"
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn lookup_obeys_deadline() {
    with_postgres(async |pool, control| {
        let storage = PostgresLearnerStorage::new(pool.clone());
        let error = storage
            .existing_subject_ids(
                &yomibu_ext::wanikani::collection(),
                &["1".into()],
                Instant::now(),
            )
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
        storage.existing_subject_ids(
            &yomibu_ext::wanikani::collection(),
            &["1".into()],
            Instant::now() + Duration::from_millis(50),
        ),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(error.to_string().contains("deadline"));
}

#[path = "support/wanikani.rs"]
mod fixtures;
use fixtures::{at, group, progress, seed, source, subject};
use sqlx::PgPool;
use yomibu_core::ports::infrastructure::collection_import::{
    ImportGroupWriter, ImportWriteError, MAX_GROUP_INPUT_BYTES, SourceRecord,
};

async fn snapshot(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT jsonb_build_object('collections',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM collections c), 'sources',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM learner_progress_sources s),'subjects',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM subjects s),'progress',(SELECT jsonb_agg(to_jsonb(p) ORDER BY learner_progress_source_id,subject_id) FROM learner_progress p))::text").fetch_one(pool).await.unwrap()
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn first_save_and_replay_all_subject_types() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let input = group(1, "a", 10, (1..=4).map(|id| subject(id, 10)).collect(), (1..=4).map(|id| progress(id, 10, 20)).collect());
        let first = storage.save_group(&input, deadline()).await.unwrap();
        let before = snapshot(control).await;
        assert_eq!(storage.save_group(&input, deadline()).await.unwrap(), first);
        assert_eq!(snapshot(control).await, before);
        let rows: Vec<(String, Option<String>, String)> = sqlx::query_as("SELECT external_id,kind::text,source_data::text FROM subjects ORDER BY external_id").fetch_all(control).await.unwrap();
        for (saved, wanted) in rows.iter().zip(input.subjects()) {
            assert_eq!(saved.0, wanted.external_id.to_string());
            assert_eq!(saved.1.as_deref(), if wanted.kind.is_some() { Some("vocabulary") } else { None });
            assert_eq!(serde_json::from_str::<serde_json::Value>(&saved.2).unwrap(), serde_json::from_str::<serde_json::Value>(wanted.source.payload().get()).unwrap());
        }
        let counts: (i64, i64, i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM learners),(SELECT count(*) FROM entries),(SELECT count(*) FROM vocabulary_details),(SELECT count(*) FROM grammar_details),(SELECT count(*) FROM learner_progress WHERE is_marked_known IS NOT NULL)").fetch_one(control).await.unwrap();
        assert_eq!(counts, (2, 0, 0, 0, 0));
        assert!(sqlx::query_scalar::<_, bool>("SELECT last_import_completed_at IS NULL FROM learner_progress_sources").fetch_one(control).await.unwrap());
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn shared_storage_persists_grammar_kind() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let mut grammar = subject(1, 10);
        grammar.kind = Some(yomibu_core::domain::MaterialKind::Grammar);
        let input = yomibu_core::ports::infrastructure::collection_import::PreparedSaveGroup::new(
            shared_collection("lessons", "grammar"),
            yomibu_core::ports::infrastructure::collection_import::ProgressSourceIdentity {
                learner_id: yomibu_core::domain::LearnerId(uuid::Uuid::from_u128(1)),
                external_account_id: "account".into(),
            },
            source(10, "account"),
            vec![grammar],
            vec![],
        )
        .expect("the shared model permits grammar subjects");
        PostgresLearnerStorage::new(pool.clone())
            .save_group(&input, deadline())
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT kind::text FROM subjects")
                .fetch_one(control)
                .await
                .unwrap(),
            "grammar"
        );
    })
    .await;
}

fn shared_collection(
    provider: &str,
    key: &str,
) -> yomibu_core::ports::infrastructure::collection_import::SharedCollection {
    yomibu_core::ports::infrastructure::collection_import::SharedCollection {
        provider: provider.into(),
        key: key.into(),
        name: "Supplied lessons".into(),
    }
}

fn shared_group(
    collection: yomibu_core::ports::infrastructure::collection_import::SharedCollection,
    id: &str,
    t: i64,
    supply_subject: bool,
) -> yomibu_core::ports::infrastructure::collection_import::PreparedSaveGroup {
    use yomibu_core::ports::infrastructure::collection_import::{
        PreparedSubject, ProgressSourceIdentity,
    };
    let subjects = if supply_subject {
        vec![PreparedSubject {
            external_id: id.into(),
            kind: Some(yomibu_core::domain::MaterialKind::Vocabulary),
            source: SourceRecord::new(at(t), r#"["opaque",{"content":"lesson"}]"#).unwrap(),
        }]
    } else {
        vec![]
    };
    yomibu_core::ports::infrastructure::collection_import::PreparedSaveGroup::new(
        collection,
        ProgressSourceIdentity {
            learner_id: yomibu_core::domain::LearnerId(uuid::Uuid::from_u128(1)),
            external_account_id: "account:learner/1".into(),
        },
        source(t, "account"),
        subjects,
        vec![
            yomibu_core::ports::infrastructure::collection_import::PreparedProgress {
                subject_external_id: id.into(),
                ..progress(1, t, t + 5)
            },
        ],
    )
    .unwrap()
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn opaque_external_id_round_trips_and_reuses_the_stored_subject() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let collection = shared_collection("lessons", "alpha");
        let id = "lesson:alpha/7";
        let input = shared_group(collection.clone(), id, 10, true);
        let first = storage.save_group(&input, deadline()).await.unwrap();
        let row: (uuid::Uuid, String, String) = sqlx::query_as("SELECT id,external_id,source_data::text FROM subjects WHERE collection_id=$1")
            .bind(first.collection_id.0).fetch_one(control).await.unwrap();
        assert_eq!(row.1, id);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&row.2).unwrap(), serde_json::json!(["opaque",{"content":"lesson"}]));
        assert_eq!(storage.existing_subject_ids(&collection, &[id.into()], deadline()).await.unwrap(), [id]);
        assert_eq!(storage.save_group(&shared_group(collection, id, 20, false), deadline()).await.unwrap(), first);
        let references: Vec<(uuid::Uuid, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as("SELECT p.subject_id,s.external_id,p.recorded_at FROM learner_progress p JOIN subjects s ON s.id=p.subject_id WHERE p.learner_progress_source_id=$1")
            .bind(first.learner_progress_source_id.0).fetch_all(control).await.unwrap();
        assert_eq!(references, [(row.0, id.into(), at(25))]);
    }).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn identical_external_ids_are_isolated_by_supplied_collection() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        // Vary key and provider separately so ignoring either part is observable.
        let collections = [shared_collection("lessons", "alpha"), shared_collection("lessons", "beta"), shared_collection("other", "alpha")];
        let id = "lesson:alpha/7";
        let mut receipts = Vec::new();
        let mut subject_ids = Vec::new();
        for collection in &collections {
            let saved = storage.save_group(&shared_group(collection.clone(), id, 10, true), deadline()).await.unwrap();
            subject_ids.push(sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM subjects WHERE collection_id=$1 AND external_id=$2").bind(saved.collection_id.0).bind(id).fetch_one(control).await.unwrap());
            receipts.push(saved);
        }
        assert_eq!(subject_ids.iter().collect::<std::collections::HashSet<_>>().len(), 3);
        assert_eq!(receipts.iter().map(|r| r.learner_progress_source_id.0).collect::<std::collections::HashSet<_>>().len(), 3);
        storage.save_group(&shared_group(collections[1].clone(), "only-beta", 20, true), deadline()).await.unwrap();
        let requested = [id.into(), "only-beta".into()];
        for index in [0, 2] {
            assert_eq!(storage.existing_subject_ids(&collections[index], &requested, deadline()).await.unwrap(), [id]);
        }
        assert_eq!(storage.existing_subject_ids(&collections[1], &requested, deadline()).await.unwrap(), [id, "only-beta"]);
        assert!(storage.existing_subject_ids(&shared_collection("absent", "alpha"), &requested, deadline()).await.unwrap().is_empty());

        let mut changed = shared_group(collections[0].clone(), id, 30, true).subjects()[0].clone();
        changed.kind = Some(yomibu_core::domain::MaterialKind::Grammar);
        let input = yomibu_core::ports::infrastructure::collection_import::PreparedSaveGroup::new(
            collections[0].clone(), shared_group(collections[0].clone(), id, 30, false).progress_source().clone(),
            source(30, "changed account"), vec![changed], vec![yomibu_core::ports::infrastructure::collection_import::PreparedProgress { subject_external_id: id.into(), ..progress(1, 30, 35) }],
        ).unwrap();
        assert_eq!(storage.save_group(&input, deadline()).await.unwrap(), receipts[0]);
        for index in 0..3 {
            let row: (uuid::Uuid, String, chrono::DateTime<chrono::Utc>, uuid::Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as("SELECT s.id,s.kind::text,s.source_updated_at,p.subject_id,p.recorded_at FROM subjects s JOIN learner_progress p ON p.subject_id=s.id WHERE s.collection_id=$1 AND s.external_id=$2 AND p.learner_progress_source_id=$3")
                .bind(receipts[index].collection_id.0).bind(id).bind(receipts[index].learner_progress_source_id.0).fetch_one(control).await.unwrap();
            let (kind, updated, observed) = if index == 0 { ("grammar", 30, 35) } else { ("vocabulary", 10, 15) };
            assert_eq!(row, (subject_ids[index], kind.into(), at(updated), subject_ids[index], at(observed)));
        }
        let before = snapshot(control).await;
        let missing = shared_group(collections[0].clone(), "only-beta", 40, false);
        assert!(matches!(storage.save_group(&missing, deadline()).await, Err(ImportWriteError::NotSaved(_))));
        assert!(matches!(storage.record_completion(receipts[0].collection_id, receipts[1].learner_progress_source_id, at(50), deadline()).await, Err(ImportWriteError::NotSaved(_))));
        assert_eq!(snapshot(control).await, before);
        storage.record_completion(receipts[0].collection_id, receipts[0].learner_progress_source_id, at(50), deadline()).await.unwrap();
        let completed: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM learner_progress_sources WHERE last_import_completed_at IS NOT NULL").fetch_all(control).await.unwrap();
        assert_eq!(completed, [receipts[0].learner_progress_source_id.0]);
    }).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn subjects_are_shared_progress_and_access_are_independent() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        for (learner, account) in [(1, "a"), (1, "b"), (2, "a")] {
            storage.save_group(&group(learner, account, 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]), deadline()).await.unwrap();
        }
        let unrelated: String = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(s) ORDER BY id)::text FROM learner_progress_sources s WHERE NOT(learner_id='00000000-0000-0000-0000-000000000001' AND external_account_id='a')").fetch_one(control).await.unwrap();
        let mut newer = progress(1, 30, 40);
        newer.is_hidden = true;
        newer.progress_stage = Some("7".into());
        storage.save_group(&group(1, "a", 30, vec![], vec![newer]), deadline()).await.unwrap();
        let counts: (i64, i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM subjects),(SELECT count(*) FROM learner_progress_sources),(SELECT count(*) FROM learner_progress),(SELECT count(*) FROM learner_progress WHERE is_hidden)").fetch_one(control).await.unwrap();
        assert_eq!(counts, (1, 3, 3, 1));
        assert_eq!(sqlx::query_scalar::<_, String>("SELECT jsonb_agg(to_jsonb(s) ORDER BY id)::text FROM learner_progress_sources s WHERE NOT(learner_id='00000000-0000-0000-0000-000000000001' AND external_account_id='a')").fetch_one(control).await.unwrap(), unrelated);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn reuse_subjects_and_resolve_a_concurrent_insert() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        assert!(
            storage
                .existing_subject_ids(
                    &yomibu_ext::wanikani::collection(),
                    &["1".into()],
                    deadline()
                )
                .await
                .unwrap()
                .is_empty()
        );
        let other = PostgresLearnerStorage::new(control.clone());
        other
            .save_group(
                &group(2, "other", 10, vec![subject(1, 10)], vec![]),
                deadline(),
            )
            .await
            .unwrap();
        storage
            .save_group(
                &group(1, "a", 10, vec![], vec![progress(1, 10, 20)]),
                deadline(),
            )
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM subjects")
                .fetch_one(control)
                .await
                .unwrap(),
            1
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn unknown_learner_and_missing_subject_save_nothing() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        sqlx::raw_sql("INSERT INTO collections(provider,key,name) VALUES ('manual','elsewhere','Elsewhere'); INSERT INTO subjects(collection_id,external_id) SELECT id,'99' FROM collections;").execute(control).await.unwrap();
        for input in [group(999, "a", 10, vec![subject(1, 10)], vec![]), group(1, "a", 10, vec![subject(1, 10)], vec![progress(99, 10, 20)])] {
            let before = snapshot(control).await;
            assert!(matches!(storage.save_group(&input, deadline()).await, Err(ImportWriteError::NotSaved(_))));
            assert_eq!(snapshot(control).await, before);
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn mixed_freshness_updates_each_record_independently() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        storage.save_group(&group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 10)]), deadline()).await.unwrap();
        let mut older = progress(1, 5, 30);
        older.progress_stage = Some("1".into());
        storage.save_group(&group(1, "a", 5, vec![subject(1, 20)], vec![older]), deadline()).await.unwrap();
        let row: (String, i64, chrono::DateTime<chrono::Utc>) = sqlx::query_as("SELECT p.progress_stage,(s.source_data->>'version')::bigint,p.recorded_at FROM learner_progress p JOIN subjects s ON s.id=p.subject_id").fetch_one(control).await.unwrap();
        assert_eq!(row, ("5".into(), 20, at(30)));
        let mut reset = progress(1, 20, 20);
        reset.progress_stage = Some("1".into());
        reset.learning_started_at = None;
        reset.first_passed_at = None;
        reset.is_hidden = true;
        storage.save_group(&group(1, "a", 20, vec![subject(1, 5)], vec![reset]), deadline()).await.unwrap();
        let row: (String, bool, bool, bool, chrono::DateTime<chrono::Utc>, i64) = sqlx::query_as("SELECT p.progress_stage,p.learning_started_at IS NULL,p.first_passed_at IS NULL,p.is_hidden,p.recorded_at,(s.source_data->>'version')::bigint FROM learner_progress p JOIN subjects s ON s.id=p.subject_id").fetch_one(control).await.unwrap();
        assert_eq!(row, ("1".into(), true, true, true, at(30), 20));
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn observations_advance_without_source_replacement() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        storage.save_group(&group(1, "a", 10, vec![subject(1, 10), subject(2, 10)], vec![progress(1, 10, 10), progress(2, 10, 10)]), deadline()).await.unwrap();
        for (source_time, observation) in [(10, 20), (5, 30), (5, 30), (5, 25)] {
            let mut supplied = progress(1, source_time, observation);
            supplied.source = source(source_time, "changed");
            supplied.progress_stage = None;
            storage.save_group(&group(1, "a", 5, vec![], vec![supplied]), deadline()).await.unwrap();
        }
        let rows: Vec<(String, chrono::DateTime<chrono::Utc>, String)> = sqlx::query_as("SELECT p.progress_stage,p.recorded_at,p.source_data->'payload'->>'marker' FROM learner_progress p JOIN subjects s ON s.id=p.subject_id ORDER BY s.external_id").fetch_all(control).await.unwrap();
        assert_eq!(rows, vec![("5".into(), at(30), "progress".into()), ("5".into(), at(10), "progress".into())]);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn account_only_save_preserves_existing_progress() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        storage
            .save_group(
                &group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]),
                deadline(),
            )
            .await
            .unwrap();
        let before: String = sqlx::query_scalar("SELECT to_jsonb(p)::text FROM learner_progress p")
            .fetch_one(control)
            .await
            .unwrap();
        storage
            .save_group(&group(1, "a", 20, vec![], vec![]), deadline())
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT to_jsonb(p)::text FROM learner_progress p")
                .fetch_one(control)
                .await
                .unwrap(),
            before
        );
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn invalid_stored_freshness_rolls_back_the_group() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let input = group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]);
        storage.save_group(&input, deadline()).await.unwrap();
        for (corrupt, restore) in [("UPDATE subjects SET source_updated_at=NULL", "UPDATE subjects SET source_updated_at=to_timestamp(1700000010)"), ("UPDATE learner_progress_sources SET source_data='{}'", "UPDATE learner_progress_sources SET source_data=jsonb_build_object('source_updated_at',to_timestamp(1700000010),'payload','{}'::jsonb)"), ("UPDATE learner_progress SET source_data='{\"source_updated_at\":\"sensitive-invalid-date\"}'", "UPDATE learner_progress SET source_data=jsonb_build_object('source_updated_at',to_timestamp(1700000010),'payload','{}'::jsonb)")] {
            sqlx::raw_sql(corrupt).execute(control).await.unwrap();
            let before = snapshot(control).await;
            let error = storage.save_group(&group(1, "a", 30, vec![subject(1, 30), subject(2, 30)], vec![progress(1, 30, 40)]), deadline()).await.unwrap_err();
            assert!(matches!(error, ImportWriteError::NotSaved(_)));
            assert!(!error.to_string().contains("sensitive"));
            assert_eq!(snapshot(control).await, before);
            sqlx::raw_sql(restore).execute(control).await.unwrap();
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn failure_in_later_chunk_rolls_back_only_its_group() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        storage.save_group(&group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]), deadline()).await.unwrap();
        let before = snapshot(control).await;
        sqlx::raw_sql("CREATE FUNCTION reject_late_subject() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.external_id='99' THEN RAISE EXCEPTION 'sensitive-failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_late BEFORE INSERT ON subjects FOR EACH ROW EXECUTE FUNCTION reject_late_subject();").execute(control).await.unwrap();
        // Lexicographic ID ordering places 99 after the first 250-row chunk.
        let input = group(1, "a", 30, (2..=301).map(|id| subject(id, 30)).collect(), vec![]);
        let error = storage.save_group(&input, deadline()).await.unwrap_err();
        assert!(matches!(error, ImportWriteError::NotSaved(_)));
        assert!(!error.to_string().contains("sensitive"));
        assert_eq!(snapshot(control).await, before);
    })
    .await;
}

#[tokio::test]
async fn postgres_representation_checks_belong_to_the_adapter() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    pool.close().await;
    let storage = PostgresLearnerStorage::new(pool);
    let bad = SourceRecord::new(at(0) + chrono::Duration::nanoseconds(1), "{}").unwrap();
    let input = prepared_group(
        yomibu_core::domain::LearnerId(uuid::Uuid::nil()),
        "a".into(),
        bad,
        vec![],
        vec![],
    )
    .unwrap();
    let error = storage.save_group(&input, deadline()).await.unwrap_err();
    assert!(matches!(error, ImportWriteError::NotSaved(_)));
    assert!(error.to_string().contains("timestamp"));
}

#[tokio::test]
async fn encoding_preserves_payload_and_accounts_for_envelope_bytes() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    pool.close().await;
    let storage = PostgresLearnerStorage::new(pool);
    let account = source(10, "a");
    let mut p = progress(1, 10, 20);
    let collection = yomibu_ext::wanikani::collection();
    let fixed = collection.provider.len()
        + collection.key.len()
        + collection.name.len()
        + 1
        + p.subject_external_id.len()
        + account.payload().get().len()
        + p.source.payload().get().len();
    p.progress_stage = Some("x".repeat(MAX_GROUP_INPUT_BYTES - fixed));
    let input = prepared_group(
        yomibu_core::domain::LearnerId(uuid::Uuid::nil()),
        "a".into(),
        account,
        vec![],
        vec![p],
    )
    .unwrap();
    let error = storage.save_group(&input, deadline()).await.unwrap_err();
    assert!(error.to_string().contains("encoded"));
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn jsonb_rejection_rolls_back_preceding_writes() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let before = snapshot(control).await;
        let mut p = progress(1, 10, 20);
        p.source = SourceRecord::new(at(10), r#"{"text":"\u0000"}"#).unwrap();
        let error = storage
            .save_group(
                &group(1, "a", 10, vec![subject(1, 10)], vec![p]),
                deadline(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ImportWriteError::NotSaved(_)));
        assert_eq!(snapshot(control).await, before);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn controlled_overlapping_first_saves_and_reimports() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let other = support::connect((*pool.connect_options()).clone()).await;
        let observer = support::connect((*pool.connect_options()).clone()).await;
        let pids: Vec<i32> = vec![sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(pool).await.unwrap(), sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&other).await.unwrap()];
        let first = PostgresLearnerStorage::new(pool.clone());
        let second = PostgresLearnerStorage::new(other.clone());
        for repeated in [false, true] {
            let mut lock = control.begin().await.unwrap();
            sqlx::query(if repeated { "LOCK TABLE learner_progress_sources IN SHARE MODE" } else { "LOCK TABLE subjects IN SHARE MODE" }).execute(&mut *lock).await.unwrap();
            let left = group(1, "a", 30, vec![subject(2, 30), subject(1, 30)], vec![progress(2, 30, 20), progress(1, 30, 20)]);
            let right = group(if repeated { 1 } else { 2 }, if repeated { "a" } else { "b" }, 5, vec![subject(1, 5), subject(2, 5)], vec![progress(1, 5, 40), progress(2, 5, 40)]);
            let release = async {
                loop {
                    let blocked: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE pid=ANY($1) AND wait_event_type='Lock'").bind(&pids).fetch_one(&observer).await.unwrap();
                    if blocked == 2 {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
                lock.commit().await.unwrap();
            };
            let (a, b, ()) = tokio::join!(first.save_group(&left, deadline()), second.save_group(&right, deadline()), release);
            a.unwrap();
            b.unwrap();
        }
        let counts: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM collections),(SELECT count(*) FROM subjects),(SELECT count(*) FROM learner_progress_sources)").fetch_one(control).await.unwrap();
        assert_eq!(counts, (1, 2, 2));
        let rows: Vec<(i64, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as("SELECT (s.source_data->>'version')::bigint,(p.source_data->>'source_updated_at')::timestamptz,p.recorded_at FROM learner_progress p JOIN subjects s ON s.id=p.subject_id JOIN learner_progress_sources a ON a.id=p.learner_progress_source_id WHERE a.external_account_id='a'").fetch_all(control).await.unwrap();
        assert_eq!(rows, vec![(30, at(30), at(40)); 2]);
        other.close().await;
        observer.close().await;
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn equal_and_older_account_and_subject_payloads_do_not_replace() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let first = group(1, "a", 10, vec![subject(1, 10)], vec![]);
        storage.save_group(&first, deadline()).await.unwrap();
        let before = snapshot(control).await;
        for t in [10, 5] {
            let changed = prepared_group(
                first.progress_source().learner_id,
                "a".into(),
                source(t, "changed access"),
                vec![
                    yomibu_core::ports::infrastructure::collection_import::PreparedSubject {
                        external_id: "1".into(),
                        kind: None,
                        source: source(t, "changed subject"),
                    },
                ],
                vec![],
            )
            .unwrap();
            storage.save_group(&changed, deadline()).await.unwrap();
            assert_eq!(snapshot(control).await, before);
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn ordinary_saves_and_explicit_account_only_completion() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let saved = storage.save_group(&group(1, "a", 10, vec![], vec![]), deadline()).await.unwrap();
        let source = saved.learner_progress_source_id;
        assert!(sqlx::query_scalar::<_, bool>("SELECT last_import_completed_at IS NULL FROM learner_progress_sources").fetch_one(control).await.unwrap());
        let payload: String = sqlx::query_scalar("SELECT source_data::text FROM learner_progress_sources").fetch_one(control).await.unwrap();
        for time in [30, 20, 30, 40] {
            let receipt = storage.record_completion(saved.collection_id, source, at(time), deadline()).await.unwrap();
            assert_eq!(receipt.learner_progress_source_id, source);
            assert_eq!(receipt.last_import_completed_at, at(time.max(30)));
            assert_eq!(sqlx::query_scalar::<_, String>("SELECT source_data::text FROM learner_progress_sources").fetch_one(control).await.unwrap(), payload);
        }
        storage.save_group(&group(1, "a", 50, vec![subject(1, 50)], vec![progress(1, 50, 60)]), deadline()).await.unwrap();
        assert_eq!(sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>("SELECT last_import_completed_at FROM learner_progress_sources").fetch_one(control).await.unwrap(), at(40));
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND column_name='last_import_started_at'").fetch_one(control).await.unwrap(), 0);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn completion_rejects_unknown_mismatched_and_out_of_scope_sources() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let saved = storage.save_group(&group(1, "a", 10, vec![], vec![]), deadline()).await.unwrap();
        let private: uuid::Uuid = sqlx::query_scalar("INSERT INTO collections(provider,key,name,owner_learner_id) VALUES('manual','private','Private',$1) RETURNING id").bind(uuid::Uuid::from_u128(1)).fetch_one(control).await.unwrap();
        let private_source: uuid::Uuid = sqlx::query_scalar("INSERT INTO learner_progress_sources(learner_id,collection_id,external_account_id) VALUES($1,$2,'external') RETURNING id").bind(uuid::Uuid::from_u128(1)).bind(private).fetch_one(control).await.unwrap();
        let manual_source: uuid::Uuid = sqlx::query_scalar("INSERT INTO learner_progress_sources(learner_id,collection_id) VALUES($1,$2) RETURNING id").bind(uuid::Uuid::from_u128(1)).bind(saved.collection_id.0).fetch_one(control).await.unwrap();
        let before = snapshot(control).await;
        for (collection, source) in [(private, private_source), (saved.collection_id.0, manual_source), (private, saved.learner_progress_source_id.0), (saved.collection_id.0, uuid::Uuid::nil())] {
            assert!(matches!(storage.record_completion(yomibu_core::domain::CollectionId(collection), yomibu_core::domain::LearnerProgressSourceId(source), at(20), deadline()).await, Err(ImportWriteError::NotSaved(_))));
            assert_eq!(snapshot(control).await, before);
        }
    }).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn overlapping_completions_do_not_regress() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let saved = storage.save_group(&group(1, "a", 10, vec![], vec![]), deadline()).await.unwrap();
        let source = saved.learner_progress_source_id;
        let other = support::connect((*pool.connect_options()).clone()).await;
        let observer = support::connect((*pool.connect_options()).clone()).await;
        let second = PostgresLearnerStorage::new(other.clone());
        let pids: Vec<i32> = vec![sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(pool).await.unwrap(), sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&other).await.unwrap()];
        let mut lock = control.begin().await.unwrap();
        sqlx::query("SELECT id FROM learner_progress_sources FOR UPDATE").execute(&mut *lock).await.unwrap();
        let release = async {
            loop {
                let count: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE pid=ANY($1) AND wait_event_type='Lock'").bind(&pids).fetch_one(&observer).await.unwrap();
                if count == 2 {
                    break;
                }
                tokio::task::yield_now().await;
            }
            lock.commit().await.unwrap();
        };
        let (new, old, ()) = tokio::join!(storage.record_completion(saved.collection_id, source, at(40), deadline()), second.record_completion(saved.collection_id, source, at(20), deadline()), release);
        assert_eq!(new.unwrap().last_import_completed_at, at(40));
        assert!(old.unwrap().last_import_completed_at >= at(20));
        assert_eq!(sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>("SELECT last_import_completed_at FROM learner_progress_sources").fetch_one(control).await.unwrap(), at(40));
        other.close().await;
        observer.close().await;
    })
    .await;
}

#[tokio::test]
async fn completion_representation_rejection_precedes_acquisition() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    pool.close().await;
    let storage = PostgresLearnerStorage::new(pool);
    let error = storage
        .record_completion(
            yomibu_core::domain::CollectionId(uuid::Uuid::nil()),
            yomibu_core::domain::LearnerProgressSourceId(uuid::Uuid::nil()),
            at(0) + chrono::Duration::nanoseconds(1),
            deadline(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, ImportWriteError::NotSaved(_)));
    assert!(error.to_string().contains("timestamp"));
}

#[tokio::test]
async fn every_timestamp_field_is_validated_before_acquisition() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    pool.close().await;
    let storage = PostgresLearnerStorage::new(pool);
    let submicro = at(10) + chrono::Duration::nanoseconds(1);
    for field in 0..5 {
        let mut s = subject(1, 10);
        let mut p = progress(1, 10, 20);
        match field {
            0 => s.source = SourceRecord::new(submicro, "{}").unwrap(),
            1 => p.source = SourceRecord::new(submicro, "{}").unwrap(),
            2 => p.observed_at = submicro,
            3 => p.learning_started_at = Some(submicro),
            _ => p.first_passed_at = Some(submicro),
        }
        let error = storage
            .save_group(&group(1, "a", 10, vec![s], vec![p]), deadline())
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("timestamp"),
            "field {field}: {error}"
        );
    }
    for timestamp in [
        chrono::DateTime::<chrono::Utc>::MIN_UTC,
        chrono::DateTime::from_timestamp(1_700_000_039, 1_000_000_000).unwrap(),
    ] {
        let input = prepared_group(
            yomibu_core::domain::LearnerId(uuid::Uuid::nil()),
            "a".into(),
            SourceRecord::new(timestamp, "{}").unwrap(),
            vec![],
            vec![],
        )
        .unwrap();
        assert!(
            storage
                .save_group(&input, deadline())
                .await
                .unwrap_err()
                .to_string()
                .contains("timestamp")
        );
    }
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn supported_timestamp_endpoints_round_trip_without_loss() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let epoch = chrono::DateTime::from_timestamp(946_684_800, 0).unwrap();
        let lower = epoch - chrono::Duration::microseconds(211_813_488_000_000_000);
        let upper = chrono::DateTime::<chrono::Utc>::MAX_UTC - chrono::Duration::nanoseconds(999);
        for (account, time) in [("lower", lower), ("upper", upper)] {
            let input = prepared_group(yomibu_core::domain::LearnerId(uuid::Uuid::from_u128(1)), account.into(), SourceRecord::new(time, "{}").unwrap(), vec![], vec![]).unwrap();
            let receipt = storage.save_group(&input, deadline()).await.unwrap();
            // Replaying exercises a timestamp cast from the persisted envelope.
            assert_eq!(storage.save_group(&input, deadline()).await.unwrap(), receipt);
            let stored: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT (source_data->>'source_updated_at')::timestamptz FROM learner_progress_sources WHERE id=$1").bind(receipt.learner_progress_source_id.0).fetch_one(control).await.unwrap();
            assert_eq!(stored, time);
            assert_eq!(storage.record_completion(receipt.collection_id, receipt.learner_progress_source_id, time, deadline()).await.unwrap().last_import_completed_at, time);
        }
    }).await;
}

// Direct shared-contract construction for backend representation tests. Provider
// validation is exercised once in the integration's unit tests.
fn prepared_group(
    learner: yomibu_core::domain::LearnerId,
    account: String,
    source: SourceRecord,
    subjects: Vec<yomibu_core::ports::infrastructure::collection_import::PreparedSubject>,
    progress: Vec<yomibu_core::ports::infrastructure::collection_import::PreparedProgress>,
) -> Result<
    yomibu_core::ports::infrastructure::collection_import::PreparedSaveGroup,
    yomibu_core::ports::infrastructure::collection_import::InvalidSaveGroup,
> {
    yomibu_core::ports::infrastructure::collection_import::PreparedSaveGroup::new(
        yomibu_ext::wanikani::collection(),
        yomibu_core::ports::infrastructure::collection_import::ProgressSourceIdentity {
            learner_id: learner,
            external_account_id: account,
        },
        source,
        subjects,
        progress,
    )
}
