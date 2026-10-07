use std::collections::BTreeMap;
use yomibu::app::{
    config::{Configuration, ConfigurationInput, Settings},
    local::{ApplicationError, Credentials, LocalApp},
};

#[tokio::test]
async fn offline_preview_and_retrieval_cannot_use_expired_source_content() {
    let dir = tempfile::tempdir().unwrap();
    let mut cache: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../tests/fixtures/mixed.json")).unwrap();
    cache["snapshot"]["learner"]["subscription"]["active"] = serde_json::json!(true);
    cache["snapshot"]["learner"]["subscription"]["period_ends_at"] =
        serde_json::json!("1900-01-01T00:00:00Z");
    std::fs::write(
        dir.path().join("wanikani.json"),
        serde_json::to_vec(&cache).unwrap(),
    )
    .unwrap();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: Settings {
                embedding_provider: Some(yomibu::app::config::EmbeddingProvider::LexicalBaseline),
                ..Default::default()
            },
        },
        &yomibu::app::Operation::Story,
    )
    .unwrap();
    let app = LocalApp::new(config, Credentials::default());
    assert!(matches!(
        app.preview(std::time::SystemTime::now().into(), 1),
        Err(yomibu::app::local::ApplicationError::AccessExpired)
    ));
    assert!(matches!(
        app.prepare_retrieval(std::time::SystemTime::now().into())
            .await,
        Err(yomibu::app::local::ApplicationError::AccessExpired)
    ));
    assert!(app.status().is_ok());
    assert!(!dir.path().join("embeddings.json").exists());
}

#[tokio::test]
async fn offline_preview_uses_manual_inventory_and_explicit_sync_requires_a_key() {
    let dir = tempfile::tempdir().unwrap();
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().join("data")),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: Settings {
                inventory: Some(inventory),
                generation_model: Some("chosen".into()),
                ..Default::default()
            },
        },
        &yomibu::app::Operation::Story,
    )
    .unwrap();
    let app = LocalApp::new(config, Credentials::default());
    let preview = app.preview(std::time::SystemTime::now().into(), 7).unwrap();
    assert!(preview.request.topic.is_none());
    let body: serde_json::Value =
        serde_json::from_str(preview.provider_request.body_utf8()).unwrap();
    assert_eq!(body["model"], "chosen");
    assert!(matches!(
        app.sync().await,
        Err(ApplicationError::Setup { .. })
    ));
    assert!(!dir.path().join("data").exists());
}
