#[path = "support/wanikani.rs"]
mod fixtures;
mod support;
use fixtures::{at, group, progress, seed, source, subject};
use std::time::{Duration, Instant};
use support::{
    another_session, backend_pid, blocked, closed_storage, lock_table, recover, snapshot,
    with_postgres,
};
use yomibu_core::{
    domain::{LearnerId, MaterialKind},
    ports::infrastructure::storage::{
        ImportWriteError, LearnerProgressImportWriter, LearnerProgressSourceIdentity,
        MAX_GROUP_RECORDS, MAX_REQUEST_INPUT_BYTES, PreparedProgress, PreparedSaveGroup,
        PreparedSubject, SharedCollection, SourceRecord, SubjectQuery, validate_subject_lookup,
    },
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

// Direct shared-contract construction for backend representation tests. Provider
// validation is exercised once in the integration's unit tests.
fn prepared(
    collection: SharedCollection,
    learner: u128,
    account: &str,
    source: SourceRecord,
    subjects: Vec<PreparedSubject>,
    progress: Vec<PreparedProgress>,
) -> PreparedSaveGroup {
    PreparedSaveGroup::new(
        collection,
        LearnerProgressSourceIdentity {
            learner_id: LearnerId(uuid::Uuid::from_u128(learner)),
            external_account_id: account.into(),
        },
        source,
        subjects,
        progress,
    )
    .unwrap()
}

fn shared_group(
    collection: SharedCollection,
    id: &str,
    t: i64,
    supply_subject: bool,
) -> PreparedSaveGroup {
    let subjects = if supply_subject {
        vec![PreparedSubject {
            external_id: id.into(),
            kind: Some(MaterialKind::Vocabulary),
            source: SourceRecord::new(at(t), r#"["opaque",{"content":"lesson"}]"#).unwrap(),
        }]
    } else {
        vec![]
    };
    prepared(
        collection,
        1,
        "account:learner/1",
        source(t, "account"),
        subjects,
        vec![PreparedProgress {
            subject_external_id: id.into(),
            ..progress(1, t, t + 5)
        }],
    )
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
async fn reference_resolution_encoding_is_bounded_before_acquisition() {
    let collection = shared_collection("source", "lessons");
    let payload = SourceRecord::new(at(10), "{}").unwrap();
    let fixed = collection.input_bytes() + 1 + 2 + MAX_GROUP_RECORDS * 2;
    let identity_bytes = MAX_REQUEST_INPUT_BYTES - fixed;
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
    let input = prepared(collection, 0, "a", payload, subjects, vec![]);
    let error = closed_storage()
        .await
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
async fn large_payloads_round_trip_across_byte_chunks() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let text = "食".repeat(300_000);
        let payload = serde_json::json!({"text": text}).to_string();
        let subjects = (1..=3)
            .map(|id| {
                let mut s = subject(id, 10);
                s.source = SourceRecord::new(at(10), &payload).unwrap();
                s
            })
            .collect();
        storage
            .save_group(&group(1, "a", 10, subjects, vec![]), deadline())
            .await
            .unwrap();
        let saved: Vec<String> = sqlx::query_scalar("SELECT source_data->>'text' FROM subjects")
            .fetch_all(control)
            .await
            .unwrap();
        assert_eq!(saved, vec![text; 3]);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn shared_storage_persists_grammar_kind() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let mut grammar = subject(1, 10);
        grammar.kind = Some(MaterialKind::Grammar);
        // The shared model permits grammar subjects; only WaniKani preparation rejects them.
        let input = prepared(
            shared_collection("lessons", "grammar"),
            1,
            "account",
            source(10, "account"),
            vec![grammar],
            vec![],
        );
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
        assert_eq!(storage.existing_subject_ids(&collection.provider, &collection.key, &[id.into()], deadline()).await.unwrap(), [id]);
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
            assert_eq!(storage.existing_subject_ids(&collections[index].provider, &collections[index].key, &requested, deadline()).await.unwrap(), [id]);
        }
        assert_eq!(storage.existing_subject_ids(&collections[1].provider, &collections[1].key, &requested, deadline()).await.unwrap(), [id, "only-beta"]);
        assert!(storage.existing_subject_ids("absent", "alpha", &requested, deadline()).await.unwrap().is_empty());

        let mut changed = shared_group(collections[0].clone(), id, 30, true).subjects()[0].clone();
        changed.kind = Some(MaterialKind::Grammar);
        let input = prepared(
            collections[0].clone(), 1, "account:learner/1", source(30, "changed account"),
            vec![changed], vec![PreparedProgress { subject_external_id: id.into(), ..progress(1, 30, 35) }],
        );
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
        assert_eq!(snapshot(control).await, before);
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
    let collection = wanikani::collection();
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        assert!(
            storage
                .existing_subject_ids(
                    &collection.provider,
                    &collection.key,
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
        for (corrupt, restore) in [("UPDATE learner_progress_sources SET source_data='{}'", "UPDATE learner_progress_sources SET source_data=jsonb_build_object('source_updated_at',to_timestamp(1700000010),'payload','{}'::jsonb)"), ("UPDATE learner_progress SET source_data='{\"source_updated_at\":\"sensitive-invalid-date\"}'", "UPDATE learner_progress SET source_data=jsonb_build_object('source_updated_at',to_timestamp(1700000010),'payload','{}'::jsonb)")] {
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
    let bad = SourceRecord::new(at(0) + chrono::Duration::nanoseconds(1), "{}").unwrap();
    let input = prepared(wanikani::collection(), 0, "a", bad, vec![], vec![]);
    let error = closed_storage()
        .await
        .save_group(&input, deadline())
        .await
        .unwrap_err();
    assert!(matches!(error, ImportWriteError::NotSaved(_)));
    assert!(error.to_string().contains("timestamp"));
}

#[tokio::test]
async fn encoding_preserves_payload_and_accounts_for_envelope_bytes() {
    let account = source(10, "a");
    let mut p = progress(1, 10, 20);
    let collection = wanikani::collection();
    let fixed = collection.input_bytes()
        + 1
        + p.subject_external_id.len()
        + account.payload().get().len()
        + p.source.payload().get().len();
    p.progress_stage = Some("x".repeat(MAX_REQUEST_INPUT_BYTES - fixed));
    let input = prepared(collection, 0, "a", account, vec![], vec![p]);
    let error = closed_storage()
        .await
        .save_group(&input, deadline())
        .await
        .unwrap_err();
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
        let other = another_session(pool).await;
        let observer = another_session(pool).await;
        let pids = [backend_pid(pool).await, backend_pid(&other).await];
        let first = PostgresLearnerStorage::new(pool.clone());
        let second = PostgresLearnerStorage::new(other.clone());
        for (repeated, unknown_freshness) in [(false, false), (true, false), (false, true)] {
            if unknown_freshness {
                sqlx::query("UPDATE subjects SET source_updated_at=NULL").execute(control).await.unwrap();
            }
            let lock = lock_table(control, if repeated { "learner_progress_sources" } else { "subjects" }, "SHARE").await;
            let left = group(1, "a", 30, vec![subject(2, 30), subject(1, 30)], vec![progress(2, 30, 20), progress(1, 30, 20)]);
            let right = group(if repeated { 1 } else { 2 }, if repeated { "a" } else { "b" }, 5, vec![subject(1, 5), subject(2, 5)], vec![progress(1, 5, 40), progress(2, 5, 40)]);
            let release = async {
                blocked(&observer, &pids).await;
                lock.commit().await.unwrap();
            };
            let (a, b, ()) = tokio::join!(first.save_group(&left, deadline()), second.save_group(&right, deadline()), release);
            a.unwrap();
            b.unwrap();
        }
        let counts: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM collections),(SELECT count(*) FROM subjects),(SELECT count(*) FROM learner_progress_sources)").fetch_one(control).await.unwrap();
        assert_eq!(counts, (1, 2, 2));
        type Row = (i64, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>);
        let rows: Vec<Row> = sqlx::query_as("SELECT (s.source_data->>'version')::bigint,s.source_updated_at,(p.source_data->>'source_updated_at')::timestamptz,p.recorded_at FROM learner_progress p JOIN subjects s ON s.id=p.subject_id JOIN learner_progress_sources a ON a.id=p.learner_progress_source_id WHERE a.external_account_id='a'").fetch_all(control).await.unwrap();
        assert_eq!(rows, vec![(30, at(30), at(30), at(40)); 2]);
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
        storage
            .save_group(&group(1, "a", 10, vec![subject(1, 10)], vec![]), deadline())
            .await
            .unwrap();
        let before = snapshot(control).await;
        for t in [10, 5] {
            let changed = prepared(
                wanikani::collection(),
                1,
                "a",
                source(t, "changed access"),
                vec![PreparedSubject {
                    external_id: "1".into(),
                    kind: None,
                    source: source(t, "changed subject"),
                }],
                vec![],
            );
            storage.save_group(&changed, deadline()).await.unwrap();
            assert_eq!(snapshot(control).await, before);
        }
    })
    .await;
}

#[tokio::test]
async fn every_timestamp_field_is_validated_before_acquisition() {
    let storage = closed_storage().await;
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
        let input = prepared(
            wanikani::collection(),
            0,
            "a",
            SourceRecord::new(timestamp, "{}").unwrap(),
            vec![],
            vec![],
        );
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
            let input = prepared(wanikani::collection(), 1, account, SourceRecord::new(time, "{}").unwrap(), vec![], vec![]);
            let receipt = storage.save_group(&input, deadline()).await.unwrap();
            // Replaying exercises a timestamp cast from the persisted envelope.
            assert_eq!(storage.save_group(&input, deadline()).await.unwrap(), receipt);
            let stored: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT (source_data->>'source_updated_at')::timestamptz FROM learner_progress_sources WHERE id=$1").bind(receipt.learner_progress_source_id.0).fetch_one(control).await.unwrap();
            assert_eq!(stored, time);
        }
    }).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn unknown_subject_freshness_initializes_without_changing_identity() {
    let collection = wanikani::collection();
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let original = storage.save_group(&group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 50)]), deadline()).await.unwrap();
        sqlx::query("UPDATE subjects SET source_updated_at=NULL").execute(control).await.unwrap();
        let before: (uuid::Uuid, String) = sqlx::query_as("SELECT id,to_jsonb(s)::text FROM subjects s").fetch_one(control).await.unwrap();
        assert_eq!(storage.existing_subject_ids(&collection.provider, &collection.key, &["1".into()], deadline()).await.unwrap(), vec!["1"]);
        let mut incoming = subject(1, 30);
        incoming.kind = None;
        let expected_payload: serde_json::Value = serde_json::from_str(incoming.source.payload().get()).unwrap();
        let saved = storage.save_group(&group(1, "a", 30, vec![incoming], vec![progress(1, 30, 40)]), deadline()).await.unwrap();
        assert_eq!(saved, original);
        let initialized: (uuid::Uuid, String) = sqlx::query_as("SELECT id,to_jsonb(s)::text FROM subjects s").fetch_one(control).await.unwrap();
        assert_eq!(initialized.0, before.0);
        let stored: serde_json::Value = serde_json::from_str(&initialized.1).unwrap();
        assert_eq!(stored["source_data"], expected_payload);
        assert!(stored["kind"].is_null());
        assert_eq!(sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>("SELECT source_updated_at FROM subjects").fetch_one(control).await.unwrap(), at(30));
        let row: (uuid::Uuid, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
            "SELECT p.subject_id,(p.source_data->>'source_updated_at')::timestamptz,
                (s.source_data->>'source_updated_at')::timestamptz,p.recorded_at
             FROM learner_progress p JOIN learner_progress_sources s ON s.id=p.learner_progress_source_id"
        ).fetch_one(control).await.unwrap();
        assert_eq!(row, (before.0, at(30), at(30), at(50)));
        // After initialization, equal/older input cannot replace any source fields.
        for time in [30, 10] {
            let mut changed = subject(1, time);
            changed.source = source(time, "different payload");
            storage.save_group(&group(1, "a", 30, vec![changed], vec![]), deadline()).await.unwrap();
            assert_eq!(sqlx::query_as::<_, (uuid::Uuid, String)>("SELECT id,to_jsonb(s)::text FROM subjects s").fetch_one(control).await.unwrap(), initialized);
        }
        // Reuse without resupplying the subject also remains valid.
        storage.save_group(&group(1, "a", 30, vec![], vec![progress(1, 30, 50)]), deadline()).await.unwrap();
        assert_eq!(sqlx::query_as::<_, (uuid::Uuid, String)>("SELECT id,to_jsonb(s)::text FROM subjects s").fetch_one(control).await.unwrap(), initialized);
        assert_eq!(sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>("SELECT recorded_at FROM learner_progress").fetch_one(control).await.unwrap(), at(50));
    }).await;
}
