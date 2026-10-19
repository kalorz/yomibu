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
        assert!(storage.existing_subject_ids(&[2, 10], deadline()).await.unwrap().is_empty());
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

#[path = "support/wanikani.rs"]
mod fixtures;
use fixtures::{at, group, progress, seed, source, subject};
use sqlx::PgPool;
use yomibu_core::ports::infrastructure::wanikani::{
    ImportWriteError, MAX_GROUP_INPUT_BYTES, PreparedSaveGroup, SourceRecord, WaniKaniGroupWriter,
};

async fn snapshot(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT jsonb_build_object('collections',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM collections c), 'sources',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM learner_progress_sources s),'subjects',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM subjects s),'progress',(SELECT jsonb_agg(to_jsonb(p) ORDER BY learner_progress_source_id,subject_id) FROM learner_progress p))::text").fetch_one(pool).await.unwrap()
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn first_save_and_replay_all_subject_types() {
    with_postgres(async |pool,control|{
    seed(control).await;
    let storage=PostgresLearnerStorage::new(pool.clone());
    let input=group(1,"a",10,(1..=4).map(|id|subject(id,10)).collect(),(1..=4).map(|id|progress(id,10,20)).collect());
    let first=storage.save_group(&input,deadline()).await.unwrap();
    let before=snapshot(control).await;
    assert_eq!(storage.save_group(&input,deadline()).await.unwrap(),first);
    assert_eq!(snapshot(control).await,before);
    let rows:Vec<(String,Option<String>,String)>=sqlx::query_as("SELECT external_id,kind::text,source_data::text FROM subjects ORDER BY external_id").fetch_all(control).await.unwrap();
    for (saved,wanted) in rows.iter().zip(input.subjects()) {
        assert_eq!(saved.0,wanted.external_id.to_string());
        assert_eq!(saved.1.as_deref(),if wanted.kind.is_some(){Some("vocabulary")}else{None});
        assert_eq!(serde_json::from_str::<serde_json::Value>(&saved.2).unwrap(),serde_json::from_str::<serde_json::Value>(wanted.source.payload().get()).unwrap());
    }
    let counts:(i64,i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM learners),(SELECT count(*) FROM entries),(SELECT count(*) FROM vocabulary_details),(SELECT count(*) FROM grammar_details),(SELECT count(*) FROM learner_progress WHERE is_marked_known IS NOT NULL)").fetch_one(control).await.unwrap();
    assert_eq!(counts,(2,0,0,0,0));
    assert!(sqlx::query_scalar::<_,bool>("SELECT last_import_completed_at IS NULL FROM learner_progress_sources").fetch_one(control).await.unwrap());
}).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn subjects_are_shared_progress_and_access_are_independent() {
    with_postgres(async |pool,control|{
    seed(control).await;let storage=PostgresLearnerStorage::new(pool.clone());
    for (learner,account) in [(1,"a"),(1,"b"),(2,"a")] {
        storage.save_group(&group(learner,account,10,vec![subject(1,10)],vec![progress(1,10,20)]),deadline()).await.unwrap();
    }
    let unrelated:String=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(s) ORDER BY id)::text FROM learner_progress_sources s WHERE NOT(learner_id='00000000-0000-0000-0000-000000000001' AND external_account_id='a')").fetch_one(control).await.unwrap();
    let mut newer=progress(1,30,40);newer.is_hidden=true;newer.progress_stage=Some("7".into());
    storage.save_group(&group(1,"a",30,vec![],vec![newer]),deadline()).await.unwrap();
    let counts:(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM subjects),(SELECT count(*) FROM learner_progress_sources),(SELECT count(*) FROM learner_progress),(SELECT count(*) FROM learner_progress WHERE is_hidden)").fetch_one(control).await.unwrap();assert_eq!(counts,(1,3,3,1));
    assert_eq!(sqlx::query_scalar::<_,String>("SELECT jsonb_agg(to_jsonb(s) ORDER BY id)::text FROM learner_progress_sources s WHERE NOT(learner_id='00000000-0000-0000-0000-000000000001' AND external_account_id='a')").fetch_one(control).await.unwrap(),unrelated);
}).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn reuse_subjects_and_resolve_a_concurrent_insert() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        assert!(
            storage
                .existing_subject_ids(&[1], deadline())
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
    with_postgres(async |pool,control|{
    seed(control).await;let storage=PostgresLearnerStorage::new(pool.clone());
    sqlx::raw_sql("INSERT INTO collections(provider,key,name) VALUES ('manual','elsewhere','Elsewhere'); INSERT INTO subjects(collection_id,external_id) SELECT id,'99' FROM collections;").execute(control).await.unwrap();
    for input in [group(999,"a",10,vec![subject(1,10)],vec![]),group(1,"a",10,vec![subject(1,10)],vec![progress(99,10,20)])] {
        let before=snapshot(control).await;
        assert!(matches!(storage.save_group(&input,deadline()).await,Err(ImportWriteError::NotSaved(_))));
        assert_eq!(snapshot(control).await,before);
    }
}).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn mixed_freshness_updates_each_record_independently() {
    with_postgres(async |pool,control|{
    seed(control).await;let storage=PostgresLearnerStorage::new(pool.clone());
    storage.save_group(&group(1,"a",10,vec![subject(1,10)],vec![progress(1,10,10)]),deadline()).await.unwrap();
    let mut older=progress(1,5,30);older.progress_stage=Some("1".into());
    storage.save_group(&group(1,"a",5,vec![subject(1,20)],vec![older]),deadline()).await.unwrap();
    let row:(String,i64,chrono::DateTime<chrono::Utc>)=sqlx::query_as("SELECT p.progress_stage,(s.source_data->>'version')::bigint,p.recorded_at FROM learner_progress p JOIN subjects s ON s.id=p.subject_id").fetch_one(control).await.unwrap();assert_eq!(row,("5".into(),20,at(30)));
    let mut reset=progress(1,20,20);reset.progress_stage=Some("1".into());reset.learning_started_at=None;reset.first_passed_at=None;reset.is_hidden=true;
    storage.save_group(&group(1,"a",20,vec![subject(1,5)],vec![reset]),deadline()).await.unwrap();
    let row:(String,bool,bool,bool,chrono::DateTime<chrono::Utc>,i64)=sqlx::query_as("SELECT p.progress_stage,p.learning_started_at IS NULL,p.first_passed_at IS NULL,p.is_hidden,p.recorded_at,(s.source_data->>'version')::bigint FROM learner_progress p JOIN subjects s ON s.id=p.subject_id").fetch_one(control).await.unwrap();assert_eq!(row,("1".into(),true,true,true,at(30),20));
}).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn observations_advance_without_source_replacement() {
    with_postgres(async |pool,control|{
    seed(control).await;let storage=PostgresLearnerStorage::new(pool.clone());
    storage.save_group(&group(1,"a",10,vec![subject(1,10),subject(2,10)],vec![progress(1,10,10),progress(2,10,10)]),deadline()).await.unwrap();
    for (source_time,observation) in [(10,20),(5,30),(5,30),(5,25)] {
        let mut supplied=progress(1,source_time,observation);supplied.source=source(source_time,"changed");supplied.progress_stage=None;
        storage.save_group(&group(1,"a",5,vec![],vec![supplied]),deadline()).await.unwrap();
    }
    let rows:Vec<(String,chrono::DateTime<chrono::Utc>,String)>=sqlx::query_as("SELECT p.progress_stage,p.recorded_at,p.source_data->'payload'->>'marker' FROM learner_progress p JOIN subjects s ON s.id=p.subject_id ORDER BY s.external_id").fetch_all(control).await.unwrap();
    assert_eq!(rows,vec![("5".into(),at(30),"progress".into()),("5".into(),at(10),"progress".into())]);
}).await;
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
    with_postgres(async |pool,control|{
    seed(control).await;let storage=PostgresLearnerStorage::new(pool.clone());
    let input=group(1,"a",10,vec![subject(1,10)],vec![progress(1,10,20)]);
    storage.save_group(&input,deadline()).await.unwrap();
    for (corrupt,restore) in [
      ("UPDATE subjects SET source_updated_at=NULL","UPDATE subjects SET source_updated_at=to_timestamp(1700000010)"),
      ("UPDATE learner_progress_sources SET source_data='{}'","UPDATE learner_progress_sources SET source_data=jsonb_build_object('source_updated_at',to_timestamp(1700000010),'payload','{}'::jsonb)"),
      ("UPDATE learner_progress SET source_data='{\"source_updated_at\":\"sensitive-invalid-date\"}'","UPDATE learner_progress SET source_data=jsonb_build_object('source_updated_at',to_timestamp(1700000010),'payload','{}'::jsonb)")
    ] {
      sqlx::raw_sql(corrupt).execute(control).await.unwrap();let before=snapshot(control).await;
      let error=storage.save_group(&group(1,"a",30,vec![subject(1,30),subject(2,30)],vec![progress(1,30,40)]),deadline()).await.unwrap_err();
      assert!(matches!(error,ImportWriteError::NotSaved(_)));assert!(!error.to_string().contains("sensitive"));assert_eq!(snapshot(control).await,before);
      sqlx::raw_sql(restore).execute(control).await.unwrap();
    }
}).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn failure_in_later_chunk_rolls_back_only_its_group() {
    with_postgres(async |pool,control|{
    seed(control).await;let storage=PostgresLearnerStorage::new(pool.clone());
    storage.save_group(&group(1,"a",10,vec![subject(1,10)],vec![progress(1,10,20)]),deadline()).await.unwrap();
    let before=snapshot(control).await;
    sqlx::raw_sql("CREATE FUNCTION reject_late_subject() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.external_id='300' THEN RAISE EXCEPTION 'sensitive-failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_late BEFORE INSERT ON subjects FOR EACH ROW EXECUTE FUNCTION reject_late_subject();").execute(control).await.unwrap();
    let input=group(1,"a",30,(2..=301).map(|id|subject(id,30)).collect(),vec![]);
    let error=storage.save_group(&input,deadline()).await.unwrap_err();assert!(matches!(error,ImportWriteError::NotSaved(_)));assert!(!error.to_string().contains("sensitive"));assert_eq!(snapshot(control).await,before);
}).await;
}

#[tokio::test]
async fn postgres_representation_checks_belong_to_the_adapter() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    pool.close().await;
    let storage = PostgresLearnerStorage::new(pool);
    let bad = SourceRecord::new(at(0) + chrono::Duration::nanoseconds(1), "{}").unwrap();
    let input = PreparedSaveGroup::new(
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
    let fixed = 1 + account.payload().get().len() + p.source.payload().get().len();
    p.progress_stage = Some("x".repeat(MAX_GROUP_INPUT_BYTES - fixed));
    let input = PreparedSaveGroup::new(
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
        let pids: Vec<i32> = vec![
            sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(pool).await.unwrap(),
            sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&other).await.unwrap(),
        ];
        let first = PostgresLearnerStorage::new(pool.clone());
        let second = PostgresLearnerStorage::new(other.clone());
        for repeated in [false, true] {
            let mut lock = control.begin().await.unwrap();
            sqlx::query(if repeated {"LOCK TABLE learner_progress_sources IN SHARE MODE"} else {"LOCK TABLE subjects IN SHARE MODE"}).execute(&mut *lock).await.unwrap();
            let left = group(1,"a",30,vec![subject(2,30),subject(1,30)],vec![progress(2,30,20),progress(1,30,20)]);
            let right = group(if repeated {1}else{2},if repeated {"a"}else{"b"},5,vec![subject(1,5),subject(2,5)],vec![progress(1,5,40),progress(2,5,40)]);
            let release = async {
                loop {
                    let blocked: i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE pid=ANY($1) AND wait_event_type='Lock'").bind(&pids).fetch_one(&observer).await.unwrap();
                    if blocked==2 { break; }
                    tokio::task::yield_now().await;
                }
                lock.commit().await.unwrap();
            };
            let (a,b,())=tokio::join!(first.save_group(&left,deadline()), second.save_group(&right,deadline()), release);
            a.unwrap();b.unwrap();
        }
        let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM collections),(SELECT count(*) FROM subjects),(SELECT count(*) FROM learner_progress_sources)").fetch_one(control).await.unwrap();
        assert_eq!(counts,(1,2,2));
        let rows:Vec<(i64,chrono::DateTime<chrono::Utc>,chrono::DateTime<chrono::Utc>)>=sqlx::query_as("SELECT (s.source_data->>'version')::bigint,(p.source_data->>'source_updated_at')::timestamptz,p.recorded_at FROM learner_progress p JOIN subjects s ON s.id=p.subject_id JOIN learner_progress_sources a ON a.id=p.learner_progress_source_id WHERE a.external_account_id='a'").fetch_all(control).await.unwrap();
        assert_eq!(rows, vec![(30,at(30),at(40));2]);
        other.close().await; observer.close().await;
    }).await;
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
            let changed = PreparedSaveGroup::new(
                first.learner_id(),
                "a".into(),
                source(t, "changed access"),
                vec![
                    yomibu_core::ports::infrastructure::wanikani::PreparedSubject {
                        external_id: 1,
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
