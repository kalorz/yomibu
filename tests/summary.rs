use std::fs;
use yomibu::{cache::load, domain::Snapshot};

fn fixture(json: &str) -> Snapshot {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("wanikani.json"), json).unwrap();
    load(dir.path()).unwrap()
}

#[test]
fn summarizes_an_empty_account() {
    let snapshot = fixture(include_str!("fixtures/empty.json"));
    let summary = snapshot.summarize().unwrap();
    assert_eq!(summary.username, "テスト");
    assert_eq!(summary.level, 1);
    assert_eq!(summary.sync_completed_at, snapshot.sync_completed_at);
    assert_eq!(summary.kanji, 0);
    assert_eq!(summary.vocabulary, 0);
    assert_eq!(summary.kana_vocabulary, 0);
    assert_eq!(summary.hidden_subjects, 0);
    assert_eq!(summary.unavailable_content, 0);
}

#[test]
fn counts_all_cached_material_and_distinguishes_hidden_from_unavailable() {
    let snapshot = fixture(include_str!("fixtures/mixed.json"));
    let summary = snapshot.summarize().unwrap();
    assert_eq!(summary.kanji, 2);
    assert_eq!(summary.vocabulary, 2);
    assert_eq!(summary.kana_vocabulary, 1);
    assert_eq!(summary.hidden_subjects, 3);
    assert_eq!(summary.unavailable_content, 1);
}

#[test]
fn groups_raw_stages_by_srs_system_without_inventing_a_missing_system() {
    use std::collections::BTreeMap;
    let snapshot = fixture(include_str!("fixtures/mixed.json"));
    let summary = snapshot.summarize().unwrap();
    assert_eq!(
        summary.srs_stages,
        BTreeMap::from([
            (None, BTreeMap::from([(4, 1)])),
            (Some(1), BTreeMap::from([(0, 1)])),
            (Some(2), BTreeMap::from([(2, 1), (9, 1)])),
        ])
    );
    assert!(
        fixture(include_str!("fixtures/empty.json"))
            .summarize()
            .unwrap()
            .srs_stages
            .is_empty()
    );
}

#[test]
fn calculates_accuracy_from_aggregate_counters_including_excluded_content() {
    let snapshot = fixture(include_str!("fixtures/mixed.json"));
    let summary = snapshot.summarize().unwrap();
    assert_eq!(summary.reading_accuracy.correct(), 9);
    assert_eq!(summary.reading_accuracy.total(), 11);
    assert!((summary.reading_accuracy.percentage().unwrap() - 900.0 / 11.0).abs() < 1e-10);
    assert_eq!(summary.meaning_accuracy.correct(), 10);
    assert_eq!(summary.meaning_accuracy.total(), 20);
    assert_eq!(summary.meaning_accuracy.percentage(), Some(50.0));
}

#[test]
fn no_reviews_is_distinct_from_zero_accuracy_for_each_answer_type() {
    let empty = fixture(include_str!("fixtures/empty.json"));
    let summary = empty.summarize().unwrap();
    assert_eq!(summary.reading_accuracy.percentage(), None);
    assert_eq!(summary.meaning_accuracy.percentage(), None);
    let mut snapshot = fixture(include_str!("fixtures/mixed.json"));
    for r in &mut snapshot.review_statistics {
        r.reading_correct = 0;
        r.reading_incorrect = 0;
    }
    let summary = snapshot.summarize().unwrap();
    assert_eq!(summary.reading_accuracy.percentage(), None);
    assert_eq!(summary.meaning_accuracy.percentage(), Some(50.0));
    snapshot.review_statistics[0].reading_incorrect = 1;
    assert_eq!(
        snapshot.summarize().unwrap().reading_accuracy.percentage(),
        Some(0.0)
    );
}

#[test]
fn aggregates_large_source_counters_without_overflow() {
    let mut snapshot = fixture(include_str!("fixtures/mixed.json"));
    for r in &mut snapshot.review_statistics {
        r.reading_correct = u64::MAX;
        r.reading_incorrect = u64::MAX;
        r.meaning_correct = u64::MAX;
        r.meaning_incorrect = u64::MAX;
    }
    let summary = snapshot.summarize().unwrap();
    assert_eq!(summary.reading_accuracy.correct(), u128::from(u64::MAX) * 4);
    assert_eq!(summary.reading_accuracy.total(), u128::from(u64::MAX) * 8);
    assert_eq!(summary.reading_accuracy.percentage(), Some(50.0));
    assert_eq!(summary.meaning_accuracy.percentage(), Some(50.0));
}
