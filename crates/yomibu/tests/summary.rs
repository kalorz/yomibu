use std::fs;
use yomibu_components::file_learning_store::cache::load;
use yomibu_core::domain::source::WaniKaniSyncData;

fn fixture(json: &str) -> WaniKaniSyncData {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("wanikani.json"), json).unwrap();
    load(dir.path()).unwrap()
}

#[test]
fn summarizes_an_empty_account() {
    let sync_data = fixture(include_str!("../../../tests/fixtures/empty.json"));
    let summary = yomibu::reports::summary::summarize(&sync_data).unwrap();
    assert_eq!(summary.username, "テスト");
    assert_eq!(summary.level, 1);
    assert_eq!(summary.sync_completed_at, sync_data.sync_completed_at);
    assert_eq!(summary.kanji, 0);
    assert_eq!(summary.vocabulary, 0);
    assert_eq!(summary.kana_vocabulary, 0);
    assert_eq!(summary.hidden_subjects, 0);
    assert_eq!(summary.unavailable_content, 0);
}

#[test]
fn counts_all_cached_material_and_distinguishes_hidden_from_unavailable() {
    let sync_data = fixture(include_str!("../../../tests/fixtures/mixed.json"));
    let summary = yomibu::reports::summary::summarize(&sync_data).unwrap();
    assert_eq!(summary.kanji, 2);
    assert_eq!(summary.vocabulary, 2);
    assert_eq!(summary.kana_vocabulary, 1);
    assert_eq!(summary.hidden_subjects, 3);
    assert_eq!(summary.unavailable_content, 1);
}

#[test]
fn groups_raw_stages_by_srs_system_without_inventing_a_missing_system() {
    use std::collections::BTreeMap;
    let sync_data = fixture(include_str!("../../../tests/fixtures/mixed.json"));
    let summary = yomibu::reports::summary::summarize(&sync_data).unwrap();
    assert_eq!(
        summary.srs_stages,
        BTreeMap::from([
            (None, BTreeMap::from([(4, 1)])),
            (Some(1), BTreeMap::from([(0, 1)])),
            (Some(2), BTreeMap::from([(2, 1), (9, 1)])),
        ])
    );
    assert!(
        yomibu::reports::summary::summarize(&fixture(include_str!(
            "../../../tests/fixtures/empty.json"
        )))
        .unwrap()
        .srs_stages
        .is_empty()
    );
}

#[test]
fn calculates_accuracy_from_aggregate_counters_including_excluded_content() {
    let sync_data = fixture(include_str!("../../../tests/fixtures/mixed.json"));
    let summary = yomibu::reports::summary::summarize(&sync_data).unwrap();
    assert_eq!(summary.reading_accuracy.correct(), 9);
    assert_eq!(summary.reading_accuracy.total(), 11);
    assert!((summary.reading_accuracy.percentage().unwrap() - 900.0 / 11.0).abs() < 1e-10);
    assert_eq!(summary.meaning_accuracy.correct(), 10);
    assert_eq!(summary.meaning_accuracy.total(), 20);
    assert_eq!(summary.meaning_accuracy.percentage(), Some(50.0));
}

#[test]
fn no_reviews_is_distinct_from_zero_accuracy_for_each_answer_type() {
    let empty = fixture(include_str!("../../../tests/fixtures/empty.json"));
    let summary = yomibu::reports::summary::summarize(&empty).unwrap();
    assert_eq!(summary.reading_accuracy.percentage(), None);
    assert_eq!(summary.meaning_accuracy.percentage(), None);
    let mut sync_data = fixture(include_str!("../../../tests/fixtures/mixed.json"));
    for r in &mut sync_data.review_statistics {
        r.reading_correct = 0;
        r.reading_incorrect = 0;
    }
    let summary = yomibu::reports::summary::summarize(&sync_data).unwrap();
    assert_eq!(summary.reading_accuracy.percentage(), None);
    assert_eq!(summary.meaning_accuracy.percentage(), Some(50.0));
    sync_data.review_statistics[0].reading_incorrect = 1;
    assert_eq!(
        yomibu::reports::summary::summarize(&sync_data)
            .unwrap()
            .reading_accuracy
            .percentage(),
        Some(0.0)
    );
}

#[test]
fn aggregates_large_source_counters_without_overflow() {
    let mut sync_data = fixture(include_str!("../../../tests/fixtures/mixed.json"));
    for r in &mut sync_data.review_statistics {
        r.reading_correct = u64::MAX;
        r.reading_incorrect = u64::MAX;
        r.meaning_correct = u64::MAX;
        r.meaning_incorrect = u64::MAX;
    }
    let summary = yomibu::reports::summary::summarize(&sync_data).unwrap();
    assert_eq!(summary.reading_accuracy.correct(), u128::from(u64::MAX) * 4);
    assert_eq!(summary.reading_accuracy.total(), u128::from(u64::MAX) * 8);
    assert_eq!(summary.reading_accuracy.percentage(), Some(50.0));
    assert_eq!(summary.meaning_accuracy.percentage(), Some(50.0));
}

#[test]
fn locked_writer_round_trips_and_fully_replaces_sync_data_privately() {
    use std::os::unix::fs::PermissionsExt;
    use yomibu_components::file_learning_store::cache::SyncGuard;
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("private/nested");
    let fixture = tempfile::tempdir().unwrap();
    fs::write(
        fixture.path().join("wanikani.json"),
        include_str!("../../../tests/fixtures/mixed.json"),
    )
    .unwrap();
    let mixed = load(fixture.path()).unwrap();
    let guard = SyncGuard::acquire(&dir).unwrap();
    guard.replace(&mixed).unwrap();
    let stored = load(&dir).unwrap();
    assert_eq!(
        yomibu::reports::summary::summarize(&stored).unwrap(),
        yomibu::reports::summary::summarize(&mixed).unwrap()
    );
    assert_eq!(stored.subjects[1].characters, "一つ");
    for path in [
        &dir,
        &root.path().join("private"),
        &dir.join("wanikani.json"),
        &dir.join("wanikani.json.lock"),
    ] {
        assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o077, 0);
    }
    fs::write(
        fixture.path().join("wanikani.json"),
        include_str!("../../../tests/fixtures/empty.json"),
    )
    .unwrap();
    guard.replace(&load(fixture.path()).unwrap()).unwrap();
    assert!(load(&dir).unwrap().subjects.is_empty());
    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    files.sort();
    assert_eq!(files, ["wanikani.json", "wanikani.json.lock"]);
}
