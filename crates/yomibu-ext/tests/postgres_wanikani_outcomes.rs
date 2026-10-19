#[path = "support/postgres_faults.rs"]
mod faults;
#[path = "support/wanikani.rs"]
mod fixtures;
mod support;
use faults::{Fault, Relay};
use fixtures::{at, group, progress, seed, subject};
use sqlx::PgPool;
use std::time::{Duration, Instant};
use support::{
    another_session, backend_pid, blocked, connect, lock_table, recover, snapshot, with_postgres,
};
use yomibu_core::{
    domain::LearnerProgressSourceId,
    ports::infrastructure::collection_import::{ImportWriteError, SaveGroupWriter},
};
use yomibu_ext::PostgresLearnerStorage;
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(8)
}
async fn empty(pool: &PgPool) {
    let counts: (i64, i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM collections),(SELECT count(*) FROM subjects),(SELECT count(*) FROM learner_progress_sources),(SELECT count(*) FROM learner_progress)").fetch_one(pool).await.unwrap();
    assert_eq!(counts, (0, 0, 0, 0));
}
/// Rows a completion must never touch.
async fn progress_and_subjects(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT jsonb_build_object('progress',(SELECT to_jsonb(p) FROM learner_progress p),'subject',(SELECT to_jsonb(s) FROM subjects s))::text").fetch_one(pool).await.unwrap()
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
async fn deferred_commit_rejection_is_confirmed_not_saved() {
    with_postgres(async |pool, control| {
        seed(control).await;
        sqlx::raw_sql("CREATE FUNCTION reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'secret payload account diagnostic' USING ERRCODE='23514'; END $$; CREATE CONSTRAINT TRIGGER reject_commit AFTER INSERT ON subjects DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_commit();").execute(control).await.unwrap();
        let storage = PostgresLearnerStorage::new(pool.clone());
        let error = storage.save_group(&group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]), deadline()).await.unwrap_err();
        assert!(matches!(error, ImportWriteError::NotSaved(_)));
        assert!(error.to_string().contains("23514"));
        assert!(!error.to_string().contains("secret"));
        recover(pool).await;
        empty(control).await;
    })
    .await;
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
async fn lost_commit_response_and_lost_request_are_both_unknown() {
    with_postgres(async |pool, control| {
        seed(control).await;
        for (index, fault) in [
            Fault::DropCommitRequest,
            Fault::DropCommitResponse,
            Fault::Pass,
        ]
        .into_iter()
        .enumerate()
        {
            let relay = Relay::start(&pool.connect_options(), fault).await;
            let relayed = connect(relay.options.clone()).await;
            relay.arm();
            let storage = PostgresLearnerStorage::new(relayed.clone());
            let input = group(
                1,
                &format!("account-{index}"),
                10,
                vec![subject(index as u64 + 1, 10)],
                vec![progress(index as u64 + 1, 10, 20)],
            );
            let result = storage.save_group(&input, deadline()).await;
            relay.reached().await;
            if fault == Fault::Pass {
                result.unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(ImportWriteError::CompletionUnknown(_))
                ));
            }
            let durable: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT FROM learner_progress_sources WHERE external_account_id=$1)",
            )
            .bind(&input.learner_progress_source().external_account_id)
            .fetch_one(control)
            .await
            .unwrap();
            assert_eq!(durable, fault != Fault::DropCommitRequest);
            let stats = relay.counters().sample();
            assert!(stats.commands > 5);
            assert!(stats.parses > 0);
            assert!(stats.exchanges > 0);
            assert!(stats.bind_bytes > 0);
            assert!(stats.max_bind_bytes < 8 * 1024 * 1024);
            recover(&relayed).await;
            relayed.close().await;
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn confirmed_commit_observed_after_deadline_retains_receipt() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let relay = Relay::start(&pool.connect_options(), Fault::GateCommitResponse).await;
        let relayed = connect(relay.options.clone()).await;
        relay.arm();
        let storage = PostgresLearnerStorage::new(relayed.clone());
        let input = group(1, "late", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]);
        let end = Instant::now() + Duration::from_millis(300);
        let mut save = storage.save_group(&input, end);
        tokio::select! {result=&mut save=>panic!("unexpected early result: {result:?}"),()=relay.reached()=>{}}
        relay.release();
        relay.delivered().await;
        // Do not poll the caller while the confirmed backend response arrives. The
        // next poll happens after expiry, with the real C/Z frames already readable.
        tokio::time::sleep_until((end + Duration::from_millis(20)).into()).await;
        let receipt = save.await.expect("observed COMMIT success must win over late timer");
        assert_eq!(sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM learner_progress_sources").fetch_one(control).await.unwrap(), receipt.learner_progress_source_id.0);
        relayed.close().await;
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn deadline_after_commit_dispatch_is_unknown_even_with_durable_rows() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let relay = Relay::start(&pool.connect_options(), Fault::GateCommitResponse).await;
        let relayed = connect(relay.options.clone()).await;
        relay.arm();
        let storage = PostgresLearnerStorage::new(relayed.clone());
        let input = group(1, "timeout", 10, vec![subject(1, 10)], vec![]);
        let end = Instant::now() + Duration::from_millis(200);
        let mut save = storage.save_group(&input, end);
        tokio::select! {result=&mut save=>panic!("unexpected early result: {result:?}"),()=relay.reached()=>{}}
        assert!(matches!(save.await, Err(ImportWriteError::CompletionUnknown(_))));
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM learner_progress_sources").fetch_one(control).await.unwrap(), 1);
        relay.release();
        recover(&relayed).await;
        relayed.close().await;
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn completion_failure_preserves_prior_groups_and_completion() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let saved = storage.save_group(&group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]), deadline()).await.unwrap();
        let source = saved.learner_progress_source_id;
        storage.record_completion(saved.collection_id, source, at(20), deadline()).await.unwrap();
        let before = snapshot(control).await;
        let mut lock = control.begin().await.unwrap();
        sqlx::query("SELECT id FROM learner_progress_sources FOR UPDATE").execute(&mut *lock).await.unwrap();
        assert!(matches!(storage.record_completion(saved.collection_id, source, at(30), Instant::now() + Duration::from_millis(50)).await, Err(ImportWriteError::NotSaved(_))));
        lock.rollback().await.unwrap();
        recover(pool).await;
        assert_eq!(snapshot(control).await, before);
        sqlx::raw_sql("CREATE FUNCTION reject_completion() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private completion error' USING ERRCODE='23514'; END $$; CREATE CONSTRAINT TRIGGER reject_completion AFTER UPDATE ON learner_progress_sources DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_completion();").execute(control).await.unwrap();
        let error = storage.record_completion(saved.collection_id, source, at(30), deadline()).await.unwrap_err();
        assert!(matches!(error, ImportWriteError::NotSaved(_)));
        assert!(!error.to_string().contains("private"));
        assert_eq!(snapshot(control).await, before);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn completion_unknown_and_late_confirmation_use_the_shared_lifecycle() {
    with_postgres(async |pool, control| {
        seed(control).await;
        let storage = PostgresLearnerStorage::new(pool.clone());
        let saved = storage.save_group(&group(1, "a", 10, vec![subject(1, 10)], vec![progress(1, 10, 20)]), deadline()).await.unwrap();
        let source = saved.learner_progress_source_id;
        let before = progress_and_subjects(control).await;
        for (index, fault) in [Fault::DropCommitRequest, Fault::DropCommitResponse, Fault::GateCommitResponse].into_iter().enumerate() {
            let relay = Relay::start(&pool.connect_options(), fault).await;
            let relayed = connect(relay.options.clone()).await;
            let writer = PostgresLearnerStorage::new(relayed.clone());
            relay.arm();
            let completed_at = at(30 + index as i64);
            let end = Instant::now() + Duration::from_millis(300);
            if fault == Fault::GateCommitResponse {
                let mut completion = writer.record_completion(saved.collection_id, source, completed_at, end);
                tokio::select! {result=&mut completion=>panic!("unexpected early result: {result:?}"),()=relay.reached()=>{}}
                relay.release();
                relay.delivered().await;
                tokio::time::sleep_until((end + Duration::from_millis(20)).into()).await;
                assert_eq!(completion.await.unwrap().last_import_completed_at, completed_at);
            } else {
                assert!(matches!(writer.record_completion(saved.collection_id, source, completed_at, end).await, Err(ImportWriteError::CompletionUnknown(_))));
                relay.reached().await;
            }
            let actual: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar("SELECT last_import_completed_at FROM learner_progress_sources").fetch_one(control).await.unwrap();
            assert_eq!(actual, if fault == Fault::DropCommitRequest { None } else { Some(completed_at) });
            assert_eq!(progress_and_subjects(control).await, before);
            recover(&relayed).await;
            relayed.close().await;
        }
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
        let receipt = storage
            .save_group(&group(1, "a", 10, vec![], vec![]), deadline())
            .await
            .unwrap();
        assert!(matches!(
            storage
                .record_completion(
                    receipt.collection_id,
                    LearnerProgressSourceId(uuid::Uuid::nil()),
                    at(20),
                    deadline()
                )
                .await,
            Err(ImportWriteError::NotSaved(_))
        ));
        assert_eq!(backend_pid(pool).await, pid);
        assert!(
            sqlx::query_scalar::<_, bool>(
                "SELECT last_import_completed_at IS NULL FROM learner_progress_sources"
            )
            .fetch_one(control)
            .await
            .unwrap()
        );
    })
    .await;
}
