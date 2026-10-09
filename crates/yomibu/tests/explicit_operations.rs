use std::{collections::BTreeMap, path::Path};
use yomibu::{
    application::{ApplicationError, Credentials, LocalApp, Operation},
    configuration::{
        Configuration, ConfigurationInput, Patch, ProcessOverrides, components::GENERATION_MODEL,
    },
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

#[test]
fn offline_status_and_preview_never_read_the_credential_store() {
    let dir = tempfile::tempdir().unwrap();
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let mut flags = ProcessOverrides::default();
    flags.application.inventory = Some(inventory);
    let config = load_config(&dir.path().join("data"), &Operation::Story, flags);
    let mut credentials = Credentials::default();
    for key in yomibu::configuration::components::credentials() {
        credentials.supply_with(key, || panic!("Offline command read the credential store"));
    }
    let app = LocalApp::new(config, credentials);
    assert!(matches!(app.status(), Err(ApplicationError::Cache(_))));
    assert!(app.preview(std::time::SystemTime::now().into(), 7).is_ok());
    assert!(!dir.path().join("data").exists());
}

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
    let mut flags = ProcessOverrides::default();
    flags.invocation.pipeline.embedding.provider =
        Patch::Set(yomibu::configuration::EmbeddingProvider::LexicalBaseline);
    let config = load_config(dir.path(), &Operation::Story, flags);
    let app = LocalApp::new(config, Credentials::default());
    assert!(matches!(
        app.preview(std::time::SystemTime::now().into(), 1),
        Err(yomibu::application::ApplicationError::AccessExpired)
    ));
    assert!(matches!(
        app.prepare_retrieval(std::time::SystemTime::now().into())
            .await,
        Err(yomibu::application::ApplicationError::AccessExpired)
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
    let mut flags = ProcessOverrides::default();
    flags.application.inventory = Some(inventory);
    flags
        .invocation
        .pipeline
        .options
        .set(GENERATION_MODEL, "chosen".into())
        .unwrap();
    let config = load_config(&dir.path().join("data"), &Operation::Story, flags);
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

#[tokio::test]
async fn story_checks_request_before_inventory_but_offline_operations_check_inventory_first() {
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
    // SAFETY: Invalid inputs stop the workflow before dictionary loading.
    assert!(matches!(
        unsafe { app.story(&store, now, 1, |_| {},) }.await,
        Err(ApplicationError::InvalidJson {
            kind: "story request",
            ..
        })
    ));
    assert!(matches!(
        app.preview(now, 1),
        Err(ApplicationError::InvalidJson {
            kind: "manual inventory",
            ..
        })
    ));
    assert!(matches!(
        app.prepare_retrieval(now).await,
        Err(ApplicationError::InvalidJson {
            kind: "manual inventory",
            ..
        })
    ));
    assert!(!data.exists());
}

#[test]
fn saved_selection_and_story_targets_drive_the_existing_preview_workflow() {
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
    std::fs::write(dir.path().join("default-pipeline.toml"), "[pipeline.selection]\nsteps=['seeded-order']\nembedding_steps=['embedding-rank','seeded-order']\nembeddings=false\n[pipeline.assessment]\nenabled=false\n").unwrap();
    std::fs::write(
        dir.path().join("default-story.toml"),
        "[story]\ntopic={clear=true}\nseed=7\n[story.targets]\nvocabulary=['cat']\ngrammar=[]\n",
    )
    .unwrap();
    let config = load_config(dir.path(), &Operation::Preview, ProcessOverrides::default());
    let report = LocalApp::new(config, Credentials::default())
        .preview(std::time::SystemTime::now().into(), 42)
        .unwrap();
    assert_eq!(report.selection.selector_revision, "seeded-only-v1");
    assert_eq!(report.request.targets.vocabulary, ["cat"]);
    assert_eq!(report.selection.vocabulary_ids[0], "cat");
    assert!(report.request.topic.is_none());
}

#[test]
fn typed_invocations_replace_defaults_without_mutating_the_application() {
    use yomibu::configuration::{Invocation, Patch};
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("inventory.json"),
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "[application]\ninventory='inventory.json'\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("default-story.toml"),
        "[story]\ntopic='cat'\nseed=7\n[story.targets]\nvocabulary=['cat']\ngrammar=[]\n",
    )
    .unwrap();
    let config = load_config(dir.path(), &Operation::Preview, ProcessOverrides::default());
    let app = LocalApp::new(config, Credentials::default());
    let mut request = Invocation::default();
    request.story.topic = Patch::Clear;
    request.story.targets.vocabulary = Some(vec!["dog".into()]);
    request.story.seed = Patch::Set(42);
    let invocation = app
        .for_invocation(request, &yomibu::application::Operation::Preview)
        .unwrap();
    let now = std::time::SystemTime::now().into();
    let changed = invocation.preview(now, 1).unwrap();
    let original = app.preview(now, 1).unwrap();
    assert!(changed.request.topic.is_none());
    assert_eq!(changed.request.targets.vocabulary, ["dog"]);
    assert_eq!(original.request.topic.unwrap().text(), "cat");
    assert_eq!(original.request.targets.vocabulary, ["cat"]);
}

#[test]
fn request_file_topic_omission_inherits_but_null_clears_saved_topic() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("inventory.json"),
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("default-story.toml"),
        "[story]\ntopic='saved'\n[story.targets]\nvocabulary=['cat']\ngrammar=[]\n",
    )
    .unwrap();
    for (topic, expected) in [
        ("", Some("saved")),
        (",\"topic\":null", None),
        (",\"topic\":\"invoked\"", Some("invoked")),
    ] {
        let request = dir.path().join("request.json");
        std::fs::write(
            &request,
            format!("{{\"version\":1,\"targets\":{{\"vocabulary\":[],\"grammar\":[]}}{topic}}}"),
        )
        .unwrap();
        let mut flags = ProcessOverrides {
            request: Some(request),
            ..Default::default()
        };
        flags.application.inventory = Some(dir.path().join("inventory.json"));
        let config = load_config(dir.path(), &Operation::Preview, flags);
        let preview = LocalApp::new(config, Credentials::default())
            .preview(std::time::SystemTime::now().into(), 7)
            .unwrap();
        assert_eq!(
            preview.request.topic.as_ref().map(|topic| topic.text()),
            expected
        );
        assert!(preview.request.targets.vocabulary.is_empty());
    }
}

#[test]
fn missing_optional_embeddings_fall_back_to_the_configured_selector() {
    let dir = tempfile::tempdir().unwrap();
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("default-pipeline.toml"),
        "[pipeline.selection]\nsteps=['seeded-order']\nembeddings=true\n",
    )
    .unwrap();
    let mut flags = ProcessOverrides::default();
    flags.application.inventory = Some(inventory);
    flags.invocation.story.topic = Patch::Set("cat".into());
    let config = load_config(dir.path(), &Operation::Preview, flags);
    let preview = LocalApp::new(config, Credentials::default())
        .preview(std::time::SystemTime::now().into(), 7)
        .unwrap();
    assert_eq!(preview.selection.selector_revision, "seeded-only-v1");
    assert_eq!(preview.warnings.len(), 1);
    assert!(preview.warnings[0].message.contains("seeded-only-v1"));
    assert!(!dir.path().join("embeddings.json").exists());
}
