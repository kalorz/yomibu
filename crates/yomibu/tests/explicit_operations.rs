use std::{collections::BTreeMap, path::Path};
use yomibu::{
    application::{ApplicationError, Credentials, LocalApp, Operation},
    configuration::{Configuration, ConfigurationInput, ProcessOverrides},
};

fn load_config(dir: &Path, operation: &Operation, flags: ProcessOverrides) -> Configuration {
    Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags,
        },
        operation,
    )
    .unwrap()
}

#[tokio::test]
async fn story_checks_request_before_inventory() {
    let dir = tempfile::tempdir().unwrap();
    let inventory = dir.path().join("inventory.json");
    let request = dir.path().join("request.json");
    std::fs::write(&inventory, "{").unwrap();
    std::fs::write(&request, "{").unwrap();
    let data = dir.path().join("data");
    let mut flags = ProcessOverrides {
        request: Some(request),
        ..Default::default()
    };
    flags.application.inventory = Some(inventory);
    let config = load_config(&data, &Operation::Story, flags);
    let app = LocalApp::new(config, Credentials::default());
    let now = std::time::SystemTime::now().into();
    let store = yomibu_components::file_learning_store::FileLearningStore::new(&data);
    assert!(matches!(
        { app.story(&store, now, 1, |_| {},) }.await,
        Err(ApplicationError::InvalidJson {
            kind: "story request",
            ..
        })
    ));
    assert!(!data.exists());
}

#[tokio::test]
async fn saved_selection_and_story_targets_drive_generation() {
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};
    use yomibu::application::ServiceEndpoints;
    use yomibu::configuration::components::GENERATION_KEY;
    use yomibu_components::file_learning_store::FileLearningStore;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("inventory.json"),
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "[application]\ninventory='inventory.json'\nsync=false\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("default-pipeline.toml"),
        "[pipeline.selection]\nsteps=['seeded-order']\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("default-story.toml"),
        "[story]\ntopic={clear=true}\nseed=7\n[story.targets]\nvocabulary=['cat']\ngrammar=[]\n",
    )
    .unwrap();
    let server = MockServer::start().await;
    Mock::given(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id":"synthetic", "model":"returned", "status":"completed",
            "output":[{"type":"message", "role":"assistant", "status":"completed",
                "content":[{"type":"output_text", "text":serde_json::json!({"candidates":[{"sentences":["猫です。","朝です。","寝ます。"]}]}).to_string()}]}]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let config = load_config(dir.path(), &Operation::Story, ProcessOverrides::default());
    let mut credentials = Credentials::default();
    credentials.supply(GENERATION_KEY, "synthetic-key".into());
    let report = LocalApp::new(config, credentials)
        .with_endpoints(ServiceEndpoints {
            openai: format!("{}/v1/", server.uri()),
            ..Default::default()
        })
        .story(
            &FileLearningStore::new(dir.path()),
            std::time::SystemTime::now().into(),
            42,
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(report.selection.selector_revision, "seeded-only-v1");
    assert_eq!(report.request.targets.vocabulary, ["cat"]);
    assert_eq!(report.selection.vocabulary_ids[0], "cat");
    assert_eq!(report.selection.seed, 7);
    assert!(report.request.topic.is_none());
    assert!(!dir.path().join("wanikani.json").exists());
}
