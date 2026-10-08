// Dictionaries are absent or incomplete in these isolated tests; none is mapped.
use serde_json::json;
use std::{collections::BTreeMap, time::SystemTime};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};
use yomibu::app::{
    config::{Configuration, ConfigurationInput, Settings},
    local::{ApplicationError, Credentials, LocalApp, ProgressEvent, ServiceEndpoints, Step},
    modules::{ModuleId, ModuleState},
};

fn config(dir: &std::path::Path, flags: Settings) -> Configuration {
    Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags,
        },
        &yomibu::app::Operation::Story,
    )
    .unwrap()
}

fn assert_completed_steps(events: &[ProgressEvent]) {
    let mut running = None;
    for event in events {
        match event {
            ProgressEvent::Started { step } => {
                assert!(
                    running.replace(*step).is_none(),
                    "overlapping steps: {events:?}"
                );
            }
            ProgressEvent::Completed { step, elapsed_ms } => {
                assert_eq!(running.take(), Some(*step));
                assert!(elapsed_ms.is_finite() && *elapsed_ms >= 0.);
            }
            ProgressEvent::Skipped { .. } => {}
        }
    }
    assert!(running.is_none(), "unfinished step: {events:?}");
}

#[tokio::test]
async fn missing_setup_is_aggregated_before_network_or_writes_and_secrets_are_redacted() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("fresh");
    let app = LocalApp::new(config(&data, Settings::default()), Credentials::default());
    let error = unsafe { app.story(SystemTime::now().into(), 7, |_| {}) }
        .await
        .unwrap_err();
    let ApplicationError::Setup { issues } = error else {
        panic!("{error:?}")
    };
    assert_eq!(
        issues.iter().map(|issue| issue.module).collect::<Vec<_>>(),
        [ModuleId::Sync, ModuleId::Generation]
    );
    assert!(!data.exists());
    assert!(
        !format!(
            "{:?}",
            Credentials::new(Some("wk-secret".into()), Some("ai-secret".into()))
        )
        .contains("secret")
    );
}

#[tokio::test]
async fn disabling_sync_without_knowledge_reports_the_missing_inventory_instead_of_a_key() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("fresh");
    let app = LocalApp::new(
        config(
            &data,
            Settings {
                disable: vec![ModuleId::Sync],
                ..Default::default()
            },
        ),
        Credentials::new(Some("supplied-wk-key".into()), None),
    );
    let ApplicationError::Setup { issues } =
        unsafe { app.story(SystemTime::now().into(), 1, |_| {}) }
            .await
            .unwrap_err()
    else {
        panic!("expected setup issues")
    };
    assert_eq!(
        issues.iter().map(|issue| issue.module).collect::<Vec<_>>(),
        [ModuleId::Knowledge, ModuleId::Generation]
    );
    assert!(!data.exists());
}

async fn mount_source(server: &MockServer) {
    mount_source_with_user(
        server,
        include_str!("../../../tests/fixtures/wanikani/user.json"),
    )
    .await;
}

async fn mount_source_with_user(server: &MockServer, user: &str) {
    for (endpoint, body) in [
        ("user", user),
        (
            "assignments",
            include_str!("../../../tests/fixtures/wanikani/assignments.json"),
        ),
        (
            "review_statistics",
            include_str!("../../../tests/fixtures/wanikani/review_statistics.json"),
        ),
        (
            "subjects",
            include_str!("../../../tests/fixtures/wanikani/subjects.json"),
        ),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("/v2/{endpoint}")))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .expect(1)
            .mount(server)
            .await;
    }
}

async fn mount_generation(server: &MockServer, count: u64) {
    Mock::given(method("POST")).and(path("/v1/responses")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"synthetic", "model":"returned", "status":"completed", "output":[{"type":"message", "role":"assistant", "status":"completed", "content":[{"type":"output_text", "text":json!({"candidates":[{"sentences":["猫です。","寝ます。","朝です。"]}]}).to_string()}]}]}))).expect(count).mount(server).await;
}

#[tokio::test]
async fn two_keys_generate_once_without_optional_resources_and_the_second_run_uses_cache() {
    let server = MockServer::start().await;
    mount_source(&server).await;
    mount_generation(&server, 2).await;
    let dir = tempfile::tempdir().unwrap();
    let app = LocalApp::new(
        config(dir.path(), Settings::default()),
        Credentials::new(Some("synthetic-wk".into()), Some("synthetic-ai".into())),
    )
    .with_endpoints(ServiceEndpoints {
        wanikani: format!("{}/v2/", server.uri()),
        openai: format!("{}/v1/", server.uri()),
    });
    let now = SystemTime::now().into();
    let mut events = Vec::new();
    let report = unsafe { app.story(now, 7, |event| events.push(event)) }
        .await
        .unwrap();
    assert!(
        report
            .modules
            .iter()
            .find(|module| module.metadata.id == ModuleId::Sync)
            .unwrap()
            .required
    );
    assert_eq!(
        report.generated.passages()[0].text,
        "猫です。寝ます。朝です。"
    );
    assert!(report.request.topic.is_none());
    assert!(matches!(
        report
            .modules
            .iter()
            .find(|module| module.metadata.id == ModuleId::Assessment)
            .unwrap()
            .state,
        ModuleState::NotConfigured
    ));
    assert!(matches!(
        report
            .modules
            .iter()
            .find(|module| module.metadata.id == ModuleId::Embeddings)
            .unwrap()
            .state,
        ModuleState::Disabled
    ));
    assert!(!events.is_empty());
    assert_completed_steps(&events);
    let cached_report = unsafe { app.story(SystemTime::now().into(), 7, |_| {}) }
        .await
        .unwrap();
    assert!(
        !cached_report
            .modules
            .iter()
            .find(|module| module.metadata.id == ModuleId::Sync)
            .unwrap()
            .required
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.method.as_str() == "GET")
            .count(),
        4
    );
    for request in requests.iter().filter(|r| r.method.as_str() == "POST") {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        let input: serde_json::Value =
            serde_json::from_str(body["input"][1]["content"].as_str().unwrap()).unwrap();
        assert!(input.get("topic").is_none());
    }
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("synthetic-ai")
    );
}

fn write_cache(dir: &std::path::Path) -> chrono::DateTime<chrono::Utc> {
    std::fs::write(
        dir.join("wanikani.json"),
        include_bytes!("../../../tests/fixtures/mixed.json"),
    )
    .unwrap();
    yomibu::adapters::stores::file::cache::load(dir)
        .unwrap()
        .sync_completed_at
}
fn endpoints(server: &MockServer) -> ServiceEndpoints {
    ServiceEndpoints {
        wanikani: format!("{}/v2/", server.uri()),
        openai: format!("{}/v1/", server.uri()),
    }
}

#[tokio::test]
async fn cache_is_fresh_until_the_one_hour_boundary_then_refreshes() {
    let dir = tempfile::tempdir().unwrap();
    let completed = write_cache(dir.path());
    let server = MockServer::start().await;
    mount_source(&server).await;
    mount_generation(&server, 2).await;
    let app = LocalApp::new(
        config(dir.path(), Settings::default()),
        Credentials::new(Some("wk".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    unsafe { app.story(completed + chrono::Duration::seconds(3599), 1, |_| {}) }
        .await
        .unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    unsafe { app.story(completed + chrono::Duration::hours(1), 1, |_| {}) }
        .await
        .unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 6);
}

#[tokio::test]
async fn cache_from_a_future_completion_time_is_refreshed() {
    let dir = tempfile::tempdir().unwrap();
    let completed = write_cache(dir.path());
    let server = MockServer::start().await;
    mount_source(&server).await;
    mount_generation(&server, 1).await;
    let app = LocalApp::new(
        config(dir.path(), Settings::default()),
        Credentials::new(Some("wk".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    unsafe { app.story(completed - chrono::Duration::seconds(1), 1, |_| {}) }
        .await
        .unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 5);
}

#[tokio::test]
async fn a_recent_cache_without_usable_vocabulary_is_refreshed_even_under_the_lock() {
    use yomibu::adapters::stores::file::cache::{SyncGuard, load};
    for replaced_by_another_writer in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let completed = write_cache(dir.path());
        let now = completed + chrono::Duration::hours(2);
        let mut empty = load(dir.path()).unwrap();
        for assignment in &mut empty.assignments {
            assignment.started_at = None;
            assignment.passed_at = None;
            assignment.burned_at = None;
            assignment.srs_stage = 0;
        }
        empty.sync_completed_at = now;
        if !replaced_by_another_writer {
            SyncGuard::acquire(dir.path())
                .unwrap()
                .replace(&empty)
                .unwrap();
        }
        let server = MockServer::start().await;
        mount_source(&server).await;
        mount_generation(&server, 1).await;
        let app = LocalApp::new(
            config(dir.path(), Settings::default()),
            Credentials::new(Some("wk".into()), Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        let report = unsafe {
            app.story(now, 1, |event| {
                if replaced_by_another_writer
                    && matches!(event, ProgressEvent::Started { step: Step::Sync })
                {
                    SyncGuard::acquire(dir.path())
                        .unwrap()
                        .replace(&empty)
                        .unwrap();
                }
            })
        }
        .await
        .unwrap();
        assert_eq!(
            report.generated.passages()[0].text,
            "猫です。寝ます。朝です。"
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 5);
    }
}

#[tokio::test]
async fn usable_old_cache_needs_no_source_key_and_disabled_sync_never_initializes_it() {
    let dir = tempfile::tempdir().unwrap();
    let completed = write_cache(dir.path());
    let server = MockServer::start().await;
    mount_generation(&server, 2).await;
    for (flags, key) in [
        (Settings::default(), None),
        (
            Settings {
                disable: vec![ModuleId::Sync],
                ..Default::default()
            },
            Some("invalid\ncredential".into()),
        ),
    ] {
        let app = LocalApp::new(
            config(dir.path(), flags),
            Credentials::new(key, Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        let report = unsafe { app.story(completed + chrono::Duration::hours(2), 1, |_| {}) }
            .await
            .unwrap();
        assert_eq!(report.warnings.len(), 1);
    }
    assert_eq!(
        std::fs::read(dir.path().join("wanikani.json")).unwrap(),
        include_bytes!("../../../tests/fixtures/mixed.json")
    );
}

#[tokio::test]
async fn temporary_refresh_failure_uses_cache_but_authentication_is_fatal_and_preserves_bytes() {
    for status in [500, 401] {
        let dir = tempfile::tempdir().unwrap();
        let completed = write_cache(dir.path());
        let server = MockServer::start().await;
        Mock::given(path("/v2/user"))
            .respond_with(ResponseTemplate::new(status))
            .expect(if status == 500 { 3 } else { 1 })
            .mount(&server)
            .await;
        if status == 500 {
            mount_generation(&server, 1).await;
        }
        let app = LocalApp::new(
            config(dir.path(), Settings::default()),
            Credentials::new(Some("wk".into()), Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        let result = unsafe { app.story(completed + chrono::Duration::hours(2), 1, |_| {}) }.await;
        if status == 500 {
            assert_eq!(result.unwrap().warnings.len(), 1);
        } else {
            assert!(matches!(
                result,
                Err(ApplicationError::Source(
                    yomibu::adapters::sources::wanikani::Error::Authentication
                ))
            ));
        }
        assert_eq!(
            std::fs::read(dir.path().join("wanikani.json")).unwrap(),
            include_bytes!("../../../tests/fixtures/mixed.json")
        );
    }
}

#[tokio::test]
async fn writer_contention_can_use_valid_cache_but_expired_access_cannot() {
    let dir = tempfile::tempdir().unwrap();
    let completed = write_cache(dir.path());
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let app = LocalApp::new(
        config(dir.path(), Settings::default()),
        Credentials::new(Some("wk".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    let guard = yomibu::adapters::stores::file::cache::SyncGuard::acquire(dir.path()).unwrap();
    let mut events = Vec::new();
    let report = unsafe {
        app.story(completed + chrono::Duration::hours(2), 1, |event| {
            events.push(event)
        })
    }
    .await
    .unwrap();
    assert!(report.warnings[0].message.contains("writer"));
    assert!(
        report
            .timings
            .iter()
            .any(|timing| timing.step == Step::Sync)
    );
    assert_completed_steps(&events);
    drop(guard);
    let mut cache: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../tests/fixtures/mixed.json")).unwrap();
    cache["snapshot"]["learner"]["subscription"]["period_ends_at"] =
        json!(completed + chrono::Duration::minutes(10));
    cache["snapshot"]["learner"]["subscription"]["active"] = json!(true);
    std::fs::write(
        dir.path().join("wanikani.json"),
        serde_json::to_vec(&cache).unwrap(),
    )
    .unwrap();
    let app = LocalApp::new(
        config(dir.path(), Settings::default()),
        Credentials::new(None, Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    assert!(
        unsafe { app.story(completed + chrono::Duration::minutes(10), 1, |_| {}) }
            .await
            .is_err()
    );
}

#[tokio::test]
async fn explicit_missing_dictionary_warns_but_absent_default_skips_assessment() {
    let server = MockServer::start().await;
    mount_generation(&server, 5).await;
    for source in ["flag", "environment", "file", "default", "broken_default"] {
        let dir = tempfile::tempdir().unwrap();
        let inventory = dir.path().join("inventory.json");
        std::fs::write(
            &inventory,
            include_bytes!("../../../tests/fixtures/story/inventory.json"),
        )
        .unwrap();
        let missing = dir.path().join("missing");
        let mut input = ConfigurationInput {
            data_dir: Some(dir.path().into()),
            flags: Settings {
                inventory: Some(inventory),
                ..Default::default()
            },
            config: None,
            home: None,
            environment: BTreeMap::new(),
        };
        match source {
            "flag" => input.flags.dictionary_dir = Some(missing),
            "environment" => {
                input.environment.insert(
                    "YOMIBU_DICTIONARY_DIR".into(),
                    missing.to_str().unwrap().into(),
                );
            }
            "file" => {
                let file = dir.path().join("config.toml");
                std::fs::write(&file, "dictionary_dir = 'missing'\n").unwrap();
                input.config = Some(file);
            }
            "broken_default" => {
                std::os::unix::fs::symlink(&missing, dir.path().join("dictionaries")).unwrap();
            }
            _ => {}
        }
        let app = LocalApp::new(
            Configuration::load(input, &yomibu::app::Operation::Story).unwrap(),
            Credentials::new(None, Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        let report = unsafe { app.story(SystemTime::now().into(), 1, |_| {}) }
            .await
            .unwrap();
        assert_eq!(
            report.generated.passages()[0].text,
            "猫です。寝ます。朝です。"
        );
        assert_eq!(
            report.warnings.len(),
            usize::from(source != "default"),
            "{source}"
        );
        let state = &report
            .modules
            .iter()
            .find(|module| module.metadata.id == ModuleId::Assessment)
            .unwrap()
            .state;
        if source == "default" {
            assert!(matches!(state, ModuleState::NotConfigured));
        } else {
            assert!(matches!(state, ModuleState::Unavailable { .. }));
            assert!(report.warnings[0].message.contains("dictionary import"));
        }
        assert!(matches!(
            report.assessments[0].sentences[0].assessment.assessment,
            yomibu::candidate::CandidateAssessment::NotRun
        ));
    }
}

#[tokio::test]
async fn optional_resources_enhance_when_available_and_failures_preserve_generation() {
    let dir = tempfile::tempdir().unwrap();
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 2).await;
    for assessment in [true, false] {
        let flags = Settings {
            inventory: Some(inventory.clone()),
            topic: Some("cat".into()),
            embedding_provider: Some(yomibu::app::config::EmbeddingProvider::LexicalBaseline),
            enable: vec![ModuleId::Embeddings],
            disable: if assessment {
                Vec::new()
            } else {
                vec![ModuleId::Assessment]
            },
            dictionary_dir: Some(dir.path().join("managed")),
            ..Default::default()
        };
        let app = LocalApp::new(
            config(dir.path(), flags),
            Credentials::new(None, Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        let report = unsafe { app.story(SystemTime::now().into(), 1, |_| {}) }
            .await
            .unwrap();
        assert!(matches!(
            report
                .modules
                .iter()
                .find(|module| module.metadata.id == ModuleId::Embeddings)
                .unwrap()
                .state,
            ModuleState::Available
        ));
        assert!(report.selection.embedding_model.is_some());
        assert!(matches!(
            report
                .modules
                .iter()
                .find(|module| module.metadata.id == ModuleId::Sync)
                .unwrap()
                .state,
            ModuleState::Skipped { .. }
        ));
        assert_eq!(report.warnings.len(), usize::from(assessment));
        assert!(matches!(
            report.assessments[0].sentences[0].assessment.assessment,
            yomibu::candidate::CandidateAssessment::NotRun
        ));
    }
    assert!(dir.path().join("embeddings.json").exists());
}

#[tokio::test]
async fn no_topic_skips_embeddings_and_the_explicit_retrieval_command_requires_a_provider() {
    let dir = tempfile::tempdir().unwrap();
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let flags = Settings {
        inventory: Some(inventory),
        enable: vec![ModuleId::Embeddings],
        ..Default::default()
    };
    let app = LocalApp::new(
        config(dir.path(), flags),
        Credentials::new(None, Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    let report = unsafe { app.story(SystemTime::now().into(), 1, |_| {}) }
        .await
        .unwrap();
    assert!(matches!(
        report
            .modules
            .iter()
            .find(|module| module.metadata.id == ModuleId::Embeddings)
            .unwrap()
            .state,
        ModuleState::Skipped { .. }
    ));
    assert!(!dir.path().join("embeddings.json").exists());
    assert!(
        app.prepare_retrieval(std::time::SystemTime::now().into())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn optional_embedding_failure_falls_back_to_builtin_selection_without_hosted_authorization() {
    let dir = tempfile::tempdir().unwrap();
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let flags = Settings {
        inventory: Some(inventory),
        topic: Some("cat".into()),
        enable: vec![ModuleId::Embeddings],
        embedding_provider: Some(yomibu::app::config::EmbeddingProvider::Openai),
        embedding_model: Some("embedding-model".into()),
        embedding_revision: Some("pinned".into()),
        embedding_dimensions: Some(2),
        ..Default::default()
    };
    let app = LocalApp::new(
        config(dir.path(), flags),
        Credentials::new(None, Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    let report = unsafe { app.story(SystemTime::now().into(), 1, |_| {}) }
        .await
        .unwrap();
    assert_eq!(report.selection.selector_revision, "builtin-v2");
    assert_eq!(report.warnings.len(), 1);
    assert!(
        report.warnings[0]
            .message
            .contains("--allow-embedding-call")
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn partial_embedding_settings_do_not_silently_reuse_another_cached_model() {
    let dir = tempfile::tempdir().unwrap();
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let flags = Settings {
        inventory: Some(inventory.clone()),
        topic: Some("cat".into()),
        embedding_provider: Some(yomibu::app::config::EmbeddingProvider::LexicalBaseline),
        ..Default::default()
    };
    LocalApp::new(config(dir.path(), flags), Credentials::default())
        .prepare_retrieval(std::time::SystemTime::now().into())
        .await
        .unwrap();
    let before = std::fs::read(dir.path().join("embeddings.json")).unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let app = LocalApp::new(
        config(
            dir.path(),
            Settings {
                inventory: Some(inventory),
                topic: Some("cat".into()),
                embedding_model: Some("another-model".into()),
                enable: vec![ModuleId::Embeddings],
                ..Default::default()
            },
        ),
        Credentials::new(None, Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    let report = unsafe { app.story(SystemTime::now().into(), 1, |_| {}) }
        .await
        .unwrap();
    assert_eq!(report.selection.selector_revision, "builtin-v2");
    assert!(report.warnings[0].message.contains("--embedding-provider"));
    assert!(matches!(
        report
            .modules
            .iter()
            .find(|module| module.metadata.id == ModuleId::Embeddings)
            .unwrap()
            .state,
        ModuleState::NotConfigured
    ));
    let preview = app.preview(SystemTime::now().into(), 1).unwrap();
    assert_eq!(
        preview.selection.selector_revision,
        report.selection.selector_revision
    );
    assert_eq!(
        preview.selection.vocabulary_ids,
        report.selection.vocabulary_ids
    );
    assert_eq!(
        preview.provider_request.body_utf8().as_bytes(),
        server.received_requests().await.unwrap()[0].body
    );
    assert_eq!(
        std::fs::read(dir.path().join("embeddings.json")).unwrap(),
        before
    );
}

#[tokio::test]
async fn complete_hosted_cache_is_reused_without_call_authorization_or_credentials() {
    use yomibu::{
        adapters::embedding_cache_file::EmbeddingCacheFile,
        app::config::EmbeddingProvider,
        inventory::LearnerInventory,
        retrieval::{EmbeddingCache, EmbeddingModelIdentity, prepare_embedding_inputs},
        story::StoryRequest,
    };
    let dir = tempfile::tempdir().unwrap();
    let inventory_path = dir.path().join("inventory.json");
    let inventory_bytes = include_bytes!("../../../tests/fixtures/story/inventory.json");
    std::fs::write(&inventory_path, inventory_bytes).unwrap();
    let inventory =
        LearnerInventory::from_manual(serde_json::from_slice(inventory_bytes).unwrap()).unwrap();
    let request_path = dir.path().join("request.json");
    let request_bytes = include_bytes!("../../../tests/fixtures/story/request.json");
    std::fs::write(&request_path, request_bytes).unwrap();
    let request: StoryRequest = serde_json::from_slice(request_bytes).unwrap();
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    let cache = EmbeddingCache::from_vectors(
        EmbeddingModelIdentity {
            provider: "openai".into(),
            model: "synthetic-model".into(),
            revision: "pinned".into(),
            dimensions: 2,
            encoding_revision: "plain-v1".into(),
        },
        &inputs,
        vec![vec![1., 0.]; inputs.len()],
    )
    .unwrap();
    let cache_path = dir.path().join("embeddings.json");
    EmbeddingCacheFile::new(&cache_path).save(&cache).unwrap();
    let before = std::fs::read(&cache_path).unwrap();
    let app = LocalApp::new(
        config(
            dir.path(),
            Settings {
                inventory: Some(inventory_path),
                request: Some(request_path.clone()),
                embedding_provider: Some(EmbeddingProvider::Openai),
                embedding_model: Some(cache.model.model.clone()),
                embedding_revision: Some(cache.model.revision.clone()),
                embedding_dimensions: Some(2),
                enable: vec![ModuleId::Embeddings],
                ..Default::default()
            },
        ),
        Credentials::default(),
    );
    let now = SystemTime::now().into();
    let reused = app.prepare_retrieval(now).await.unwrap();
    assert_eq!(
        serde_json::to_value(reused).unwrap(),
        serde_json::to_value(&cache).unwrap()
    );
    let preview = app.preview(now, 7).unwrap();
    assert_eq!(preview.selection.embedding_model, Some(cache.model));
    assert!(preview.warnings.is_empty());

    let mut changed: serde_json::Value = serde_json::from_slice(request_bytes).unwrap();
    changed["topic"] = json!("An uncached topic");
    std::fs::write(request_path, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(matches!(
        app.prepare_retrieval(now).await,
        Err(ApplicationError::ResourceConfiguration(
            "Hosted embeddings require --allow-embedding-call."
        ))
    ));
    let preview = app.preview(now, 7).unwrap();
    assert_eq!(preview.selection.selector_revision, "builtin-v2");
    assert_eq!(preview.warnings.len(), 1);
    assert_eq!(std::fs::read(cache_path).unwrap(), before);
}

#[tokio::test]
async fn custom_source_cache_is_refreshed_in_place_and_an_invalid_manual_input_prevents_writes() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("knowledge.json");
    let completed = write_cache(dir.path());
    std::fs::rename(dir.path().join("wanikani.json"), &cache).unwrap();
    let server = MockServer::start().await;
    mount_source(&server).await;
    mount_generation(&server, 1).await;
    let app = LocalApp::new(
        config(
            &dir.path().join("data"),
            Settings {
                wanikani_cache: Some(cache.clone()),
                ..Default::default()
            },
        ),
        Credentials::new(Some("wk".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    unsafe { app.story(completed + chrono::Duration::hours(2), 1, |_| {}) }
        .await
        .unwrap();
    assert_ne!(
        std::fs::read(&cache).unwrap(),
        include_bytes!("../../../tests/fixtures/mixed.json")
    );
    assert!(!dir.path().join("data/wanikani.json").exists());
    let inventory = dir.path().join("inventory.json");
    std::fs::write(&inventory, r#"{"version":1,"vocabulary":[{"id":"","written_form":"猫","readings":[],"meanings":[],"direct_object":null}],"grammar_declarations":[],"grammar_bindings":[]}"#).unwrap();
    let app = LocalApp::new(
        config(
            &dir.path().join("absent"),
            Settings {
                inventory: Some(inventory),
                wanikani_cache: Some(dir.path().join("absent/wanikani.json")),
                ..Default::default()
            },
        ),
        Credentials::new(Some("wk".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    assert!(matches!(
        unsafe { app.story(SystemTime::now().into(), 1, |_| {}) }.await,
        Err(ApplicationError::Inventory(_))
    ));
    assert!(!dir.path().join("absent").exists());
}

#[tokio::test]
async fn invalid_request_files_are_rejected_before_automatic_sync_or_writes() {
    let dir = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    let request = dir.path().join("request.json");
    for body in [
        "{",
        r#"{"version":0,"targets":{"vocabulary":[],"grammar":[]}}"#,
    ] {
        std::fs::write(&request, body).unwrap();
        let data = dir.path().join("absent");
        let app = LocalApp::new(
            config(
                &data,
                Settings {
                    request: Some(request.clone()),
                    ..Default::default()
                },
            ),
            Credentials::new(Some("wk".into()), Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        assert!(matches!(
            unsafe { app.story(SystemTime::now().into(), 1, |_| {}) }.await,
            Err(ApplicationError::InvalidJson { .. } | ApplicationError::Story(_))
        ));
        assert!(server.received_requests().await.unwrap().is_empty());
        assert!(!data.exists());
    }
}

#[tokio::test]
async fn refreshed_expired_access_is_rejected_before_replacing_a_usable_cache() {
    let dir = tempfile::tempdir().unwrap();
    let completed = write_cache(dir.path());
    let now = completed + chrono::Duration::hours(2);
    let mut user: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/wanikani/user.json")).unwrap();
    user["data"]["subscription"]["active"] = json!(true);
    user["data"]["subscription"]["period_ends_at"] = json!(now);
    let server = MockServer::start().await;
    mount_source_with_user(&server, &user.to_string()).await;
    let app = LocalApp::new(
        config(dir.path(), Settings::default()),
        Credentials::new(Some("wk".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    assert!(matches!(
        unsafe { app.story(now, 1, |_| {}) }.await,
        Err(ApplicationError::AccessExpired)
    ));
    assert_eq!(
        std::fs::read(dir.path().join("wanikani.json")).unwrap(),
        include_bytes!("../../../tests/fixtures/mixed.json")
    );
}

#[tokio::test]
async fn concurrent_story_runs_refresh_once_and_keep_a_complete_usable_cache() {
    let dir = tempfile::tempdir().unwrap();
    let completed = write_cache(dir.path());
    let server = MockServer::start().await;
    mount_source(&server).await;
    mount_generation(&server, 2).await;
    let app = LocalApp::new(
        config(dir.path(), Settings::default()),
        Credentials::new(Some("wk".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    let now = completed + chrono::Duration::hours(2);
    let (first, second) = tokio::join!(unsafe { app.story(now, 1, |_| {}) }, unsafe {
        app.story(now, 2, |_| {})
    });
    let reports = [first.unwrap(), second.unwrap()];
    assert_eq!(
        reports
            .iter()
            .flat_map(|report| &report.warnings)
            .filter(|warning| warning.message.contains("writer"))
            .count(),
        1
    );
    assert_eq!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| request.method.as_str() == "GET")
            .count(),
        4
    );
    assert!(
        yomibu::adapters::stores::file::cache::load(dir.path())
            .unwrap()
            .sync_completed_at
            > completed
    );
}

#[tokio::test]
async fn freshness_is_rechecked_under_the_lock_after_another_writer_refreshes() {
    use yomibu::adapters::stores::file::cache::{SyncGuard, load};
    let dir = tempfile::tempdir().unwrap();
    let completed = write_cache(dir.path());
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let app = LocalApp::new(
        config(dir.path(), Settings::default()),
        Credentials::new(Some("wk".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    let now = completed + chrono::Duration::hours(2);
    let report = unsafe {
        app.story(now, 1, |event| {
            if matches!(
                event,
                yomibu::app::local::ProgressEvent::Started {
                    step: yomibu::app::local::Step::Sync
                }
            ) {
                let writer = SyncGuard::acquire(dir.path()).unwrap();
                let mut data = load(dir.path()).unwrap();
                data.sync_completed_at = now;
                writer.replace(&data).unwrap();
            }
        })
    }
    .await
    .unwrap();
    assert!(matches!(
        report
            .modules
            .iter()
            .find(|module| module.metadata.id == ModuleId::Sync)
            .unwrap()
            .state,
        ModuleState::Skipped { .. }
    ));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    assert!(report.warnings.is_empty());
}

#[tokio::test]
async fn an_inactive_subscription_retains_its_recorded_free_content_access() {
    let dir = tempfile::tempdir().unwrap();
    let completed = write_cache(dir.path());
    let mut data: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../tests/fixtures/mixed.json")).unwrap();
    data["snapshot"]["learner"]["subscription"]["period_ends_at"] =
        json!(completed - chrono::Duration::days(1));
    std::fs::write(
        dir.path().join("wanikani.json"),
        serde_json::to_vec(&data).unwrap(),
    )
    .unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let app = LocalApp::new(
        config(dir.path(), Settings::default()),
        Credentials::new(None, Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    unsafe { app.story(completed + chrono::Duration::minutes(10), 1, |_| {}) }
        .await
        .unwrap();
}
