#[path = "support/wanikani.rs"]
mod fixtures;
mod support;
use fixtures::{group, progress, seed, subject};
use sqlx::PgPool;
use std::time::{Duration, Instant};
use support::{another_session, backend_pid, blocked, lock_table, recover, with_postgres};
use yomibu_core::ports::infrastructure::storage::{ImportWriteError, LearnerProgressImportWriter};
use yomibu_ext::PostgresLearnerStorage;
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(8)
}
async fn empty(pool: &PgPool) {
    let counts: (i64, i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM collections),(SELECT count(*) FROM subjects),(SELECT count(*) FROM learner_progress_sources),(SELECT count(*) FROM learner_progress)").fetch_one(pool).await.unwrap();
    assert_eq!(counts, (0, 0, 0, 0));
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn expired_acquisition_and_lock_deadlines_save_nothing() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let input = group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]);
        assert!(matches!(
            storage.save_group(&input, Instant::now()).await,
            Err(ImportWriteError::NotSaved(_))
        ));
        let held = pool.acquire().await.unwrap();
        assert!(matches!(
            storage
                .save_group(&input, Instant::now() + Duration::from_millis(50))
                .await,
            Err(ImportWriteError::NotSaved(_))
        ));
        drop(held);
        let lock = lock_table(control, "subjects", "SHARE").await;
        let began = Instant::now();
        let error = storage
            .save_group(&input, began + Duration::from_millis(100))
            .await
            .unwrap_err();
        assert!(matches!(error, ImportWriteError::NotSaved(_)));
        assert!(began.elapsed() < Duration::from_secs(2));
        lock.rollback().await.unwrap();
        recover(pool).await;
        empty(control).await;
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn statements_and_chunks_share_one_deadline() {
    with_postgres(async |pool, control| {
        seed(control).await;
        sqlx::raw_sql("CREATE FUNCTION slow_chunk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(0.12); RETURN NULL; END $$; CREATE TRIGGER slow_chunk AFTER INSERT ON subjects FOR EACH STATEMENT EXECUTE FUNCTION slow_chunk();").execute(control).await.unwrap();
        let storage = PostgresLearnerStorage::new(pool.clone());
        let input = group(1, "a", 10, (1..=300).map(|id| subject(id, 10)).collect(), vec![]);
        let error = storage.save_group(&input, Instant::now() + Duration::from_millis(180)).await.unwrap_err();
        assert!(matches!(error, ImportWriteError::NotSaved(_)));
        recover(pool).await;
        empty(control).await;
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn server_deadline_replaces_an_inherited_longer_timeout() {
    with_postgres(async |pool, control| {
        seed(control).await;
        sqlx::raw_sql("SET transaction_timeout='5s'").execute(pool).await.unwrap();
        let observer = another_session(pool).await;
        let pid = backend_pid(pool).await;
        let pids = [pid];
        let lock = lock_table(control, "subjects", "SHARE").await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let input = group(1, "a", 10, vec![subject(1, 10)], vec![]);
        let mut save = storage.save_group(&input, Instant::now() + Duration::from_millis(300));
        tokio::select! {result=&mut save=>panic!("save completed before lock wait: {result:?}"),()=blocked(&observer,&pids)=>{}}
        // Stop polling the client: the server must enforce the budget itself.
        tokio::time::timeout(Duration::from_secs(2), async {
            while sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT FROM pg_stat_activity WHERE pid=$1)").bind(pid).fetch_one(&observer).await.unwrap() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("server retained the inherited transaction timeout");
        drop(save);
        lock.rollback().await.unwrap();
        recover(pool).await;
        empty(control).await;
        observer.close().await;
    }).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn cancellation_and_backend_loss_before_commit_recover_the_sole_slot() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let observer = another_session(pool).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let input = group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]);
        for terminate in [false, true] {
            let pids = [backend_pid(pool).await];
            let lock = lock_table(control, "subjects", "SHARE").await;
            let mut save = storage.save_group(&input, deadline());
            tokio::select! {result=&mut save=>panic!("save completed before fault: {result:?}"),()=blocked(&observer,&pids)=>{}}
            if terminate {
                assert!(sqlx::query_scalar::<_, bool>("SELECT pg_terminate_backend($1)").bind(pids[0]).fetch_one(&observer).await.unwrap());
                assert!(matches!(save.await, Err(ImportWriteError::NotSaved(_))));
            } else {
                drop(save);
            }
            lock.rollback().await.unwrap();
            recover(pool).await;
            empty(&observer).await;
        }
        observer.close().await;
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn deferred_commit_rejection_is_reported_conservatively() {
    with_postgres(async |pool, control| {
        seed(control).await;
        sqlx::raw_sql("CREATE FUNCTION reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'secret payload account diagnostic' USING ERRCODE='23514'; END $$; CREATE CONSTRAINT TRIGGER reject_commit AFTER INSERT ON subjects DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_commit();").execute(control).await.unwrap();
        let storage = PostgresLearnerStorage::new(pool.clone());
        let error = storage.save_group(&group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]), deadline()).await.unwrap_err();
        assert!(matches!(error, ImportWriteError::CompletionUnknown(_)));
        assert!(error.to_string().contains("23514"));
        assert!(!error.to_string().contains("secret"));
        recover(pool).await;
        empty(control).await;
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn commit_timeout_is_unknown_and_releases_the_connection() {
    with_postgres(async |pool, control| {
        seed(control).await;
        sqlx::raw_sql("CREATE FUNCTION slow_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(1); RETURN NEW; END $$; CREATE CONSTRAINT TRIGGER slow_commit AFTER INSERT ON subjects DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION slow_commit();").execute(control).await.unwrap();
        let storage = PostgresLearnerStorage::new(pool.clone());
        let input = group(1, "a", 10, vec![subject(1, 10)], vec![]);
        let began = Instant::now();
        let error = storage.save_group(&input, began + Duration::from_millis(200)).await.unwrap_err();
        assert!(matches!(error, ImportWriteError::CompletionUnknown(_)));
        assert!(began.elapsed() < Duration::from_secs(2));
        recover(pool).await;
        empty(control).await;
    }).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn actual_deadlock_rolls_back_without_retry() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        storage.save_group(&group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]), deadline()).await.unwrap();
        sqlx::raw_sql("CREATE SEQUENCE attempts; CREATE FUNCTION count_attempt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('attempts'); RETURN NEW; END $$; CREATE TRIGGER count_attempt BEFORE INSERT ON learner_progress_sources FOR EACH ROW EXECUTE FUNCTION count_attempt();").execute(control).await.unwrap();
        sqlx::query("SET deadlock_timeout='50ms'").execute(pool).await.unwrap();
        // Only the import may detect the cycle within the operation's deadline.
        sqlx::query("SET deadlock_timeout='10s'").execute(control).await.unwrap();
        let pid = backend_pid(pool).await;
        let control_pid = backend_pid(control).await;
        let observer = another_session(pool).await;
        let gate = backend_pid(&observer).await;
        sqlx::query("SELECT pg_advisory_lock($1::bigint)").bind(i64::from(gate)).execute(&observer).await.unwrap();
        // A test-only gate pauses the import after it owns the source row, but
        // before it starts waiting for the subject. The PID isolates the gate.
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE FUNCTION gate_subject() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock({gate}::bigint); RETURN NEW; END $$; CREATE TRIGGER gate_subject BEFORE INSERT ON subjects FOR EACH ROW EXECUTE FUNCTION gate_subject();"))).execute(control).await.unwrap();
        let mut lock = control.begin().await.unwrap();
        sqlx::query("SELECT id FROM subjects FOR UPDATE").execute(&mut *lock).await.unwrap();
        let input = group(1, "a", 30, vec![subject(1, 30)], vec![progress(1, 30, 40)]);
        let conflict = async {
            blocked(&observer, &[pid]).await;
            let update = async {
                sqlx::query("UPDATE learner_progress_sources SET source_data=source_data").execute(&mut *lock).await.unwrap();
                lock.commit().await.unwrap();
            };
            let release = async {
                blocked(&observer, &[control_pid]).await;
                // The control is already waiting on the import. Releasing the
                // gate starts the import's fresh subject wait with the cycle set.
                assert!(sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1::bigint)").bind(i64::from(gate)).fetch_one(&observer).await.unwrap());
            };
            tokio::join!(update, release);
        };
        let (result, ()) = tokio::join!(storage.save_group(&input, deadline()), conflict);
        let error = result.unwrap_err();
        assert!(matches!(error, ImportWriteError::NotSaved(_)));
        assert!(error.to_string().contains("40P01"));
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT last_value FROM attempts").fetch_one(&observer).await.unwrap(), 1);
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT (source_data->>'version')::bigint FROM subjects").fetch_one(&observer).await.unwrap(), 10);
        recover(pool).await;
        observer.close().await;
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn acknowledged_rollbacks_reuse_the_connection() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let pid = backend_pid(pool).await;
        // A contract rejection after writes must roll back, preserving the session.
        let input = group(1, "a", 10, vec![subject(1, 10)], vec![progress(2, 10, 20)]);
        assert!(matches!(
            storage.save_group(&input, deadline()).await,
            Err(ImportWriteError::NotSaved(_))
        ));
        empty(control).await;
        assert_eq!(backend_pid(pool).await, pid);
    })
    .await;
}
