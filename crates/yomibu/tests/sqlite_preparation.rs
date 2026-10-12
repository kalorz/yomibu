use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};
use yomibu::{
    application::{
        KnowledgePreparationError, Operation, StoryInputs, StoryResources,
        prepare_stored_inventory, run_story,
    },
    configuration::{Configuration, ProcessOverrides, modules::ModuleId},
};
use yomibu_components::{
    in_memory_learning_store::InMemoryLearningStore,
    openai_story_generation::Client,
    sqlite_knowledge_store::{
        KnowledgeScope, MappedKnowledgeRecord, Provider, ScopeMetadata, SqliteKnowledgeStore,
    },
    wanikani_source::map_content_and_progress,
};
use yomibu_core::domain::{
    inventory::{LearnerInventory, MappedContent, SourceOrigin},
    knowledge::LearnerKnowledgePolicy,
    source::WaniKaniSyncData,
    story::StoryRequest,
};

fn published() -> (
    tempfile::TempDir,
    SqliteKnowledgeStore,
    KnowledgeScope,
    Value,
    chrono::DateTime<chrono::Utc>,
) {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/mixed.json")).unwrap();
    let source: WaniKaniSyncData = serde_json::from_value(fixture["snapshot"].clone()).unwrap();
    let policy = LearnerKnowledgePolicy::LessonStarted;
    let expected = serde_json::to_value(
        LearnerInventory::from_mapped(map_content_and_progress(&source).unwrap(), &policy).unwrap(),
    )
    .unwrap();
    let scope = KnowledgeScope {
        learner_id: 1,
        source_collection_id: 1,
        source_learner_profile_id: 1,
    };
    let metadata = ScopeMetadata {
        provider: Provider::Wanikani,
        learner_name: "Fixture learner".into(),
        source_collection_name: "WaniKani".into(),
        owner_learner_id: None,
        external_account_id: Some(source.learner.id.clone()),
        access_expires_at: source
            .learner
            .subscription
            .active
            .then_some(source.learner.subscription.period_ends_at)
            .flatten(),
    };
    let records: Vec<_> = map_content_and_progress(&source)
        .unwrap()
        .map(|(content, progress)| {
            let origin = content.origin();
            let SourceOrigin::Wanikani { subject_id } = origin else {
                panic!("WaniKani fixture")
            };
            MappedKnowledgeRecord {
                external_id: subject_id.to_string(),
                content,
                progress,
            }
        })
        .collect();
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("knowledge.sqlite");
    let store = SqliteKnowledgeStore::new(&database);
    store.initialize().unwrap();
    store.publish(&scope, &metadata, &records).unwrap();
    let connection = rusqlite::Connection::open(&database).unwrap();
    let aggregate_tables: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name IN ('source_vocabulary','source_vocabulary_readings','source_vocabulary_meanings')",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(aggregate_tables, 0);
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM vocabulary_entries", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let unresolved: String = connection
        .query_row(
            "SELECT source_data FROM source_subjects WHERE external_id='3'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let unresolved: Value = serde_json::from_str(&unresolved).unwrap();
    assert_eq!(
        unresolved["observation"],
        serde_json::to_value(
            source
                .subjects
                .iter()
                .find(|subject| subject.id == 3)
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(unresolved["word"]["id"], "wanikani:3");
    drop(connection);
    drop(store);
    (
        directory,
        SqliteKnowledgeStore::new(database),
        scope,
        expected,
        source.sync_completed_at,
    )
}

#[test]
fn expired_metadata_is_readable_but_preparation_enforces_the_boundary() {
    let (_directory, store, scope, expected, now) = published();
    let stored = store.read(&scope).unwrap();
    store
        .publish(
            &scope,
            &ScopeMetadata {
                access_expires_at: Some(now),
                ..stored.metadata
            },
            &stored.records,
        )
        .unwrap();
    assert_eq!(
        store.read(&scope).unwrap().metadata.access_expires_at,
        Some(now)
    );
    let policy = LearnerKnowledgePolicy::LessonStarted;
    for time in [now, now + chrono::Duration::seconds(1)] {
        assert!(matches!(
            prepare_stored_inventory(store.read(&scope).unwrap(), &policy, time),
            Err(KnowledgePreparationError::AccessExpired)
        ));
    }
    assert_eq!(
        serde_json::to_value(
            prepare_stored_inventory(
                store.read(&scope).unwrap(),
                &policy,
                now - chrono::Duration::seconds(1)
            )
            .unwrap()
        )
        .unwrap(),
        expected
    );
}

#[tokio::test]
async fn reopened_sqlite_knowledge_generates_through_supplied_inventory_without_source_requests() {
    let (directory, store, scope, expected, now) = published();
    let stored = store.read(&scope).unwrap();
    assert!(
        stored
            .records
            .iter()
            .any(|record| matches!(record.content, MappedContent::UnresolvedVocabulary(_)))
    );
    assert!(
        !stored
            .records
            .iter()
            .any(|record| matches!(record.content, MappedContent::Vocabulary { .. }))
    );
    assert_ne!(
        stored.scope.learner_id.to_string(),
        stored
            .metadata
            .external_account_id
            .as_ref()
            .unwrap()
            .as_str()
    );
    drop(store);
    let inventory =
        prepare_stored_inventory(stored, &LearnerKnowledgePolicy::LessonStarted, now).unwrap();
    assert_eq!(serde_json::to_value(&inventory).unwrap(), expected);
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id":"synthetic", "model":"test", "status":"completed",
            "output":[{"type":"message","role":"assistant","status":"completed","content":[{
                "type":"output_text","text":json!({"candidates":[
                    {"sentences":["これです。","これです。","これです。"]}
                ]}).to_string()
            }]}]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let client = Client::with_base_url("synthetic", &format!("{}/v1/", server.uri())).unwrap();
    let config = Configuration::from_overrides(
        directory.path().into(),
        ProcessOverrides {
            disable: vec![ModuleId::Embeddings, ModuleId::Assessment],
            ..Default::default()
        },
        &Operation::Story,
    )
    .unwrap();
    let request: StoryRequest =
        serde_json::from_value(json!({"version":1,"targets":{"vocabulary":[],"grammar":[]}}))
            .unwrap();
    let database = directory.path().join("knowledge.sqlite");
    let before = std::fs::read(&database).unwrap();
    let report = run_story::<InMemoryLearningStore>(
        &config,
        StoryInputs {
            request,
            manual: Some(inventory),
        },
        StoryResources {
            store: None,
            source_client: None,
            generation_client: &client,
            analyzer: None,
            embedding_cache: None,
        },
        now,
        7,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(report.selection.vocabulary_ids, ["wanikani:3"]);
    assert_eq!(
        report.generated.passages()[0].text,
        "これです。これです。これです。"
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/v1/responses");
    assert_eq!(requests[0].method.as_str(), "POST");
    assert_eq!(std::fs::read(database).unwrap(), before);
}
