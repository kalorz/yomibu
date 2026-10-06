use std::fs;
use yomibu::cache::load;

#[test]
fn loads_an_empty_account_with_unicode_and_utc_times() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("wanikani.json"),
        include_str!("../../../tests/fixtures/empty.json"),
    )
    .unwrap();
    let sync_data = load(dir.path()).unwrap();
    assert_eq!(sync_data.learner.id, "synthetic-learner");
    assert_eq!(sync_data.learner.username, "テスト");
    assert_eq!(sync_data.learner.level, 1);
    assert_eq!(sync_data.learner.subscription.max_level_granted, 3);
    assert_eq!(sync_data.learner.current_vacation_started_at, None);
    assert_eq!(sync_data.learner.subscription.period_ends_at, None);
    assert_eq!(
        sync_data.sync_completed_at.to_rfc3339(),
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
        include_str!("../../../tests/fixtures/mixed.json"),
    )
    .unwrap();
    let sync_data = load(dir.path()).unwrap();
    assert_eq!(sync_data.subjects.len(), 4);
    let kanji = &sync_data.subjects[0];
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
    } = &sync_data.subjects[1].lexical
    else {
        panic!("expected vocabulary")
    };
    assert_eq!(readings[0].reading, "ひとつ");
    assert_eq!(parts_of_speech, &["noun"]);
    assert_eq!(context_sentences[0].japanese, "一つあります。");
    assert!(matches!(
        &sync_data.subjects[2].lexical,
        LexicalContent::KanaVocabulary { .. }
    ));
    assert_eq!(sync_data.assignments[0].srs_stage, 0);
    assert_eq!(sync_data.assignments[0].started_at, None);
    assert!(sync_data.assignments[1].burned_at.is_some());
    assert_eq!(sync_data.unavailable_subjects[0].id, 5);
    assert!(
        !sync_data
            .review_statistics
            .iter()
            .any(|r| r.subject_id == 3)
    );
    assert_eq!(sync_data.review_statistics[0].reading_max_streak, 9);
    assert_eq!(sync_data.review_statistics[0].percentage_correct, 91);
}

fn rejects_change(pointer: &str, replacement: serde_json::Value) -> bool {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/mixed.json")).unwrap();
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
        serde_json::from_str(include_str!("../../../tests/fixtures/mixed.json")).unwrap();
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

#[test]
fn locked_writer_round_trips_and_fully_replaces_sync_data_privately() {
    use std::os::unix::fs::PermissionsExt;
    use yomibu::cache::SyncGuard;
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
    assert_eq!(stored.summarize().unwrap(), mixed.summarize().unwrap());
    assert_eq!(stored.subjects[1].characters, "一つ");
    for path in [
        &dir,
        &root.path().join("private"),
        &dir.join("wanikani.json"),
        &dir.join("wanikani.lock"),
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
    assert_eq!(files, ["wanikani.json", "wanikani.lock"]);
}

#[test]
fn writer_preserves_invalid_caches_and_rejects_other_accounts_or_invalid_sync_data() {
    use yomibu::cache::SyncGuard;
    let fixture = tempfile::tempdir().unwrap();
    fs::write(
        fixture.path().join("wanikani.json"),
        include_str!("../../../tests/fixtures/empty.json"),
    )
    .unwrap();
    let mut next = load(fixture.path()).unwrap();
    for contents in [
        "{truncated",
        r#"{"schema_version":999}"#,
        include_str!("../../../tests/fixtures/empty.json"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("wanikani.json"), contents).unwrap();
        next.learner.id = "another-account".into();
        let result = SyncGuard::acquire(dir.path()).and_then(|guard| guard.replace(&next));
        assert!(result.is_err(), "overwrote protected cache: {contents}");
        assert_eq!(
            fs::read_to_string(dir.path().join("wanikani.json")).unwrap(),
            contents
        );
        if contents.contains("synthetic-learner") {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("another --data-dir PATH")
            );
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let guard = SyncGuard::acquire(dir.path()).unwrap();
    next.learner.level = 0;
    assert!(guard.replace(&next).is_err());
    assert!(!dir.path().join("wanikani.json").exists());
}

#[test]
fn writer_lock_fails_promptly_and_status_can_read_until_guard_is_dropped() {
    use yomibu::cache::SyncGuard;
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("wanikani.json"),
        include_str!("../../../tests/fixtures/empty.json"),
    )
    .unwrap();
    let guard = SyncGuard::acquire(dir.path()).unwrap();
    assert!(matches!(
        SyncGuard::acquire(dir.path()),
        Err(yomibu::cache::WriteError::Locked)
    ));
    assert_eq!(load(dir.path()).unwrap().learner.id, "synthetic-learner");
    drop(guard);
    assert!(SyncGuard::acquire(dir.path()).is_ok());
}
