#[path = "support/wanikani.rs"]
mod fixtures;
mod support;
// This target uses the same measured relay; fault modes are exercised separately.
#[allow(dead_code)]
#[path = "support/postgres_faults.rs"]
mod faults;
use faults::{Counters, Fault, Relay};
use fixtures::{at, group, progress, seed, subject};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use yomibu_core::ports::infrastructure::collection_import::{
    MAX_REQUEST_INPUT_BYTES, PreparedSaveGroup, SaveGroupWriter, SourceRecord, SubjectQuery,
};
use yomibu_ext::PostgresLearnerStorage;
const GROUP_ROWS: usize = 500;
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(15)
}

// This bounded fixture generator never retains the inventory. Payload sizes vary
// independently of the four provider types, including multibyte text and arrays.
fn generated(
    start: u64,
    count: usize,
    learner: u128,
    account: &str,
    t: i64,
    observation: i64,
    include_subjects: bool,
) -> PreparedSaveGroup {
    let subjects = if include_subjects {
        (start..start + count as u64)
            .map(|id| synthetic_subject(id, t))
            .collect()
    } else {
        vec![]
    };
    let progress = (start..start + count as u64)
        .map(|id| {
            let mut p = progress(id, t, observation);
            p.source = SourceRecord::new(at(t), &serde_json::json!({"id":id,"object":"assignment","data_updated_at":"2023-11-14T22:13:30Z","data":{"subject_id":id,"srs_stage":5,"hidden":false,"started_at":"2023-11-14T22:13:21Z","passed_at":"2023-11-14T22:13:22Z","burned_at":null,"resurrected_at":null,"available_at":null,"created_at":"2023-11-14T22:13:20Z","unlocked_at":"2023-11-14T22:13:20Z"},"access":{"excluded":false,"reasons":[]}}).to_string()).unwrap();
            p
        })
        .collect();
    group(learner, account, t, subjects, progress)
}
fn synthetic_subject(
    id: u64,
    t: i64,
) -> yomibu_core::ports::infrastructure::collection_import::PreparedSubject {
    let mut s = subject(id, t);
    let repeats = if id.is_multiple_of(97) {
        1024
    } else {
        [1, 4, 16, 64][(id as usize / 4) % 4]
    };
    let text = "説明・reading ".repeat(repeats);
    let mut value: serde_json::Value = serde_json::from_str(s.source.payload().get()).unwrap();
    value["data"]["meaning_mnemonic"] = text.clone().into();
    value["data"]["reading_mnemonic"] = text.into();
    value["data"]["auxiliary_meanings"] = serde_json::json!([{"meaning":"synthetic long meaning","type":"whitelist"},{"meaning":"別の意味","type":"blacklist"}]);
    value["data"]["component_subject_ids"] = serde_json::json!([1, 2, 3]);
    s.source = SourceRecord::new(at(t), &value.to_string()).unwrap();
    s
}
fn raw_bytes(input: &PreparedSaveGroup) -> usize {
    input.collection().provider.len()
        + input.collection().key.len()
        + input.collection().name.len()
        + input.learner_progress_source().external_account_id.len()
        + input.account().payload().get().len()
        + input
            .subjects()
            .iter()
            .map(|s| s.external_id.len() + s.source.payload().get().len())
            .sum::<usize>()
        + input
            .progress()
            .iter()
            .map(|p| {
                p.subject_external_id.len()
                    + p.source.payload().get().len()
                    + p.progress_stage.as_ref().map_or(0, String::len)
            })
            .sum::<usize>()
}
#[derive(Default)]
struct Run {
    groups: usize,
    input_bytes: usize,
    max_group_bytes: usize,
    max_subjects: usize,
    max_progress: usize,
    payload_sizes: Vec<usize>,
    elapsed_ms: u128,
}
impl Run {
    fn observe(&mut self, input: &PreparedSaveGroup) {
        let bytes = raw_bytes(input);
        self.input_bytes += bytes;
        self.max_group_bytes = self.max_group_bytes.max(bytes);
        self.max_subjects = self.max_subjects.max(input.subjects().len());
        self.max_progress = self.max_progress.max(input.progress().len());
        self.groups += 1;
        for size in input
            .subjects()
            .iter()
            .map(|s| s.source.payload().get().len())
            .chain(
                input
                    .progress()
                    .iter()
                    .map(|p| p.source.payload().get().len()),
            )
        {
            if self.payload_sizes.len() < 2000 {
                self.payload_sizes.push(size);
            }
        }
    }
}
async fn import(
    storage: &PostgresLearnerStorage,
    total: usize,
    learner: u128,
    account: &str,
    t: i64,
    observation: i64,
    include_subjects: bool,
) -> Run {
    let began = Instant::now();
    let mut run = Run::default();
    let mut saved = None;
    for offset in (0..total).step_by(GROUP_ROWS) {
        let count = GROUP_ROWS.min(total - offset);
        if !include_subjects {
            let mut ids: Vec<String> = (offset + 1..=offset + count)
                .map(|id| id.to_string())
                .collect();
            ids.sort_unstable();
            assert_eq!(
                storage
                    .existing_subject_ids(&yomibu_ext::wanikani::collection(), &ids, deadline())
                    .await
                    .unwrap(),
                ids
            );
        }
        let input = generated(
            offset as u64 + 1,
            count,
            learner,
            account,
            t,
            observation,
            include_subjects,
        );
        run.observe(&input);
        saved = Some(storage.save_group(&input, deadline()).await.unwrap());
    }
    storage
        .record_completion(
            saved.unwrap().collection_id,
            saved.unwrap().learner_progress_source_id,
            at(t + 100),
            deadline(),
        )
        .await
        .unwrap();
    run.elapsed_ms = began.elapsed().as_millis();
    run
}
fn counters(c: &Counters) -> [usize; 5] {
    [
        c.commands.load(Ordering::Relaxed),
        c.parses.load(Ordering::Relaxed),
        c.exchanges.load(Ordering::Relaxed),
        c.bind_bytes.load(Ordering::Relaxed),
        c.max_bind_bytes.load(Ordering::Relaxed),
    ]
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL 18+ with vector and YOMIBU_TEST_DATABASE_URL"]
async fn storage_resource_scenarios() {
    let scenario =
        std::env::var("YOMIBU_STORAGE_RESOURCE_SCENARIO").unwrap_or_else(|_| "cold".into());
    assert!(
        [
            "cold",
            "reuse",
            "replay",
            "identical-replay",
            "byte-chunks",
            "overlap-source",
            "overlap-accounts",
            "near-limit"
        ]
        .contains(&scenario.as_str())
    );
    let total: usize = std::env::var("YOMIBU_STORAGE_RESOURCE_TOTAL")
        .map(|v| v.parse().unwrap())
        .unwrap_or(1000);
    let capacity: u32 = std::env::var("YOMIBU_STORAGE_RESOURCE_POOL")
        .map(|v| v.parse().unwrap())
        .unwrap_or(2);
    assert!((1..=2).contains(&capacity));
    assert!(total > 0);
    support::with_postgres(async |pool, control| {
        seed(control).await;
        let relay = Relay::start(&pool.connect_options(), Fault::Pass).await;
        let measured = PgPoolOptions::new().max_connections(capacity).min_connections(capacity).acquire_timeout(Duration::from_secs(5)).connect_with(relay.options.clone()).await.unwrap();
        let storage = PostgresLearnerStorage::new(measured.clone());
        if matches!(scenario.as_str(), "reuse" | "replay" | "identical-replay") {
            for offset in (0..total).step_by(GROUP_ROWS) {
                let count = GROUP_ROWS.min(total - offset);
                let input = if scenario == "reuse" { group(2, "seed-subjects", 10, (offset as u64 + 1..=offset as u64 + count as u64).map(|id| synthetic_subject(id, 10)).collect(), vec![]) } else { generated(offset as u64 + 1, count, 1, "a", 10, 20, true) };
                storage.save_group(&input, deadline()).await.unwrap();
            }
        }
        let stats = relay.counters();
        let setup = counters(&stats);
        stats.max_bind_bytes.store(0, Ordering::Relaxed);
        let began = Instant::now();
        let runs = match scenario.as_str() {
            "overlap-source" | "overlap-accounts" => {
                let (a, b) = tokio::join!(import(&storage, total, 1, "a", 30, 20, true), import(&storage, total, if scenario == "overlap-source" { 1 } else { 2 }, if scenario == "overlap-source" { "a" } else { "b" }, 10, 40, true));
                vec![a, b]
            }
            "byte-chunks" => {
                let subjects = (1..=8)
                    .map(|id| {
                        let mut s = synthetic_subject(id, 10);
                        s.source = SourceRecord::new(at(10), &serde_json::json!({"id":id,"long_synthetic_field":"x".repeat(900*1024)}).to_string()).unwrap();
                        s
                    })
                    .collect();
                let input = group(1, "a", 10, subjects, (1..=8).map(|id| progress(id, 10, 20)).collect());
                let mut run = Run::default();
                run.observe(&input);
                storage.save_group(&input, deadline()).await.unwrap();
                run.elapsed_ms = began.elapsed().as_millis();
                vec![run]
            }
            "near-limit" => {
                let input = generated(1, 1, 1, "a", 10, 20, true);
                let mut p = input.progress()[0].clone();
                let fixed = raw_bytes(&input) - p.progress_stage.as_ref().map_or(0, String::len);
                p.progress_stage = Some("x".repeat(MAX_REQUEST_INPUT_BYTES - 4096 - fixed));
                let input = PreparedSaveGroup::new(input.collection().clone(), input.learner_progress_source().clone(), input.account().clone(), input.subjects().to_vec(), vec![p]).unwrap();
                let mut run = Run::default();
                run.observe(&input);
                storage.save_group(&input, deadline()).await.unwrap();
                run.elapsed_ms = began.elapsed().as_millis();
                vec![run]
            }
            _ => vec![import(&storage, total, 1, "a", if scenario == "replay" { 5 } else { 10 }, 40, matches!(scenario.as_str(), "cold" | "identical-replay")).await],
        };
        let elapsed = began.elapsed().as_millis();
        let after = counters(&stats);
        let measured_commands = after[0] - setup[0];
        let groups: usize = runs.iter().map(|r| r.groups).sum();
        // Reject row-by-row SQL independently of timings. Payload-sensitive chunk
        // counts may vary, but this matrix must remain far below 500 commands/group.
        assert!(measured_commands < groups * 64 + 16, "commands grew per row: {measured_commands}/{groups}");
        assert!(after[4] <= 8 * 1024 * 1024, "encoded chunk exceeded hard bound");
        if scenario == "near-limit" {
            assert!(after[4] > 1024 * 1024);
        }
        if scenario == "byte-chunks" {
            assert!(after[4] <= 1024 * 1024);
            // Verify full payload persistence plus the measured frame bound,
            // rather than counting unrelated setup/timeout SQL statements.
            let complete: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM subjects
                 WHERE source_data->>'long_synthetic_field'=repeat('x',900*1024)",
            ).fetch_one(control).await.unwrap();
            assert_eq!(complete, 8);
        }
        for run in &runs {
            assert!(run.max_group_bytes <= MAX_REQUEST_INPUT_BYTES);
            assert!(run.max_subjects <= GROUP_ROWS);
            assert!(run.max_progress <= GROUP_ROWS);
        }
        verify(
            control,
            &scenario,
            if scenario == "near-limit" {
                1
            } else if scenario == "byte-chunks" {
                8
            } else {
                total
            },
        )
        .await;
        let mut payloads: Vec<_> = runs.iter().flat_map(|r| r.payload_sizes.iter().copied()).collect();
        payloads.sort_unstable();
        let distribution = serde_json::json!({"sample_count":payloads.len(),"min":payloads[0],"p50":payloads[payloads.len()/2],"p95":payloads[payloads.len()*95/100],"max":payloads[payloads.len()-1]});
        println!("RESOURCE {}", serde_json::json!({"scenario":scenario,"total_per_import":total,"pool":capacity,"active_imports":runs.len(),"group_rows":GROUP_ROWS,"groups":groups,"input_bytes":runs.iter().map(|r|r.input_bytes).sum::<usize>(),"max_group_bytes":runs.iter().map(|r|r.max_group_bytes).max(),"max_retained_groups":runs.len(),"max_chunk_bind_bytes":after[4],"bind_bytes":after[3]-setup[3],"commands":measured_commands,"parse_messages":after[1]-setup[1],"protocol_exchanges":after[2]-setup[2],"setup_commands":setup[0],"setup_parse_messages":setup[1],"setup_exchanges":setup[2],"elapsed_ms":elapsed,"import_elapsed_ms":runs.iter().map(|r|r.elapsed_ms).collect::<Vec<_>>(),"payload_utf8_bytes":distribution}));
        measured.close().await;
    })
    .await;
}
async fn verify(pool: &PgPool, scenario: &str, total: usize) {
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM subjects")
            .fetch_one(pool)
            .await
            .unwrap(),
        total as i64
    );
    let progress_count = if scenario == "overlap-accounts" {
        2 * total
    } else {
        total
    };
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM learner_progress")
            .fetch_one(pool)
            .await
            .unwrap(),
        progress_count as i64
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM entries")
            .fetch_one(pool)
            .await
            .unwrap(),
        0
    );
    if matches!(scenario, "overlap-source" | "replay" | "identical-replay") {
        let expected = if scenario == "overlap-source" { 30 } else { 10 };
        let bad: i64 = sqlx::query_scalar("SELECT count(*) FROM learner_progress WHERE (source_data->>'source_updated_at')::timestamptz<>$1 OR recorded_at<>$2").bind(at(expected)).bind(at(40)).fetch_one(pool).await.unwrap();
        assert_eq!(bad, 0);
    }
}
