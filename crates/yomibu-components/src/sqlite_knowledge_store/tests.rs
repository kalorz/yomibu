use super::*;

fn scope() -> KnowledgeScope {
    KnowledgeScope {
        learner_id: "learner".into(),
        source_collection_id: "manual".into(),
        source_learner_profile_id: "profile".into(),
    }
}

fn metadata() -> ScopeMetadata {
    ScopeMetadata {
        provider: Provider::Manual,
        external_account_id: None,
        access_expires_at: None,
    }
}

#[test]
fn initialization_rechecks_journaling_after_acquiring_the_write_transaction() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("knowledge.sqlite");
    let mut initializer = Connection::open(&path).unwrap();
    initializer
        .pragma_update(None, "journal_mode", "DELETE")
        .unwrap();
    let external = Connection::open(&path).unwrap();
    external.pragma_update(None, "journal_mode", "WAL").unwrap();
    drop(external);
    let transaction = initializer
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    assert!(matches!(
        schema_state(&transaction).unwrap(),
        SchemaState::Empty
    ));
    assert!(matches!(
        require_delete_journal(&transaction),
        Err(SqliteKnowledgeError::UnsupportedJournalMode)
    ));
    drop(transaction);
    assert!(matches!(
        schema_state(&initializer).unwrap(),
        SchemaState::Empty
    ));
}

#[test]
fn reads_recheck_journaling_within_their_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("knowledge.sqlite");
    let store = SqliteKnowledgeStore::new(&path);
    store.initialize().unwrap();
    let scope = scope();
    store.publish(&scope, &metadata(), &[]).unwrap();
    let mut reader = store.open(OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let external = Connection::open(&path).unwrap();
    external.pragma_update(None, "journal_mode", "WAL").unwrap();
    drop(external);
    let snapshot = reader.transaction().unwrap();
    require_schema(&snapshot).unwrap();
    assert!(matches!(
        require_delete_journal(&snapshot),
        Err(SqliteKnowledgeError::UnsupportedJournalMode)
    ));
}

#[test]
fn adapter_connections_enforce_foreign_keys_and_readers_hold_one_snapshot_until_publication() {
    let directory = tempfile::tempdir().unwrap();
    let store = SqliteKnowledgeStore::new(directory.path().join("knowledge.sqlite"));
    store.initialize().unwrap();
    let scope = scope();
    let mut metadata = metadata();
    let mut records = [MappedKnowledgeRecord {
        external_id: "subject".into(),
        content: MappedContent::NonVocabulary(SourceOrigin::Manual),
        progress: None,
    }];
    store.publish(&scope, &metadata, &records).unwrap();
    let mut reader = store.open(OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut writer = store.open(OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    for connection in [&reader, &writer] {
        assert_eq!(
            connection
                .pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    assert_eq!(
        writer
            .pragma_query_value(None, "synchronous", |row| row.get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        writer
            .pragma_query_value(None, "fullfsync", |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        writer
            .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
            .unwrap(),
        "delete"
    );
    assert_eq!(
        reader
            .execute("DELETE FROM learners", [])
            .unwrap_err()
            .sqlite_error_code(),
        Some(rusqlite::ErrorCode::ReadOnly)
    );
    let snapshot = reader.transaction().unwrap();
    assert_eq!(read_metadata(&snapshot, &scope).unwrap(), metadata);
    writer.busy_timeout(Duration::ZERO).unwrap();
    let publication = writer
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    metadata.access_expires_at = DateTime::from_timestamp(1, 0);
    records[0].content = MappedContent::Unavailable(SourceOrigin::Manual);
    records[0].progress = Some(LearnerProgress {
        is_hidden: true,
        ..Default::default()
    });
    publish_scope(&publication, &scope, &metadata, &records).unwrap();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(directory.path().join("knowledge.sqlite-journal"))
            .unwrap()
            .permissions()
            .mode()
            & 0o077,
        0
    );
    assert!(
        matches!(commit_transaction(publication), Err(SqliteKnowledgeError::Database(ref error)) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy))
    );
    let old = read_scope(&snapshot, &scope).unwrap();
    assert_eq!(old.metadata.access_expires_at, None);
    assert!(matches!(
        old.records[0].content,
        MappedContent::NonVocabulary(_)
    ));
    assert_eq!(old.records[0].progress, None);
    drop(snapshot);
    assert_eq!(store.read(&scope).unwrap().metadata.access_expires_at, None);
    store.publish(&scope, &metadata, &records).unwrap();
    let new = store.read(&scope).unwrap();
    assert_eq!(new.metadata, metadata);
    assert!(matches!(
        new.records[0].content,
        MappedContent::Unavailable(_)
    ));
    assert_eq!(new.records[0].progress, records[0].progress);
}
