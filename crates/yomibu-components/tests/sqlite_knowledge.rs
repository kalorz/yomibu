use yomibu_components::sqlite_knowledge_store::{
    KnowledgeScope, MappedKnowledgeRecord, ScopeMetadata, SourceKind, SqliteKnowledgeError,
    SqliteKnowledgeStore, StoredKnowledge,
};
use yomibu_core::domain::{
    inventory::{InventoryWord, LearnerInventory, MappedContent, SourceOrigin},
    knowledge::{LearnerKnowledgePolicy, LearnerProgress},
};

fn initialized_database() -> (tempfile::TempDir, std::path::PathBuf, SqliteKnowledgeStore) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("knowledge.sqlite");
    let store = SqliteKnowledgeStore::new(&path);
    store.initialize().unwrap();
    (directory, path, store)
}

fn sql_integer(connection: &rusqlite::Connection, sql: &str) -> i64 {
    connection.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn word(id: &str) -> MappedKnowledgeRecord {
    MappedKnowledgeRecord {
        external_id: id.into(),
        content: MappedContent::Vocabulary(InventoryWord {
            id: format!("word:{id}"),
            written_form: "猫".into(),
            readings: vec!["ねこ".into(), "ネコ".into()],
            meanings: vec!["cat".into()],
            direct_object: Some(false),
            origin: SourceOrigin::Manual,
        }),
        progress: Some(LearnerProgress {
            is_marked_known: Some(true),
            ..Default::default()
        }),
    }
}

fn facts(stored: &StoredKnowledge) -> Vec<(String, serde_json::Value, Option<LearnerProgress>)> {
    stored
        .records
        .iter()
        .map(|record| {
            let content = match &record.content {
                MappedContent::Vocabulary(word) => serde_json::json!(["vocabulary", word]),
                MappedContent::NonVocabulary(origin) => {
                    serde_json::json!(["non-vocabulary", origin])
                }
                MappedContent::Unavailable(origin) => serde_json::json!(["unavailable", origin]),
            };
            (record.external_id.clone(), content, record.progress)
        })
        .collect()
}

fn prepare(stored: StoredKnowledge, policy: LearnerKnowledgePolicy) -> serde_json::Value {
    serde_json::to_value(
        LearnerInventory::from_mapped(
            stored
                .records
                .into_iter()
                .map(|record| (record.content, record.progress)),
            &policy,
        )
        .unwrap(),
    )
    .unwrap()
}

fn scope(profile: &str) -> KnowledgeScope {
    KnowledgeScope {
        learner_id: profile.into(),
        collection_id: "shared".into(),
        profile_id: profile.into(),
    }
}

fn metadata() -> ScopeMetadata {
    ScopeMetadata {
        source_kind: SourceKind::Manual,
        provider_account_id: None,
        access_expires_at: None,
    }
}

#[test]
fn initialization_is_explicit_and_empty_publications_survive_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("knowledge.sqlite");
    let store = SqliteKnowledgeStore::new(&path);
    assert!(!path.exists());
    assert!(matches!(
        store.read(&scope("a")),
        Err(SqliteKnowledgeError::MissingDatabase)
    ));
    assert!(!path.exists());
    store.initialize().unwrap();
    assert!(matches!(
        store.read(&scope("a")),
        Err(SqliteKnowledgeError::MissingScope)
    ));
    store.publish(&scope("a"), &metadata(), &[]).unwrap();
    drop(store);
    let store = SqliteKnowledgeStore::new(&path);
    store.initialize().unwrap();
    let stored = store.read(&scope("a")).unwrap();
    assert_eq!(stored.scope, scope("a"));
    assert_eq!(stored.metadata, metadata());
    assert!(stored.records.is_empty());
    assert!(matches!(
        store.read(&scope("b")),
        Err(SqliteKnowledgeError::MissingScope)
    ));
}

#[test]
fn shared_content_updates_preserve_other_profiles_facts_and_availability() {
    let (_directory, path, store) = initialized_database();
    let mut a_metadata = metadata();
    a_metadata.access_expires_at = chrono::DateTime::from_timestamp(10, 0);
    let a = [
        word("x"),
        MappedKnowledgeRecord {
            external_id: "n".into(),
            content: MappedContent::NonVocabulary(SourceOrigin::Manual),
            progress: Some(LearnerProgress::default()),
        },
        MappedKnowledgeRecord {
            external_id: "u".into(),
            content: MappedContent::Unavailable(SourceOrigin::Manual),
            progress: None,
        },
    ];
    store.publish(&scope("a"), &a_metadata, &a).unwrap();
    let mut b = [word("x"), word("u")];
    b[0].progress.as_mut().unwrap().is_marked_known = Some(false);
    let mut b_metadata = metadata();
    b_metadata.provider_account_id = Some("b-account".into());
    b_metadata.access_expires_at = chrono::DateTime::from_timestamp(20, 0);
    store.publish(&scope("b"), &b_metadata, &b).unwrap();
    let before_b = facts(&store.read(&scope("b")).unwrap());
    let before_a = facts(&store.read(&scope("a")).unwrap());
    assert_eq!(before_a[2].1[0], "unavailable");
    assert_eq!(before_b[1].1[0], "vocabulary");
    let connection = rusqlite::Connection::open(&path).unwrap();
    let subject = sql_integer(
        &connection,
        "SELECT subject_id FROM source_subjects WHERE external_id='x'",
    );
    assert_eq!(
        sql_integer(&connection, "SELECT count(*) FROM source_vocabulary"),
        2
    );
    let mut updated = word("x");
    if let MappedContent::Vocabulary(word) = &mut updated.content {
        word.written_form = "犬".into();
        word.readings = vec!["いぬ".into()];
        word.meanings.clear();
    }
    updated.progress = None;
    a_metadata.access_expires_at = None;
    store.publish(&scope("a"), &a_metadata, &[updated]).unwrap();
    let after_b = store.read(&scope("b")).unwrap();
    assert_eq!(after_b.metadata, b_metadata);
    assert_eq!(after_b.records[0].progress, b[0].progress);
    assert_eq!(after_b.records[1].progress, b[1].progress);
    assert_eq!(facts(&after_b)[1], before_b[1]);
    assert_eq!(facts(&after_b)[0].1[1]["written_form"], "犬");
    assert_eq!(facts(&after_b)[0].1[1]["meanings"], serde_json::json!([]));
    assert_eq!(
        sql_integer(
            &connection,
            "SELECT subject_id FROM source_subjects WHERE external_id='x'"
        ),
        subject
    );
    for content in [
        MappedContent::NonVocabulary(SourceOrigin::Manual),
        MappedContent::Unavailable(SourceOrigin::Manual),
    ] {
        store
            .publish(
                &scope("a"),
                &a_metadata,
                &[MappedKnowledgeRecord {
                    external_id: "x".into(),
                    content,
                    progress: None,
                }],
            )
            .unwrap();
        assert_eq!(facts(&store.read(&scope("b")).unwrap()), facts(&after_b));
    }
    store.publish(&scope("a"), &a_metadata, &[]).unwrap();
    assert!(store.read(&scope("a")).unwrap().records.is_empty());
    assert_eq!(
        sql_integer(&connection, "SELECT count(*) FROM source_vocabulary"),
        2
    );
    assert_eq!(facts(&store.read(&scope("b")).unwrap()), facts(&after_b));
}

#[test]
fn a_late_database_constraint_failure_rolls_back_content_and_new_profile() {
    let (_directory, path, store) = initialized_database();
    store
        .publish(&scope("a"), &metadata(), &[word("x"), word("retained")])
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut records = [word("x"), word("new")];
    if let MappedContent::Vocabulary(word) = &mut records[0].content {
        word.written_form = "犬".into();
    }
    if let MappedContent::Vocabulary(word) = &mut records[1].content {
        word.id = "word:retained".into();
    }
    let mut new_metadata = metadata();
    new_metadata.provider_account_id = Some("new-account".into());
    let error = store
        .publish(&scope("new-profile"), &new_metadata, &records)
        .unwrap_err();
    assert!(
        matches!(error, SqliteKnowledgeError::Database(ref error) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation))
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "all database pages must roll back"
    );
    assert!(matches!(
        store.read(&scope("new-profile")),
        Err(SqliteKnowledgeError::MissingScope)
    ));
    store
        .publish(&scope("new-profile"), &new_metadata, &records[..1])
        .unwrap();
}

#[test]
fn ordinary_reads_do_not_adopt_unmarked_or_unsupported_databases() {
    for setup in [
        "CREATE TABLE foreign_data (value TEXT)",
        "PRAGMA application_id=123",
        "PRAGMA user_version=2",
        "PRAGMA application_id=0x594f4d49; PRAGMA user_version=2",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("knowledge.sqlite");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(setup).unwrap();
        drop(connection);
        let before = std::fs::read(&path).unwrap();
        let store = SqliteKnowledgeStore::new(&path);
        assert!(
            matches!(
                store.initialize(),
                Err(SqliteKnowledgeError::UnsupportedSchema)
            ),
            "{setup}"
        );
        assert!(
            matches!(
                store.read(&scope("a")),
                Err(SqliteKnowledgeError::UnsupportedSchema)
            ),
            "{setup}"
        );
        assert!(matches!(
            store.publish(&scope("a"), &metadata(), &[]),
            Err(SqliteKnowledgeError::UnsupportedSchema)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before, "{setup}");
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("corrupt.sqlite");
    std::fs::write(&path, b"not a SQLite database").unwrap();
    assert!(
        matches!(SqliteKnowledgeStore::new(&path).initialize(), Err(SqliteKnowledgeError::Database(ref error)) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::NotADatabase))
    );
    assert_eq!(std::fs::read(path).unwrap(), b"not a SQLite database");
}

#[test]
fn reads_reject_corrupt_scoped_values_and_required_relationships() {
    for corruption in [
        "UPDATE source_vocabulary SET direct_object=2",
        "UPDATE source_vocabulary SET written_form=CAST(x'ff' AS TEXT)",
        "UPDATE source_learner_progress SET is_hidden=-1",
        "UPDATE source_learner_progress SET is_marked_known='true'",
        "UPDATE source_learner_progress SET learning_started_at='yesterday'",
        "UPDATE source_learner_progress SET first_passed_at=''",
        "UPDATE source_learner_progress SET first_passed_at='2026-01-01T00:00:00'",
        "UPDATE source_learner_progress SET first_passed_at='2026-01-01 09:00:00+09:00'",
        "UPDATE source_learner_progress SET first_passed_at='2026-01-01T00:00:00 UTC'",
        "UPDATE source_learner_progress SET first_passed_at='2026-01-01T00:00:00Z'",
        "UPDATE source_learner_profiles SET access_expires_at='bad'",
        "UPDATE source_learner_profiles SET provider_account_id=''",
        "UPDATE source_collections SET source_kind='unknown'",
        "UPDATE source_profile_subjects SET content_kind='unknown'",
        "UPDATE source_profile_subjects SET position=-1",
        "UPDATE source_vocabulary_readings SET position=9 WHERE position=0",
        "UPDATE source_vocabulary_meanings SET meaning=' '",
        "UPDATE source_subjects SET external_id=''",
        "UPDATE source_subjects SET collection_id='other'",
        "UPDATE source_profile_subjects SET collection_id='other'",
        "UPDATE source_vocabulary SET collection_id='other'",
        "DELETE FROM source_vocabulary",
        "DELETE FROM source_subjects",
        "DELETE FROM learners",
        "DELETE FROM source_collections",
        "INSERT INTO source_learner_progress (profile_id,subject_id,is_hidden) VALUES ('a',999,0)",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("knowledge.sqlite");
        let store = SqliteKnowledgeStore::new(&path);
        store.initialize().unwrap();
        store
            .publish(&scope("a"), &metadata(), &[word("x")])
            .unwrap();
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        connection.execute_batch(corruption).unwrap();
        assert!(
            matches!(
                store.read(&scope("a")),
                Err(SqliteKnowledgeError::Corrupt(_))
            ),
            "{corruption}"
        );
    }
}

#[test]
fn initialization_keeps_new_and_existing_empty_databases_private() {
    use std::os::unix::fs::PermissionsExt;
    for existing in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("knowledge.sqlite");
        if existing {
            std::fs::write(&path, []).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        let store = SqliteKnowledgeStore::new(&path);
        store.initialize().unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
            0
        );
        store
            .publish(&scope("a"), &metadata(), &[word("x")])
            .unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
            0
        );
    }
}

#[test]
fn a_duplicate_account_binding_rolls_back_the_new_profile() {
    let (_directory, _path, store) = initialized_database();
    let mut metadata = metadata();
    metadata.provider_account_id = Some("account".into());
    store.publish(&scope("a"), &metadata, &[]).unwrap();
    let mut duplicate = scope("new-profile");
    duplicate.learner_id = "a".into();
    assert!(matches!(
        store.publish(&duplicate, &metadata, &[]),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    assert!(matches!(
        store.read(&duplicate),
        Err(SqliteKnowledgeError::MissingScope)
    ));
}

#[test]
fn ordinary_operations_reject_changed_journaling_until_explicit_initialization() {
    let (_directory, path, store) = initialized_database();
    store
        .publish(&scope("a"), &metadata(), &[word("x")])
        .unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    let mode: String = connection
        .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    drop(connection);
    assert!(matches!(
        store.read(&scope("a")),
        Err(SqliteKnowledgeError::UnsupportedJournalMode)
    ));
    assert!(matches!(
        store.publish(&scope("a"), &metadata(), &[]),
        Err(SqliteKnowledgeError::UnsupportedJournalMode)
    ));
    store.initialize().unwrap();
    assert_eq!(store.read(&scope("a")).unwrap().records.len(), 1);
}

#[test]
fn publication_replaces_a_malformed_deadline_without_rebinding_the_profile() {
    let (_directory, path, store) = initialized_database();
    store
        .publish(&scope("a"), &metadata(), &[word("x")])
        .unwrap();
    rusqlite::Connection::open(path)
        .unwrap()
        .execute(
            "UPDATE source_learner_profiles SET access_expires_at='bad'",
            [],
        )
        .unwrap();
    store
        .publish(&scope("a"), &metadata(), &[word("x")])
        .unwrap();
    assert_eq!(store.read(&scope("a")).unwrap().metadata, metadata());
}

#[test]
fn the_full_common_utc_timestamp_range_survives_publication() {
    let (_directory, _path, store) = initialized_database();
    let mut metadata = metadata();
    metadata.access_expires_at = Some(chrono::DateTime::<chrono::Utc>::MAX_UTC);
    let mut record = word("x");
    record.progress = Some(LearnerProgress {
        learning_started_at: Some(chrono::DateTime::<chrono::Utc>::MAX_UTC),
        first_passed_at: Some(chrono::DateTime::<chrono::Utc>::MIN_UTC),
        ..Default::default()
    });
    store.publish(&scope("a"), &metadata, &[record]).unwrap();
    let stored = store.read(&scope("a")).unwrap();
    assert_eq!(stored.metadata, metadata);
    assert_eq!(
        stored.records[0].progress.unwrap().learning_started_at,
        metadata.access_expires_at
    );
    assert_eq!(
        stored.records[0].progress.unwrap().first_passed_at,
        Some(chrono::DateTime::<chrono::Utc>::MIN_UTC)
    );
}

#[test]
fn read_integrity_is_scoped_and_database_bytes_remain_unchanged() {
    let (_directory, path, store) = initialized_database();
    store
        .publish(&scope("a"), &metadata(), &[word("x")])
        .unwrap();
    store
        .publish(&scope("b"), &metadata(), &[word("y")])
        .unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    connection
        .execute("DELETE FROM learners WHERE learner_id='b'", [])
        .unwrap();
    drop(connection);
    let before = std::fs::read(&path).unwrap();
    let stored = store.read(&scope("a")).unwrap();
    assert_eq!(stored.records.len(), 1);
    assert!(matches!(
        store.read(&scope("b")),
        Err(SqliteKnowledgeError::Corrupt("missing learner"))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn sql_enforces_relationships_uniqueness_and_required_values_without_rust_validation() {
    use rusqlite::ffi::{
        SQLITE_CONSTRAINT_FOREIGNKEY as FK, SQLITE_CONSTRAINT_NOTNULL as NULL,
        SQLITE_CONSTRAINT_PRIMARYKEY as PK, SQLITE_CONSTRAINT_UNIQUE as UNIQUE,
    };
    let (_directory, path, store) = initialized_database();
    let mut account = metadata();
    account.provider_account_id = Some("account".into());
    store
        .publish(&scope("a"), &account, &[word("x"), word("y")])
        .unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    connection
        .execute(
            "INSERT INTO source_collections VALUES ('other','manual')",
            [],
        )
        .unwrap();
    for (sql, expected) in [
        (
            "INSERT INTO source_subjects (collection_id,external_id) VALUES ('missing','z')",
            FK,
        ),
        (
            "INSERT INTO source_learner_profiles (profile_id,learner_id,collection_id) VALUES ('p','missing','shared')",
            FK,
        ),
        (
            "INSERT INTO source_learner_progress (profile_id,subject_id,is_hidden) VALUES ('missing',1,0)",
            FK,
        ),
        (
            "INSERT INTO source_vocabulary_readings VALUES (999,0,'missing')",
            FK,
        ),
        (
            "UPDATE source_profile_subjects SET collection_id='other'",
            FK,
        ),
        ("UPDATE source_vocabulary SET collection_id='other'", FK),
        ("DELETE FROM learners WHERE learner_id='a'", FK),
        (
            "DELETE FROM source_collections WHERE collection_id='shared'",
            FK,
        ),
        (
            "INSERT INTO source_subjects (collection_id,external_id) VALUES ('shared','x')",
            UNIQUE,
        ),
        ("UPDATE source_profile_subjects SET position=0", UNIQUE),
        ("UPDATE source_vocabulary SET word_id='word:x'", UNIQUE),
        (
            "INSERT INTO source_vocabulary_readings VALUES (1,0,'duplicate')",
            PK,
        ),
        (
            "INSERT INTO source_learner_profiles (profile_id,learner_id,collection_id,provider_account_id) VALUES ('p','a','shared','account')",
            UNIQUE,
        ),
        ("INSERT INTO learners VALUES (NULL)", NULL),
        (
            "INSERT INTO source_subjects (collection_id,external_id) VALUES ('shared',NULL)",
            NULL,
        ),
        ("UPDATE source_vocabulary SET written_form=NULL", NULL),
        ("UPDATE source_learner_progress SET is_hidden=NULL", NULL),
    ] {
        let error = connection.execute(sql, []).unwrap_err();
        // SQLite reports immediate RESTRICT failures as constraint-trigger errors.
        assert!(
            matches!(error, rusqlite::Error::SqliteFailure(ref detail, _)
            if detail.extended_code == expected || (expected == FK && detail.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER)),
            "{sql}: {error:?}"
        );
    }
    let mut second = scope("second");
    second.learner_id = "a".into();
    store.publish(&second, &metadata(), &[]).unwrap();
    second.profile_id = "third".into();
    store.publish(&second, &metadata(), &[]).unwrap();
    second.collection_id = "other".into();
    second.profile_id = "separate-collection".into();
    store.publish(&second, &metadata(), &[word("x")]).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM source_vocabulary WHERE word_id='word:x'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
}

#[test]
fn ordered_observations_and_unpaired_alternatives_support_both_policies() {
    let (_directory, path, store) = initialized_database();
    let mut records = [
        word("z"),
        word("a"),
        word("none"),
        word("observed"),
        word("hidden"),
        word("passed"),
    ];
    records[0].progress = Some(LearnerProgress {
        learning_started_at: chrono::DateTime::from_timestamp(1, 0),
        ..Default::default()
    });
    records[1].progress = Some(LearnerProgress {
        is_marked_known: Some(false),
        ..Default::default()
    });
    records[2].progress = None;
    records[3].progress = Some(LearnerProgress::default());
    records[4].progress.as_mut().unwrap().is_hidden = true;
    records[5].progress = Some(LearnerProgress {
        learning_started_at: chrono::DateTime::from_timestamp(1, 1),
        first_passed_at: chrono::DateTime::from_timestamp(2, 123456789),
        ..Default::default()
    });
    store.publish(&scope("a"), &metadata(), &records).unwrap();
    let expected = StoredKnowledge {
        scope: scope("a"),
        metadata: metadata(),
        records: records.into(),
    };
    drop(store);
    let store = SqliteKnowledgeStore::new(&path);
    assert_eq!(facts(&store.read(&scope("a")).unwrap()), facts(&expected));
    let lesson = prepare(
        store.read(&scope("a")).unwrap(),
        LearnerKnowledgePolicy::LessonStarted,
    );
    let pass = prepare(
        store.read(&scope("a")).unwrap(),
        LearnerKnowledgePolicy::RecordedPass,
    );
    assert_eq!(lesson["vocabulary"].as_array().unwrap().len(), 2);
    assert_eq!(pass["vocabulary"].as_array().unwrap().len(), 1);
    assert_eq!(pass["vocabulary"][0]["id"], "word:passed");
    assert_eq!(
        lesson["vocabulary"][0]["readings"],
        serde_json::json!(["ねこ", "ネコ"])
    );
    assert_eq!(
        lesson["vocabulary"][0]["meanings"],
        serde_json::json!(["cat"])
    );
    assert_eq!(
        lesson,
        prepare(expected, LearnerKnowledgePolicy::LessonStarted)
    );
}

#[test]
fn invalid_publications_do_not_change_existing_facts_or_bindings() {
    let directory = tempfile::tempdir().unwrap();
    let store = SqliteKnowledgeStore::new(directory.path().join("knowledge.sqlite"));
    store.initialize().unwrap();
    store
        .publish(&scope("a"), &metadata(), &[word("x")])
        .unwrap();
    let previous = facts(&store.read(&scope("a")).unwrap());
    let mut invalid = word("x");
    invalid.progress = None;
    if let MappedContent::Vocabulary(word) = &mut invalid.content {
        word.written_form = "x".repeat(257);
    }
    assert!(matches!(
        store.publish(&scope("a"), &metadata(), &[invalid]),
        Err(SqliteKnowledgeError::Inventory(_))
    ));
    assert!(matches!(
        store.publish(&scope("a"), &metadata(), &[word("x"), word("x")]),
        Err(SqliteKnowledgeError::InvalidInput(_))
    ));
    let mut wrong_origin = word("x");
    wrong_origin.content = MappedContent::Unavailable(SourceOrigin::Wanikani { subject_id: 1 });
    assert!(matches!(
        store.publish(&scope("a"), &metadata(), &[wrong_origin]),
        Err(SqliteKnowledgeError::InvalidInput(_))
    ));
    let mut changed = metadata();
    changed.source_kind = SourceKind::Wanikani;
    assert!(matches!(
        store.publish(&scope("a"), &changed, &[]),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    changed = metadata();
    changed.provider_account_id = Some("account".into());
    assert!(matches!(
        store.publish(&scope("a"), &changed, &[]),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    let mut wrong_scope = scope("a");
    wrong_scope.learner_id = "another".into();
    assert!(matches!(
        store.publish(&wrong_scope, &metadata(), &[]),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    assert!(matches!(
        store.read(&wrong_scope),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    wrong_scope = scope("a");
    wrong_scope.collection_id.clear();
    assert!(matches!(
        store.publish(&wrong_scope, &metadata(), &[]),
        Err(SqliteKnowledgeError::InvalidInput(_))
    ));
    assert_eq!(facts(&store.read(&scope("a")).unwrap()), previous);
}
