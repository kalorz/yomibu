use super::*;

#[test]
fn adapter_connections_enforce_foreign_keys_and_readers_hold_one_snapshot_until_publication() {
    let directory = tempfile::tempdir().unwrap();
    let store = SqliteKnowledgeStore::new(directory.path().join("knowledge.sqlite"));
    store.initialize().unwrap();
    let scope = KnowledgeScope {
        learner_id: "learner".into(),
        collection_id: "manual".into(),
        profile_id: "profile".into(),
    };
    let mut metadata = ScopeMetadata {
        source_kind: SourceKind::Manual,
        provider_account_id: None,
        access_expires_at: None,
    };
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
    assert_eq!(
        publication
            .execute_batch("COMMIT")
            .unwrap_err()
            .sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy)
    );
    let old = read_scope(&snapshot, &scope).unwrap();
    assert_eq!(old.metadata.access_expires_at, None);
    assert!(matches!(
        old.records[0].content,
        MappedContent::NonVocabulary(_)
    ));
    assert_eq!(old.records[0].progress, None);
    drop(snapshot);
    publication.commit().unwrap();
    let new = store.read(&scope).unwrap();
    assert_eq!(new.metadata, metadata);
    assert!(matches!(
        new.records[0].content,
        MappedContent::Unavailable(_)
    ));
    assert_eq!(new.records[0].progress, records[0].progress);
}
