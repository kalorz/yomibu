use crate::composition::selection::run_story;
use crate::composition::{
    ApplicationError, Credentials, LocalApp, Operation, ServiceEndpoints, SetupIssue,
    configuration::{
        Configuration, ConfigurationInput, Invocation, Patch,
        components::{GENERATION_KEY, GENERATION_MODEL, SOURCE_KEY},
    },
};
use serde_json::json;
use std::{collections::BTreeMap, time::SystemTime};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};
use yomibu_app::application::generate_story::{StoryInputs, progress::RunProgress};
use yomibu_core::{
    domain::knowledge::inventory::InventoryError, ports::infrastructure::LearningStore,
};
use yomibu_ext::{file_store::FileLearningStore, openai::Client};

fn config(dir: &std::path::Path, flags: Invocation) -> Configuration {
    Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags,
        },
        &crate::composition::Operation::Story,
    )
    .unwrap()
}

async fn mount_generation(server: &MockServer, count: u64) {
    Mock::given(method("POST")).and(path("/v1/responses")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"synthetic", "model":"returned", "status":"completed", "output":[{"type":"message", "role":"assistant", "status":"completed", "content":[{"type":"output_text", "text":json!({"candidates":[{"sentences":["猫です。","寝ます。","朝です。"]}]}).to_string()}]}]}))).expect(count).mount(server).await;
}

fn write_cache(dir: &std::path::Path) -> chrono::DateTime<chrono::Utc> {
    std::fs::write(
        dir.join("wanikani.json"),
        include_bytes!("../../../../../tests/fixtures/mixed.json"),
    )
    .unwrap();
    yomibu_ext::file_store::cache::load(dir)
        .unwrap()
        .sync_completed_at
}
fn endpoints(server: &MockServer) -> ServiceEndpoints {
    ServiceEndpoints {
        wanikani: format!("{}/v2/", server.uri()),
        openai: format!("{}/v1/", server.uri()),
    }
}

fn supplied_credentials(wanikani: Option<String>, openai: Option<String>) -> Credentials {
    let mut credentials = Credentials::default();
    if let Some(value) = wanikani {
        credentials.supply(SOURCE_KEY, value.into());
    }
    if let Some(value) = openai {
        credentials.supply(GENERATION_KEY, value.into());
    }
    credentials
}

#[tokio::test]
async fn missing_setup_is_aggregated_before_network_or_writes_and_secrets_are_redacted() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("fresh");
    let store = FileLearningStore::new(&data);
    let app = LocalApp::new(config(&data, Invocation::default()), Credentials::default());
    let error = {
        app.story(
            store.read_learner_data().unwrap(),
            SystemTime::now().into(),
            7,
            |_| {},
        )
    }
    .await
    .unwrap_err();
    let ApplicationError::Setup { issues } = error else {
        panic!("{error:?}")
    };
    assert!(matches!(issues.as_slice(),
        [SetupIssue::Inventory, SetupIssue::Credential(generation)]
        if generation.name == GENERATION_KEY.name));
    assert!(!data.exists());
    assert!(
        !format!(
            "{:?}",
            supplied_credentials(Some("wk-secret".into()), Some("ai-secret".into()))
        )
        .contains("secret")
    );
}

#[tokio::test]
async fn story_never_resolves_source_credentials_even_for_old_data() {
    let dir = tempfile::tempdir().unwrap();
    let completed = write_cache(dir.path());
    let before = std::fs::read(dir.path().join("wanikani.json")).unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let mut credentials = supplied_credentials(None, Some("ai".into()));
    credentials.supply_with(SOURCE_KEY, || panic!("story resolved source credentials"));
    let app = LocalApp::new(config(dir.path(), Invocation::default()), credentials)
        .with_endpoints(endpoints(&server));
    let report = app
        .story(
            app.read_learner_data().unwrap(),
            completed + chrono::Duration::days(30),
            7,
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(report.generated.passages().len(), 1);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    assert_eq!(
        std::fs::read(dir.path().join("wanikani.json")).unwrap(),
        before
    );
    assert!(!dir.path().join("wanikani.json.lock").exists());
}

#[tokio::test]
async fn story_reads_old_data_even_while_an_explicit_sync_holds_the_writer() {
    let dir = tempfile::tempdir().unwrap();
    let now = write_cache(dir.path());
    let store = FileLearningStore::new(dir.path());
    let _writer = store.begin_sync().unwrap();
    let before = std::fs::read(dir.path().join("wanikani.json")).unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let app = LocalApp::new(
        config(dir.path(), Invocation::default()),
        supplied_credentials(Some("invalid\nsource".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    let report = app
        .story(
            store.read_learner_data().unwrap(),
            now + chrono::Duration::days(30),
            7,
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(report.selection.vocabulary_ids, ["wanikani:3"]);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    assert_eq!(
        std::fs::read(dir.path().join("wanikani.json")).unwrap(),
        before
    );
}

#[tokio::test]
async fn invalid_or_expired_data_fails_before_credentials_or_generation() {
    for mode in ["expired", "empty"] {
        let dir = tempfile::tempdir().unwrap();
        let now = write_cache(dir.path());
        let path = dir.path().join("wanikani.json");
        let mut data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        match mode {
            "expired" => {
                data["snapshot"]["learner"]["subscription"]["active"] = json!(true);
                data["snapshot"]["learner"]["subscription"]["period_ends_at"] = json!(now);
                std::fs::write(&path, data.to_string()).unwrap();
            }
            _ => {
                for assignment in data["snapshot"]["assignments"].as_array_mut().unwrap() {
                    assignment["hidden"] = json!(true);
                }
                std::fs::write(&path, data.to_string()).unwrap();
            }
        }
        let before = std::fs::read(&path).unwrap();
        let mut credentials = Credentials::default();
        credentials.supply_with(GENERATION_KEY, || panic!("invalid data read credentials"));
        let result = LocalApp::new(config(dir.path(), Invocation::default()), credentials)
            .story(
                FileLearningStore::new(dir.path())
                    .read_learner_data()
                    .unwrap(),
                now,
                7,
                |_| {},
            )
            .await;
        assert!(
            match mode {
                "expired" => matches!(
                    result,
                    Err(ApplicationError::Inventory(InventoryError::AccessExpired))
                ),
                _ => matches!(
                    result,
                    Err(ApplicationError::Story(
                        yomibu_core::domain::generation::StoryError::Invalid(
                            "no available vocabulary"
                        )
                    ))
                ),
            },
            "{mode}: {result:?}"
        );
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert!(!dir.path().join("wanikani.json.lock").exists());
    }
}

#[tokio::test]
async fn inactive_subscription_keeps_recorded_free_content_access() {
    let dir = tempfile::tempdir().unwrap();
    let now = write_cache(dir.path());
    let path = dir.path().join("wanikani.json");
    let mut data: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    data["snapshot"]["learner"]["subscription"]["active"] = json!(false);
    data["snapshot"]["learner"]["subscription"]["period_ends_at"] = json!(now);
    std::fs::write(path, data.to_string()).unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    LocalApp::new(
        config(dir.path(), Invocation::default()),
        supplied_credentials(None, Some("ai".into())),
    )
    .with_endpoints(endpoints(&server))
    .story(
        FileLearningStore::new(dir.path())
            .read_learner_data()
            .unwrap(),
        now + chrono::Duration::days(30),
        7,
        |_| {},
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn concurrent_supplied_stories_share_a_client_and_keep_independent_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let now = write_cache(dir.path());
    let server = MockServer::start().await;
    mount_generation(&server, 2).await;
    let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
    let base = config(dir.path(), Invocation::default());
    let run = |topic: &'static str, seed| {
        let base = &base;
        let client = &client;
        let dir = &dir;
        async move {
            let mut invocation = Invocation::default();
            invocation.story.topic = Patch::Set(topic.into());
            invocation.story.seed = Patch::Set(seed);
            invocation
                .pipeline
                .options
                .set(GENERATION_MODEL, topic.into())
                .unwrap();
            let config = base.for_invocation(invocation, &Operation::Story).unwrap();
            let inventory = FileLearningStore::new(dir.path())
                .read_learner_data()
                .unwrap()
                .unwrap()
                .into_inventory(now)
                .unwrap();
            let request = serde_json::from_value(
                json!({"version":1, "topic":topic,"targets":{"vocabulary":[],"grammar":[]}}),
            )
            .unwrap();
            let mut steps = Vec::new();
            let report = run_story(
                &config,
                StoryInputs { request, inventory },
                client,
                0,
                RunProgress::new(|event| steps.push(event)),
            )
            .await
            .unwrap();
            assert_eq!(report.selection.seed, seed);
            assert_eq!(report.request.topic.as_ref().unwrap().text(), topic);
            assert_eq!(report.generated.provenance().requested_model, topic);
            assert_eq!(steps.len(), 6);
        }
    };
    tokio::join!(run("garden", 7), run("snow", 42));
    assert_eq!(base.generation().unwrap().model, "gpt-6-luna");
    assert_eq!(base.story.seed, None);
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn supplied_invalid_inventory_and_targets_fail_before_generation() {
    use yomibu_core::domain::generation::{PracticeTargets, StoryError, StoryRequest};
    let dir = tempfile::tempdir().unwrap();
    let now = write_cache(dir.path());
    let server = MockServer::start().await;
    let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
    for invalid_inventory in [true, false] {
        let mut inventory = FileLearningStore::new(dir.path())
            .read_learner_data()
            .unwrap()
            .unwrap()
            .into_inventory(now)
            .unwrap();
        let mut request = StoryRequest {
            version: 1,
            topic: None,
            targets: PracticeTargets {
                vocabulary: vec![],
                grammar: vec![],
            },
        };
        if invalid_inventory {
            inventory.vocabulary[0].id.clear();
        } else {
            request.targets.vocabulary.push("missing".into());
        }
        let error = run_story(
            &config(dir.path(), Invocation::default()),
            StoryInputs { request, inventory },
            &client,
            7,
            RunProgress::new(|_| {}),
        )
        .await
        .unwrap_err();
        assert!(
            if invalid_inventory {
                matches!(
                    error,
                    ApplicationError::Inventory(_)
                        | ApplicationError::Story(StoryError::Inventory(_))
                )
            } else {
                matches!(
                    error,
                    ApplicationError::Story(StoryError::Invalid(
                        "missing or duplicate vocabulary target"
                    ))
                )
            },
            "{error}"
        );
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}
