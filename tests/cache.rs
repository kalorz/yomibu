use std::fs;
use yomibu::cache::load;

#[test]
fn loads_an_empty_account_with_unicode_and_utc_times() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("wanikani.json"),
        include_str!("fixtures/empty.json"),
    )
    .unwrap();
    let snapshot = load(dir.path()).unwrap();
    assert_eq!(snapshot.learner.id, "synthetic-learner");
    assert_eq!(snapshot.learner.username, "テスト");
    assert_eq!(snapshot.learner.level, 1);
    assert_eq!(snapshot.learner.subscription.max_level_granted, 3);
    assert_eq!(snapshot.learner.current_vacation_started_at, None);
    assert_eq!(snapshot.learner.subscription.period_ends_at, None);
    assert_eq!(
        snapshot.sync_completed_at.to_rfc3339(),
        "2026-09-27T10:00:05+00:00"
    );
}

#[test]
fn missing_cache_error_identifies_the_path_and_remedy() {
    let dir = tempfile::tempdir().unwrap();
    let error = load(dir.path()).unwrap_err().to_string();
    assert!(error.contains("No cache"), "{error}");
    assert!(error.contains(&dir.path().display().to_string()), "{error}");
    assert!(error.contains("--data-dir"), "{error}");
    assert!(!dir.path().join("wanikani.json").exists());
}

#[test]
fn corrupt_cache_is_preserved_and_has_recovery_guidance() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wanikani.json");
    fs::write(&path, "{truncated").unwrap();
    let error = load(dir.path()).unwrap_err().to_string();
    assert!(error.contains("Corrupt cache"), "{error}");
    assert!(error.contains("restore"), "{error}");
    assert_eq!(fs::read_to_string(path).unwrap(), "{truncated");
}

#[test]
fn unsupported_schema_is_reported_before_decoding_its_payload() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("wanikani.json"),
        r#"{"schema_version":999,"snapshot":null}"#,
    )
    .unwrap();
    let error = load(dir.path()).unwrap_err().to_string();
    assert!(error.contains("Unsupported cache schema 999"), "{error}");
    assert!(error.contains("upgrade"), "{error}");
}

#[test]
fn io_failure_is_distinct_from_a_missing_or_corrupt_cache() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("wanikani.json")).unwrap();
    let error = load(dir.path()).unwrap_err().to_string();
    assert!(error.contains("Cannot read cache"), "{error}");
    assert!(error.contains("permissions"), "{error}");
}

#[test]
fn preserves_subject_variants_progress_and_absence() {
    use yomibu::domain::LexicalContent;
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("wanikani.json"),
        include_str!("fixtures/mixed.json"),
    )
    .unwrap();
    let snapshot = load(dir.path()).unwrap();
    assert_eq!(snapshot.subjects.len(), 4);
    let kanji = &snapshot.subjects[0];
    assert_eq!(kanji.characters, "一");
    assert!(kanji.meanings[0].accepted_answer);
    assert!(kanji.hidden_at.is_some());
    let LexicalContent::Kanji { readings } = &kanji.lexical else {
        panic!("expected kanji")
    };
    assert_eq!(readings[0].kind, "onyomi");
    assert_eq!(readings[0].reading, "いち");
    let LexicalContent::Vocabulary {
        readings,
        parts_of_speech,
        context_sentences,
    } = &snapshot.subjects[1].lexical
    else {
        panic!("expected vocabulary")
    };
    assert_eq!(readings[0].reading, "ひとつ");
    assert_eq!(parts_of_speech, &["noun"]);
    assert_eq!(context_sentences[0].japanese, "一つあります。");
    assert!(matches!(
        &snapshot.subjects[2].lexical,
        LexicalContent::KanaVocabulary { .. }
    ));
    assert_eq!(snapshot.assignments[0].srs_stage, 0);
    assert_eq!(snapshot.assignments[0].started_at, None);
    assert!(snapshot.assignments[1].burned_at.is_some());
    assert_eq!(snapshot.unavailable_subjects[0].id, 5);
    assert!(!snapshot.review_statistics.iter().any(|r| r.subject_id == 3));
    assert_eq!(snapshot.review_statistics[0].reading_max_streak, 9);
    assert_eq!(snapshot.review_statistics[0].percentage_correct, 91);
}

fn rejects_change(pointer: &str, replacement: serde_json::Value) -> bool {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/mixed.json")).unwrap();
    *value.pointer_mut(pointer).unwrap() = replacement;
    let dir = tempfile::tempdir().unwrap();
    let bytes = serde_json::to_vec(&value).unwrap();
    let path = dir.path().join("wanikani.json");
    fs::write(&path, &bytes).unwrap();
    let result = load(dir.path());
    assert_eq!(fs::read(path).unwrap(), bytes);
    result.is_err()
}

#[test]
fn rejects_invalid_required_domain_values() {
    let cases = [
        ("/snapshot/learner/id", serde_json::json!("")),
        (
            "/snapshot/learner/username",
            serde_json::json!("\u{1b}[31m"),
        ),
        ("/snapshot/learner/level", serde_json::json!(0)),
        (
            "/snapshot/learner/subscription/max_level_granted",
            serde_json::json!(61),
        ),
        ("/snapshot/learner/subscription/kind", serde_json::json!("")),
        (
            "/snapshot/sync_completed_at",
            serde_json::json!("2026-09-26T00:00:00Z"),
        ),
        ("/snapshot/subjects/0/id", serde_json::json!(0)),
        ("/snapshot/subjects/0/level", serde_json::json!(4)),
        ("/snapshot/subjects/0/srs_system_id", serde_json::json!(0)),
        ("/snapshot/subjects/0/characters", serde_json::json!(" ")),
        ("/snapshot/subjects/0/meanings", serde_json::json!([])),
        (
            "/snapshot/subjects/0/meanings/0/meaning",
            serde_json::json!(""),
        ),
        (
            "/snapshot/subjects/0/lexical/readings",
            serde_json::json!([]),
        ),
        (
            "/snapshot/subjects/1/lexical/readings/0/reading",
            serde_json::json!(""),
        ),
        (
            "/snapshot/review_statistics/0/percentage_correct",
            serde_json::json!(101),
        ),
    ];
    let failures: Vec<_> = cases
        .into_iter()
        .filter_map(|(p, v)| (!rejects_change(p, v)).then_some(p))
        .collect();
    assert!(failures.is_empty(), "accepted invalid fields: {failures:?}");
}

#[test]
fn rejects_missing_and_mismatched_subject_references() {
    let cases = [
        ("/snapshot/assignments/0/subject_id", serde_json::json!(999)),
        (
            "/snapshot/assignments/0/subject_kind",
            serde_json::json!("vocabulary"),
        ),
        (
            "/snapshot/review_statistics/0/subject_id",
            serde_json::json!(999),
        ),
        (
            "/snapshot/review_statistics/0/subject_kind",
            serde_json::json!("vocabulary"),
        ),
        (
            "/snapshot/unavailable_subjects/0/kind",
            serde_json::json!("kanji"),
        ),
        ("/snapshot/unavailable_subjects", serde_json::json!([])),
    ];
    let failures: Vec<_> = cases
        .into_iter()
        .filter_map(|(p, v)| (!rejects_change(p, v)).then_some(p))
        .collect();
    assert!(
        failures.is_empty(),
        "accepted broken references: {failures:?}"
    );
}

#[test]
fn rejects_duplicate_and_unreferenced_records() {
    let mut cases = Vec::new();
    let value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/mixed.json")).unwrap();
    for collection in [
        "subjects",
        "unavailable_subjects",
        "assignments",
        "review_statistics",
    ] {
        let pointer = format!("/snapshot/{collection}");
        let mut records = value.pointer(&pointer).unwrap().as_array().unwrap().clone();
        records.push(records[0].clone());
        cases.push((pointer, serde_json::json!(records)));
    }
    cases.extend([
        (
            "/snapshot/unavailable_subjects/0/id".into(),
            serde_json::json!(1),
        ),
        ("/snapshot/subjects/3/id".into(), serde_json::json!(99)),
        (
            "/snapshot/assignments/1/subject_id".into(),
            serde_json::json!(1),
        ),
        (
            "/snapshot/review_statistics/1/subject_id".into(),
            serde_json::json!(1),
        ),
    ]);
    let failures: Vec<_> = cases
        .into_iter()
        .filter_map(|(p, v)| (!rejects_change(&p, v)).then_some(p))
        .collect();
    assert!(
        failures.is_empty(),
        "accepted duplicate/unreferenced records: {failures:?}"
    );
}

#[test]
fn rejects_blank_kanji_reading_classification() {
    assert!(rejects_change(
        "/snapshot/subjects/0/lexical/readings/0/kind",
        serde_json::json!(" "),
    ));
}
