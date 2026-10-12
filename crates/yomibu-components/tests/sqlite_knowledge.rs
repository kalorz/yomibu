use yomibu_components::sqlite_knowledge_store::{
    KnowledgeScope, MappedKnowledgeRecord, Provider, ScopeMetadata, SqliteKnowledgeError,
    SqliteKnowledgeStore, StoredKnowledge,
};
use yomibu_core::domain::{
    inventory::{
        InventoryWord, LearnerInventory, MappedContent, SourceOrigin, UnresolvedVocabulary,
        VocabularyEntry, VocabularyTranslation,
    },
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
    let entry_id = match id {
        "y" => 12,
        "x" => 1,
        "retained" => 2,
        "new" => 3,
        "u" => 4,
        _ => panic!("unknown fixture entry"),
    };
    MappedKnowledgeRecord {
        external_id: id.into(),
        content: MappedContent::Vocabulary {
            origin: SourceOrigin::Manual,
            entries: vec![VocabularyEntry {
                id: entry_id,
                spelling: "猫".into(),
                reading: "ねこ".into(),
                parts_of_speech: vec!["noun".into()],
                is_primary_reading_in_source: Some(true),
                mapping_method: "explicit-fixture-association".into(),
                has_direct_object_evidence: Some(false),
                translations: vec![VocabularyTranslation {
                    id: entry_id * 10,
                    language_code: "en".into(),
                    text: "cat".into(),
                    display_order: 0,
                    is_primary_in_source: None,
                    origin: "explicit-fixture-association".into(),
                }],
            }],
        },
        progress: Some(LearnerProgress {
            is_marked_known: Some(true),
            ..Default::default()
        }),
    }
}

fn unresolved(id: &str) -> MappedKnowledgeRecord {
    MappedKnowledgeRecord {
        external_id: id.into(),
        content: MappedContent::UnresolvedVocabulary(UnresolvedVocabulary {
            word: InventoryWord {
                id: format!("word:{id}"),
                written_form: "猫".into(),
                readings: vec!["ねこ".into(), "ネコ".into()],
                meanings: vec!["cat".into()],
                direct_object: Some(false),
                origin: SourceOrigin::Manual,
            },
            observation: serde_json::json!({"independent_readings":["ねこ","ネコ"],"meanings":["cat"]}),
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
                MappedContent::Vocabulary { entries, .. } => {
                    serde_json::json!(["vocabulary", entries])
                }
                MappedContent::UnresolvedVocabulary(unresolved) => {
                    serde_json::json!(["unresolved-vocabulary", unresolved])
                }
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
    let id = match profile {
        "a" => 1,
        "b" => 2,
        "new-profile" => 3,
        "second" => 4,
        "third" => 5,
        "separate-collection" => 6,
        _ => panic!("unknown fixture profile"),
    };
    KnowledgeScope {
        learner_id: id,
        source_collection_id: 1,
        source_learner_profile_id: id,
    }
}

fn metadata() -> ScopeMetadata {
    ScopeMetadata {
        provider: Provider::Manual,
        learner_name: "Fixture learner".into(),
        source_collection_name: "Shared".into(),
        owner_learner_id: None,
        external_account_id: None,
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
    a_metadata.learner_name = "First learner".into();
    a_metadata.owner_learner_id = Some(1);
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
    b_metadata.learner_name = "Second learner".into();
    b_metadata.owner_learner_id = Some(1);
    b_metadata.external_account_id = Some("b-account".into());
    b_metadata.access_expires_at = chrono::DateTime::from_timestamp(20, 0);
    store.publish(&scope("b"), &b_metadata, &b).unwrap();
    let before_b = facts(&store.read(&scope("b")).unwrap());
    let before_a = facts(&store.read(&scope("a")).unwrap());
    assert_eq!(before_a[2].1[0], "unavailable");
    assert_eq!(before_b[1].1[0], "vocabulary");
    let connection = rusqlite::Connection::open(&path).unwrap();
    let subject = sql_integer(
        &connection,
        "SELECT id FROM source_subjects WHERE external_id='x'",
    );
    assert_eq!(
        sql_integer(&connection, "SELECT count(*) FROM vocabulary_entries"),
        2
    );
    let mut updated = word("x");
    if let MappedContent::Vocabulary { entries, .. } = &mut updated.content {
        entries[0].spelling = "犬".into();
        entries[0].reading = "いぬ".into();
        entries[0].translations.clear();
    }
    updated.progress = None;
    a_metadata.learner_name = "Renamed first learner".into();
    a_metadata.access_expires_at = None;
    store.publish(&scope("a"), &a_metadata, &[updated]).unwrap();
    let after_b = store.read(&scope("b")).unwrap();
    assert_eq!(after_b.metadata, b_metadata);
    assert_eq!(after_b.records[0].progress, b[0].progress);
    assert_eq!(after_b.records[1].progress, b[1].progress);
    assert_eq!(facts(&after_b)[1], before_b[1]);
    assert_eq!(facts(&after_b)[0].1[1][0]["spelling"], "犬");
    assert_eq!(
        facts(&after_b)[0].1[1][0]["translations"],
        serde_json::json!([])
    );
    assert_eq!(
        sql_integer(
            &connection,
            "SELECT id FROM source_subjects WHERE external_id='x'"
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
        sql_integer(&connection, "SELECT count(*) FROM vocabulary_entries"),
        2
    );
    assert_eq!(facts(&store.read(&scope("b")).unwrap()), facts(&after_b));
}

#[test]
fn a_late_database_constraint_failure_rolls_back_content_and_profile_changes() {
    let (_directory, path, store) = initialized_database();
    store
        .publish(&scope("a"), &metadata(), &[word("x"), word("retained")])
        .unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    // A test-only relational constraint creates a deterministic failure after writes begin.
    connection
        .execute("UPDATE vocabulary_entries SET spelling='犬' WHERE id=2", [])
        .unwrap();
    connection
        .execute(
            "CREATE UNIQUE INDEX publication_failure ON vocabulary_entries(spelling)",
            [],
        )
        .unwrap();
    drop(connection);
    let before = std::fs::read(&path).unwrap();
    let mut records = [word("x"), word("new")];
    if let MappedContent::Vocabulary { entries, .. } = &mut records[0].content {
        entries[0].spelling = "鳥".into();
    }
    if let MappedContent::Vocabulary { entries, .. } = &mut records[1].content {
        entries[0].spelling = "犬".into();
    }
    let mut new_metadata = metadata();
    new_metadata.external_account_id = Some("new-account".into());
    let existing_metadata = metadata();
    for (scope, metadata) in [
        (scope("new-profile"), &new_metadata),
        (scope("a"), &existing_metadata),
    ] {
        let error = store.publish(&scope, metadata, &records).unwrap_err();
        assert!(
            matches!(error, SqliteKnowledgeError::Database(ref error) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation))
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "all database pages must roll back"
        );
    }
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
        "PRAGMA user_version=3",
        "PRAGMA application_id=0x594f4d49; PRAGMA user_version=3",
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
fn the_superseded_schema_is_rejected_without_migration_or_file_changes() {
    let (_directory, path, store) = initialized_database();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    drop(connection);
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        store.initialize(),
        Err(SqliteKnowledgeError::UnsupportedSchema)
    ));
    assert!(matches!(
        store.read(&scope("a")),
        Err(SqliteKnowledgeError::UnsupportedSchema)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn progress_has_collection_qualified_profile_and_subject_foreign_keys() {
    let (_directory, path, store) = initialized_database();
    store
        .publish(&scope("a"), &metadata(), &[word("x")])
        .unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    let parents: Vec<String> = connection.prepare("SELECT DISTINCT \"table\" FROM pragma_foreign_key_list('source_learner_progress') ORDER BY \"table\"").unwrap().query_map([], |row| row.get(0)).unwrap().collect::<Result<_,_>>().unwrap();
    assert_eq!(
        parents,
        [
            "source_learner_profiles",
            "source_profile_subjects",
            "source_subjects"
        ]
    );
    assert_eq!(
        connection
            .execute(
                "UPDATE source_learner_progress SET source_collection_id=999",
                []
            )
            .unwrap_err()
            .sqlite_error_code(),
        Some(rusqlite::ErrorCode::ConstraintViolation)
    );
    connection
        .execute("DELETE FROM source_learner_progress", [])
        .unwrap();
    connection.execute("INSERT INTO source_learner_progress (source_learner_profile_id,source_subject_id,source_collection_id) VALUES (1,1,1)", []).unwrap();
    assert!(
        !store.read(&scope("a")).unwrap().records[0]
            .progress
            .unwrap()
            .is_hidden
    );
}

#[test]
fn reads_reject_corrupt_scoped_values_and_required_relationships() {
    for corruption in [
        "UPDATE vocabulary_entries SET has_direct_object_evidence=2",
        "UPDATE vocabulary_entries SET spelling=CAST(x'ff' AS TEXT)",
        "UPDATE source_learner_progress SET is_hidden=-1",
        "UPDATE source_learner_progress SET source_collection_id=999",
        "UPDATE source_collections SET owner_learner_id=999",
        "UPDATE source_learner_progress SET is_marked_known='true'",
        "UPDATE source_learner_progress SET learning_started_at='yesterday'",
        "UPDATE source_learner_progress SET first_passed_at=''",
        "UPDATE source_learner_progress SET first_passed_at='2026-01-01T00:00:00'",
        "UPDATE source_learner_progress SET first_passed_at='2026-01-01 09:00:00+09:00'",
        "UPDATE source_learner_progress SET first_passed_at='2026-01-01T00:00:00 UTC'",
        "UPDATE source_learner_progress SET first_passed_at='2026-01-01T00:00:00Z'",
        "UPDATE source_learner_profiles SET access_expires_at='bad'",
        "UPDATE source_learner_profiles SET external_account_id=''",
        "UPDATE source_collections SET provider='unknown'",
        "UPDATE source_profile_subjects SET content_kind='unknown'",
        "UPDATE source_profile_subjects SET position=-1",
        "UPDATE vocabulary_entry_parts_of_speech SET position=9 WHERE position=0",
        "UPDATE vocabulary_translations SET text=' '",
        "UPDATE source_subjects SET external_id=''",
        "UPDATE source_subjects SET source_collection_id=2",
        "UPDATE source_profile_subjects SET source_collection_id=2",
        "UPDATE source_subjects SET material_type='grammar'",
        "DELETE FROM vocabulary_entries",
        "DELETE FROM source_subjects",
        "DELETE FROM learners",
        "DELETE FROM source_collections",
        "INSERT INTO source_learner_progress (source_learner_profile_id,source_subject_id,source_collection_id,is_hidden) VALUES (1,999,1,0)",
    ] {
        let (_directory, path, store) = initialized_database();
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
fn initialization_preserves_supported_database_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let (_directory, path, store) = initialized_database();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    store.initialize().unwrap();
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[test]
fn one_profile_per_learner_and_collection_is_enforced_without_relying_on_account_identity() {
    let (_directory, _path, store) = initialized_database();
    let mut metadata = metadata();
    metadata.external_account_id = Some("account".into());
    store.publish(&scope("a"), &metadata, &[]).unwrap();
    let mut duplicate = scope("new-profile");
    duplicate.learner_id = 1;
    for account in [None, Some("another-account".into()), Some("account".into())] {
        metadata.external_account_id = account;
        assert!(matches!(
            store.publish(&duplicate, &metadata, &[]),
            Err(SqliteKnowledgeError::BindingMismatch)
        ));
    }
    assert!(matches!(
        store.read(&duplicate),
        Err(SqliteKnowledgeError::MissingScope)
    ));
}

#[test]
fn entity_keys_use_generated_sqlite_integer_identity_and_explicit_names() {
    let (_directory, path, _store) = initialized_database();
    let connection = rusqlite::Connection::open(path).unwrap();
    for table in [
        "learners",
        "source_collections",
        "source_subjects",
        "source_learner_profiles",
    ] {
        let kind: String = connection
            .query_row(
                &format!("SELECT type FROM pragma_table_info('{table}') WHERE name='id'"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(kind, "INTEGER", "{table}");
    }
    connection
        .execute(
            "INSERT INTO learners (name) VALUES ('First'), ('Second')",
            [],
        )
        .unwrap();
    assert_eq!(
        sql_integer(&connection, "SELECT count(DISTINCT id) FROM learners"),
        2
    );
    connection.execute("INSERT INTO source_collections (provider,name,owner_learner_id) VALUES ('manual','Private',1)", []).unwrap();
    connection
        .execute(
            "INSERT INTO source_subjects (source_collection_id,external_id) VALUES (1,'x')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO source_learner_profiles (learner_id,source_collection_id) VALUES (1,1)",
            [],
        )
        .unwrap();
    assert_eq!(
        sql_integer(&connection, "SELECT id FROM source_subjects"),
        1
    );
    assert_eq!(
        sql_integer(&connection, "SELECT id FROM source_learner_profiles"),
        1
    );
}

#[test]
fn retained_shared_vocabulary_restricts_deletion_of_its_subject() {
    let (_directory, path, store) = initialized_database();
    store
        .publish(&scope("a"), &metadata(), &[word("x")])
        .unwrap();
    store.publish(&scope("a"), &metadata(), &[]).unwrap();
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    assert_eq!(
        connection
            .execute("DELETE FROM source_subjects", [])
            .unwrap_err()
            .sqlite_error_code(),
        Some(rusqlite::ErrorCode::ConstraintViolation)
    );
    assert_eq!(
        sql_integer(&connection, "SELECT count(*) FROM vocabulary_entries"),
        1
    );
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
        .execute("DELETE FROM learners WHERE id=2", [])
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
    account.external_account_id = Some("account".into());
    store
        .publish(&scope("a"), &account, &[word("x"), word("y")])
        .unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    connection
        .execute(
            "INSERT INTO source_collections (id,provider,name) VALUES (2,'manual','Other')",
            [],
        )
        .unwrap();
    for (sql, expected) in [
        (
            "INSERT INTO source_collections (provider,name,owner_learner_id) VALUES ('manual','Private',999)",
            FK,
        ),
        (
            "UPDATE source_learner_progress SET source_collection_id=2",
            FK,
        ),
        (
            "INSERT INTO source_learner_profiles (learner_id,source_collection_id) VALUES (1,1)",
            UNIQUE,
        ),
        (
            "INSERT INTO source_learner_profiles (learner_id,source_collection_id,external_account_id) VALUES (1,1,'different')",
            UNIQUE,
        ),
        ("UPDATE source_collections SET name=NULL", NULL),
        (
            "INSERT INTO source_subjects (source_collection_id,external_id) VALUES (999,'z')",
            FK,
        ),
        (
            "INSERT INTO source_learner_profiles (id,learner_id,source_collection_id) VALUES (999,999,1)",
            FK,
        ),
        (
            "INSERT INTO source_learner_progress (source_learner_profile_id,source_subject_id,source_collection_id,is_hidden) VALUES (999,1,1,0)",
            FK,
        ),
        (
            "INSERT INTO vocabulary_entry_parts_of_speech VALUES (999,0,'missing')",
            FK,
        ),
        (
            "UPDATE source_profile_subjects SET source_collection_id=2",
            FK,
        ),
        ("UPDATE source_subjects SET material_type='grammar'", FK),
        ("DELETE FROM learners WHERE id=1", FK),
        ("DELETE FROM source_collections WHERE id=1", FK),
        (
            "INSERT INTO source_subjects (source_collection_id,external_id) VALUES (1,'x')",
            UNIQUE,
        ),
        ("UPDATE source_profile_subjects SET position=0", UNIQUE),
        ("UPDATE vocabulary_entries SET id=1", PK),
        (
            "INSERT INTO vocabulary_entry_parts_of_speech VALUES (1,0,'duplicate')",
            PK,
        ),
        (
            "INSERT INTO source_learner_profiles (id,learner_id,source_collection_id,external_account_id) VALUES (999,1,1,'account')",
            UNIQUE,
        ),
        ("INSERT INTO learners (name) VALUES (NULL)", NULL),
        (
            "INSERT INTO source_subjects (source_collection_id,external_id) VALUES (1,NULL)",
            NULL,
        ),
        ("UPDATE vocabulary_entries SET spelling=NULL", NULL),
        ("UPDATE vocabulary_entries SET reading=NULL", NULL),
        ("UPDATE vocabulary_entries SET mapping_method=NULL", NULL),
        (
            "UPDATE vocabulary_translations SET language_code=NULL",
            NULL,
        ),
        (
            "INSERT INTO vocabulary_translations (vocabulary_entry_id,language_code,text,display_order,origin) VALUES (999,'en','missing',0,'explicit')",
            FK,
        ),
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
    second.learner_id = 1;
    assert!(matches!(
        store.publish(&second, &metadata(), &[]),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    second.source_learner_profile_id = 5;
    assert!(matches!(
        store.publish(&second, &metadata(), &[]),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    second.source_collection_id = 2;
    second.source_learner_profile_id = 6;
    let mut separate = word("x");
    if let MappedContent::Vocabulary { entries, .. } = &mut separate.content {
        entries[0].id = 11;
        entries[0].translations[0].id = 110;
    }
    store.publish(&second, &metadata(), &[separate]).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM vocabulary_entries v JOIN source_subjects s ON s.id=v.source_subject_id WHERE s.external_id='x'",
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
        unresolved("z"),
        unresolved("a"),
        unresolved("none"),
        unresolved("observed"),
        unresolved("hidden"),
        unresolved("passed"),
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
    let (_directory, _path, store) = initialized_database();
    store
        .publish(&scope("a"), &metadata(), &[word("x")])
        .unwrap();
    let previous = facts(&store.read(&scope("a")).unwrap());
    let mut invalid = word("x");
    invalid.progress = None;
    if let MappedContent::Vocabulary { entries, .. } = &mut invalid.content {
        entries[0].spelling = "x".repeat(257);
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
    changed.provider = Provider::Wanikani;
    assert!(matches!(
        store.publish(&scope("a"), &changed, &[]),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    changed = metadata();
    changed.external_account_id = Some("account".into());
    assert!(matches!(
        store.publish(&scope("a"), &changed, &[]),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    let mut wrong_scope = scope("a");
    wrong_scope.learner_id = 999;
    assert!(matches!(
        store.publish(&wrong_scope, &metadata(), &[]),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    assert!(matches!(
        store.read(&wrong_scope),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    wrong_scope = scope("a");
    wrong_scope.source_collection_id = 999;
    assert!(matches!(
        store.publish(&wrong_scope, &metadata(), &[]),
        Err(SqliteKnowledgeError::BindingMismatch)
    ));
    assert_eq!(facts(&store.read(&scope("a")).unwrap()), previous);
}

#[test]
fn vocabulary_entries_have_generated_type_and_multiple_entries_per_subject() {
    let (_directory, path, store) = initialized_database();
    store.publish(&scope("a"), &metadata(), &[]).unwrap();
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    connection.execute("INSERT INTO source_subjects (source_collection_id,external_id,material_type) VALUES (1,'x','vocabulary'),(1,'grammar','grammar'),(1,'unknown',NULL)", []).unwrap();
    connection.execute("INSERT INTO vocabulary_entries (source_subject_id,spelling,reading,mapping_method) VALUES (1,'猫','ねこ','explicit-association'),(1,'猫','ネコ','explicit-association')", []).unwrap();
    assert_eq!(
        sql_integer(
            &connection,
            "SELECT count(*) FROM vocabulary_entries WHERE source_subject_id=1 AND material_type='vocabulary'"
        ),
        2
    );
    for subject in [2, 3, 999] {
        assert_eq!(connection.execute("INSERT INTO vocabulary_entries (source_subject_id,spelling,reading,mapping_method) VALUES (?1,'猫','ねこ','explicit-association')", [subject]).unwrap_err().sqlite_error_code(), Some(rusqlite::ErrorCode::ConstraintViolation));
    }
    assert!(
        connection
            .execute("UPDATE vocabulary_entries SET material_type='grammar'", [])
            .is_err()
    );
    connection.execute("INSERT INTO vocabulary_translations (vocabulary_entry_id,language_code,text,display_order,origin) VALUES (1,'en','cat',0,'explicit-association')", []).unwrap();
    assert_eq!(connection.execute("INSERT INTO vocabulary_translations (vocabulary_entry_id,language_code,text,display_order,origin) VALUES (1,'en','duplicate',0,'explicit-association')", []).unwrap_err().sqlite_error_code(), Some(rusqlite::ErrorCode::ConstraintViolation));
    connection
        .execute("DELETE FROM vocabulary_entries WHERE id=1", [])
        .unwrap();
    assert_eq!(
        sql_integer(&connection, "SELECT count(*) FROM vocabulary_translations"),
        0
    );
    assert_eq!(
        sql_integer(&connection, "SELECT count(*) FROM vocabulary_entries"),
        1
    );
}

#[test]
fn evidenced_entries_round_trip_with_their_own_translations_and_stable_ids() {
    let (_directory, path, store) = initialized_database();
    let mut record = word("x");
    if let MappedContent::Vocabulary { entries, .. } = &mut record.content {
        entries[0].translations.insert(
            0,
            VocabularyTranslation {
                id: 11,
                language_code: "en".into(),
                text: "domestic cat".into(),
                display_order: 1,
                is_primary_in_source: Some(false),
                origin: "explicit-fixture-association".into(),
            },
        );
        entries.insert(
            0,
            VocabularyEntry {
                id: 13,
                spelling: "猫".into(),
                reading: "ネコ".into(),
                parts_of_speech: vec![],
                is_primary_reading_in_source: Some(false),
                mapping_method: "explicit-second-association".into(),
                has_direct_object_evidence: None,
                translations: vec![VocabularyTranslation {
                    id: 130,
                    language_code: "en".into(),
                    text: "kitty".into(),
                    display_order: 0,
                    is_primary_in_source: Some(true),
                    origin: "explicit-second-association".into(),
                }],
            },
        );
    }
    store.publish(&scope("a"), &metadata(), &[record]).unwrap();
    let stored = store.read(&scope("a")).unwrap();
    let expected = facts(&stored);
    let MappedContent::Vocabulary { entries, .. } = &stored.records[0].content else {
        panic!("resolved entries")
    };
    assert_eq!(
        entries.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        [1, 13]
    );
    assert_eq!(
        entries[0]
            .translations
            .iter()
            .map(|translation| translation.text.as_str())
            .collect::<Vec<_>>(),
        ["cat", "domestic cat"]
    );
    assert_eq!(entries[1].translations[0].text, "kitty");
    let mut reordered = stored.records;
    if let MappedContent::Vocabulary { entries, .. } = &mut reordered[0].content {
        entries.reverse();
        entries[1].translations.reverse();
    }
    let direct = prepare(
        StoredKnowledge {
            scope: scope("a"),
            metadata: metadata(),
            records: reordered,
        },
        LearnerKnowledgePolicy::LessonStarted,
    );
    assert_eq!(
        direct,
        prepare(
            store.read(&scope("a")).unwrap(),
            LearnerKnowledgePolicy::LessonStarted
        )
    );
    let stored = store.read(&scope("a")).unwrap();
    store
        .publish(&scope("a"), &metadata(), &stored.records)
        .unwrap();
    assert_eq!(facts(&store.read(&scope("a")).unwrap()), expected);
    drop(store);
    assert_eq!(
        facts(&SqliteKnowledgeStore::new(path).read(&scope("a")).unwrap()),
        expected
    );
}

#[test]
fn unknown_material_type_and_unresolved_observations_survive_evidenced_publication() {
    let (_directory, path, store) = initialized_database();
    let unavailable = MappedKnowledgeRecord {
        content: MappedContent::Unavailable(SourceOrigin::Manual),
        ..word("x")
    };
    store
        .publish(&scope("a"), &metadata(), &[unavailable])
        .unwrap();
    let connection = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT material_type FROM source_subjects", [], |row| {
                row.get::<_, Option<String>>(0)
            })
            .unwrap(),
        None
    );
    store
        .publish(&scope("b"), &metadata(), &[unresolved("x")])
        .unwrap();
    let observation: String = connection
        .query_row("SELECT source_data FROM source_subjects", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        sql_integer(&connection, "SELECT count(*) FROM vocabulary_entries"),
        0
    );
    assert!(matches!(
        store.read(&scope("a")).unwrap().records[0].content,
        MappedContent::Unavailable(_)
    ));
    store
        .publish(&scope("a"), &metadata(), &[word("x")])
        .unwrap();
    assert_eq!(
        connection
            .query_row("SELECT source_data FROM source_subjects", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        observation
    );
    assert!(matches!(
        store.read(&scope("b")).unwrap().records[0].content,
        MappedContent::UnresolvedVocabulary(_)
    ));
    assert!(matches!(
        store.read(&scope("a")).unwrap().records[0].content,
        MappedContent::Vocabulary { .. }
    ));
    store.publish(&scope("a"), &metadata(), &[]).unwrap();
    assert_eq!(
        sql_integer(&connection, "SELECT count(*) FROM vocabulary_entries"),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT source_data FROM source_subjects", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        observation
    );
}

#[test]
fn unresolved_data_is_decoded_strictly_and_all_vocabulary_is_validated_before_writes() {
    for corruption in [
        "UPDATE source_subjects SET source_data=NULL",
        "UPDATE source_subjects SET source_data='not JSON'",
        "UPDATE source_subjects SET source_data=json_set(source_data,'$.word.direct_object',2)",
        "UPDATE source_subjects SET source_data=json_set(source_data,'$.word.origin.kind','wanikani')",
        "UPDATE source_subjects SET source_data=json_set(source_data,'$.word.readings',json('[1]'))",
        "UPDATE source_subjects SET source_data=json_set(source_data,'$.extra',1)",
        "UPDATE source_subjects SET material_type=NULL",
    ] {
        let (_directory, path, store) = initialized_database();
        store
            .publish(&scope("a"), &metadata(), &[unresolved("x")])
            .unwrap();
        let connection = rusqlite::Connection::open(path).unwrap();
        connection.execute(corruption, []).unwrap();
        assert!(
            matches!(
                store.read(&scope("a")),
                Err(SqliteKnowledgeError::Corrupt(_))
            ),
            "{corruption}"
        );
    }
    let (_directory, path, store) = initialized_database();
    store
        .publish(&scope("a"), &metadata(), &[unresolved("x")])
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut invalid = unresolved("x");
    invalid.progress = None;
    if let MappedContent::UnresolvedVocabulary(unresolved) = &mut invalid.content {
        unresolved.word.written_form = "x".repeat(257);
    }
    assert!(matches!(
        store.publish(&scope("a"), &metadata(), &[invalid]),
        Err(SqliteKnowledgeError::Inventory(_))
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn unresolved_word_id_duplicates_fail_publication_and_scoped_read() {
    let (_directory, path, store) = initialized_database();
    let mut records = [unresolved("x"), unresolved("y")];
    store.publish(&scope("a"), &metadata(), &records).unwrap();
    let before = std::fs::read(&path).unwrap();
    if let MappedContent::UnresolvedVocabulary(unresolved) = &mut records[1].content {
        unresolved.word.id = "word:x".into();
    }
    records[1].progress = None;
    assert!(matches!(
        store.publish(&scope("a"), &metadata(), &records),
        Err(SqliteKnowledgeError::InvalidInput(_))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let connection = rusqlite::Connection::open(path).unwrap();
    connection.execute("UPDATE source_subjects SET source_data=json_set(source_data,'$.word.id','word:x') WHERE external_id='y'", []).unwrap();
    assert!(matches!(
        store.read(&scope("a")),
        Err(SqliteKnowledgeError::Corrupt(_))
    ));
}
