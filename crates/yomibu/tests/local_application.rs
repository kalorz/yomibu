#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::SystemTime,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};
use yomibu::{
    application::{
        ApplicationError, CredentialError, Credentials, LocalApp, Operation, ServiceEndpoints,
        StoryInputs,
        progress::{ProgressEvent, Step},
        run_story,
    },
    configuration::{
        Configuration, ConfigurationInput, EmbeddingProvider, Invocation, Patch, ProcessOverrides,
        components::{
            EMBEDDING_DIMENSIONS, EMBEDDING_ENDPOINT, EMBEDDING_MODEL, EMBEDDING_REVISION,
            GENERATION_KEY, GENERATION_MODEL, SOURCE_KEY,
        },
        modules::{ModuleId, ModuleState},
    },
};
use yomibu_components::{
    embedding_vocabulary_selection::prepare_embedding_inputs,
    file_learning_store::FileLearningStore,
    in_memory_learning_store::InMemoryLearningStore,
    openai_story_generation::{Client, ProviderError},
    wanikani_source,
};
use yomibu_core::{
    capabilities::{LearningStore, SourceSyncWriter},
    domain::{
        candidate::CandidateAssessment,
        embedding::{EmbeddingCache, EmbeddingModelIdentity},
        evaluation::EvaluationBasis,
        inventory::LearnerInventory,
        source::WaniKaniSyncData,
        story::{StoryError, StoryRequest},
    },
};

macro_rules! source_stores {
    ($dir:ident, $store:ident, $body:block) => {{
        let $dir = tempfile::tempdir().unwrap();
        let $store = FileLearningStore::new($dir.path());
        $body
        let $dir = tempfile::tempdir().unwrap();
        let $store = InMemoryLearningStore::new();
        $body
        assert!(std::fs::read_dir($dir.path()).unwrap().next().is_none());
    }};
}

fn config(dir: &std::path::Path, flags: ProcessOverrides) -> Configuration {
    Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags,
        },
        &yomibu::application::Operation::Story,
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
    let store = FileLearningStore::new(&data);
    let app = LocalApp::new(
        config(&data, ProcessOverrides::default()),
        Credentials::default(),
    );
    let error = unsafe { app.story(&store, SystemTime::now().into(), 7, |_| {}) }
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
            supplied_credentials(Some("wk-secret".into()), Some("ai-secret".into()))
        )
        .contains("secret")
    );
}

#[tokio::test]
async fn disabling_sync_without_knowledge_reports_the_missing_inventory_instead_of_a_key() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("fresh");
    let store = FileLearningStore::new(&data);
    let app = LocalApp::new(
        config(
            &data,
            ProcessOverrides {
                disable: vec![ModuleId::Sync],
                ..Default::default()
            },
        ),
        supplied_credentials(Some("supplied-wk-key".into()), None),
    );
    let ApplicationError::Setup { issues } =
        unsafe { app.story(&store, SystemTime::now().into(), 1, |_| {}) }
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

async fn mount_source(server: &MockServer, count: u64) {
    mount_source_with_user(
        server,
        include_str!("../../../tests/fixtures/wanikani/user.json"),
        count,
    )
    .await;
}

async fn mount_source_with_user(server: &MockServer, user: &str, count: u64) {
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
            .expect(count)
            .mount(server)
            .await;
    }
}

async fn mount_generation(server: &MockServer, count: u64) {
    Mock::given(method("POST")).and(path("/v1/responses")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"synthetic", "model":"returned", "status":"completed", "output":[{"type":"message", "role":"assistant", "status":"completed", "content":[{"type":"output_text", "text":json!({"candidates":[{"sentences":["猫です。","寝ます。","朝です。"]}]}).to_string()}]}]}))).expect(count).mount(server).await;
}

fn embedding_fixture() -> (StoryInputs, EmbeddingCache) {
    let manual = LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap();
    let request =
        serde_json::from_str(include_str!("../../../tests/fixtures/story/request.json")).unwrap();
    let inputs = prepare_embedding_inputs(&manual, &request).unwrap();
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
    (
        StoryInputs {
            request,
            manual: Some(manual),
        },
        cache,
    )
}

fn embedding_flags() -> ProcessOverrides {
    let mut flags = ProcessOverrides {
        enable: vec![ModuleId::Embeddings],
        ..Default::default()
    };
    flags.invocation.pipeline.embedding.provider = Patch::Set(EmbeddingProvider::Openai);
    let options = &mut flags.invocation.pipeline.options;
    options
        .set(EMBEDDING_MODEL, "synthetic-model".into())
        .unwrap();
    options.set(EMBEDDING_REVISION, "pinned".into()).unwrap();
    options.set(EMBEDDING_DIMENSIONS, 2).unwrap();
    flags
}

#[tokio::test]
async fn supplied_client_and_analyzer_are_reused_with_per_call_models_and_disabled_assessment() {
    let analyzer = test_dictionary::load_analyzer();
    let dir = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 3).await;
    let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
    let (_, mut cache) = embedding_fixture();
    for entry in &mut cache.entries[..3] {
        entry.vector = vec![0., 1.];
    }
    let mut flags = embedding_flags();
    flags.application.dictionary_dir = Some(dir.path().join("missing-dictionary"));
    flags.application.embedding_cache = Some(dir.path().into());
    flags.application.allow_embedding_call = Some(true);
    let base = config(dir.path(), flags);
    for (index, (model, disabled)) in [
        ("first-model", false),
        ("second-model", false),
        ("second-model", true),
    ]
    .into_iter()
    .enumerate()
    {
        let mut invocation = Invocation::default();
        invocation
            .pipeline
            .options
            .set(GENERATION_MODEL, model.into())
            .unwrap();
        invocation.pipeline.assessment.enabled = Some(!disabled);
        invocation.story.select = Some(3);
        let config = base.for_invocation(invocation, &Operation::Story).unwrap();
        let (inputs, _) = embedding_fixture();
        let plan = yomibu::application::story::plan_generation(
            inputs.manual.as_ref().unwrap(),
            &inputs.request,
            &cache,
            &cache.model,
            3,
            yomibu_core::domain::story::StoryGenerationOptions {
                model: model.into(),
                ..Default::default()
            },
        )
        .unwrap();
        let expected_body = plan.prepared_request().body_utf8().as_bytes().to_vec();
        let mut events = Vec::new();
        let report = run_story::<InMemoryLearningStore>(
            &config,
            inputs,
            None,
            None,
            &client,
            Some(&analyzer),
            Some(&cache),
            SystemTime::now().into(),
            7,
            |event| events.push(event),
        )
        .await
        .unwrap();
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), index + 1);
        assert_eq!(requests[index].method.as_str(), "POST");
        assert_eq!(requests[index].url.path(), "/v1/responses");
        assert_eq!(requests[index].body, expected_body);
        let body: serde_json::Value = serde_json::from_slice(&requests[index].body).unwrap();
        assert_eq!(body["model"], model);
        assert_eq!(report.generated.provenance().requested_model, model);
        assert_eq!(report.generated.provenance().request_count, 1);
        assert_eq!(report.selection.vocabulary_ids, ["sleep", "cat", "walk"]);
        assert_eq!(
            report.selection.selector_revision,
            "inventory-similarity-v1"
        );
        assert_eq!(
            report.selection.embedding_model.as_ref(),
            Some(&cache.model)
        );
        assert_eq!(
            report.generated.passages()[0].text,
            "猫です。寝ます。朝です。"
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(report.assessments[0].sentences.len(), 3);
        for sentence in &report.assessments[0].sentences {
            match &sentence.assessment.assessment {
                CandidateAssessment::NotRun if disabled => {}
                CandidateAssessment::Completed {
                    analysis,
                    evaluation,
                } if !disabled => {
                    assert_eq!(evaluation.basis, EvaluationBasis::FullLearnerInventory);
                    assert!(!analysis.units.is_empty());
                }
                other => panic!("{other:?}"),
            }
        }
        let state = &report
            .modules
            .iter()
            .find(|m| m.metadata.id == ModuleId::Assessment)
            .unwrap()
            .state;
        assert!(matches!(
            (disabled, state),
            (true, ModuleState::Disabled) | (false, ModuleState::Available)
        ));
        assert_completed_steps(&events);
        if disabled {
            assert!(
                matches!(events.last(), Some(ProgressEvent::Skipped { step: Step::Assessment, reason }) if reason == "Assessment disabled")
            );
        } else {
            assert!(matches!(
                events.last(),
                Some(ProgressEvent::Completed {
                    step: Step::Assessment,
                    ..
                })
            ));
        }
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[tokio::test]
async fn supplied_embedding_evidence_is_checked_against_inputs_and_configuration_only_when_used() {
    use yomibu::configuration::SelectionStep;
    let dir = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    let cases = [
        "version",
        "vector",
        "incomplete",
        "document",
        "query",
        "model",
        "revision",
        "dimensions",
        "encoding",
        "provider",
        "partial",
        "none",
        "inferred",
        "local",
        "disabled",
        "no-topic",
    ];
    mount_generation(&server, cases.len() as u64).await;
    Mock::given(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
    for case in cases {
        let (mut inputs, mut cache) = embedding_fixture();
        let mut flags = embedding_flags();
        flags.application.embedding_cache = Some(dir.path().join("absent/embeddings.json"));
        flags.application.allow_embedding_call = Some(true);
        flags.invocation.pipeline.selection.steps = Some(vec![SelectionStep::SeededOrder]);
        flags.invocation.pipeline.selection.embedding_steps = Some(vec![
            SelectionStep::EmbeddingRank,
            SelectionStep::SeededOrder,
        ]);
        flags
            .invocation
            .pipeline
            .options
            .set(EMBEDDING_ENDPOINT, format!("{}/v1/", server.uri()))
            .unwrap();
        let mut config = config(dir.path(), flags);
        match case {
            "version" | "disabled" | "no-topic" => cache.version = 0,
            "vector" => cache.entries[0].vector[0] = f32::NAN,
            "incomplete" => {
                cache.entries.pop();
            }
            "document" => {
                inputs.manual.as_mut().unwrap().vocabulary[0].meanings = vec!["changed".into()]
            }
            "query" => {
                inputs.request.topic =
                    Some(yomibu_core::domain::story::StoryTopic::new("changed".into()).unwrap())
            }
            "model" => cache.model.model = "another-model".into(),
            "revision" => cache.model.revision = "another-revision".into(),
            "dimensions" => {
                cache.model.dimensions = 3;
                for entry in &mut cache.entries {
                    entry.vector.push(0.);
                }
            }
            "encoding" => cache.model.encoding_revision = "another-encoding".into(),
            "provider" => cache.model.provider = "local".into(),
            "partial" => config.pipeline.embedding_provider = None,
            "inferred" => {
                config.pipeline.embedding_provider = None;
                config.pipeline.options = Default::default();
            }
            "local" => {
                config.pipeline.embedding_provider = Some(EmbeddingProvider::Local);
                cache.model.provider = "local".into();
            }
            _ => {}
        }
        if case == "disabled" {
            config.pipeline.embeddings = false;
        }
        if case == "no-topic" {
            inputs.request.topic = None;
        }
        let mut events = Vec::new();
        let report = run_story::<InMemoryLearningStore>(
            &config,
            inputs,
            None,
            None,
            &client,
            None,
            (case != "none").then_some(&cache),
            SystemTime::now().into(),
            7,
            |event| events.push(event),
        )
        .await
        .unwrap();
        let available = matches!(case, "inferred" | "local");
        let skipped = matches!(case, "disabled" | "no-topic");
        assert_eq!(
            report.selection.selector_revision,
            if available {
                "inventory-similarity-seeded-v1"
            } else {
                "seeded-only-v1"
            },
            "{case}"
        );
        assert_eq!(
            report.selection.embedding_model.is_some(),
            available,
            "{case}"
        );
        assert_eq!(
            report.warnings.len(),
            usize::from(!available && !skipped),
            "{case}"
        );
        let state = &report
            .modules
            .iter()
            .find(|m| m.metadata.id == ModuleId::Embeddings)
            .unwrap()
            .state;
        let status = match case {
            "inferred" | "local" => "available",
            "disabled" => "disabled",
            "no-topic" => "skipped",
            "partial" | "none" => "not_configured",
            _ => "unavailable",
        };
        assert_eq!(
            serde_json::to_value(state).unwrap()["status"],
            status,
            "{case}"
        );
        if let Some(warning) = report.warnings.first() {
            let diagnostic = match case {
                "version" | "vector" => "Invalid embedding data",
                "partial" => "Select --embedding-provider",
                "none" => "No embedding cache supplied",
                _ => "Embedding cache is missing or stale",
            };
            assert!(
                warning.message.starts_with(diagnostic),
                "{case}: {:?}",
                report.warnings
            );
            assert_eq!(warning.module, ModuleId::Embeddings);
            assert!(warning.message.ends_with("Using seeded-only-v1 selection."));
            if let ModuleState::Unavailable { error } = state {
                assert_eq!(
                    warning.message,
                    format!("{error} Using seeded-only-v1 selection.")
                );
            }
        }
        assert_completed_steps(&events);
        assert_eq!(
            events.iter().any(|event| matches!(
                event,
                ProgressEvent::Started {
                    step: Step::Embeddings
                }
            )),
            !skipped
        );
    }
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|r| r.url.path() == "/v1/responses")
    );
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[tokio::test]
async fn supplied_source_none_reports_missing_knowledge_without_fetching() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("fresh");
    let store = FileLearningStore::new(&data);
    let server = MockServer::start().await;
    let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
    for sync in [true, false] {
        let mut flags = ProcessOverrides::default();
        if !sync {
            flags.disable.push(ModuleId::Sync);
        }
        let config = config(&data, flags);
        let error = run_story(
            &config,
            source_story_inputs(),
            Some(&store),
            None,
            &client,
            None,
            None,
            SystemTime::now().into(),
            7,
            |_| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(error, ApplicationError::Setup { issues }
            if issues.len() == 1 && issues[0].module == ModuleId::Knowledge));
        assert!(server.received_requests().await.unwrap().is_empty());
        assert!(!data.exists());
    }
}

#[tokio::test]
async fn supplied_source_refresh_reuses_client_and_invalidates_prepared_embedding_evidence() {
    source_stores!(dir, store, {
        let server = MockServer::start().await;
        mount_source(&server, 2).await;
        mount_generation(&server, 2).await;
        let mut source_client =
            wanikani_source::Client::with_base_url("supplied-wk", &format!("{}/v2/", server.uri()))
                .unwrap();
        let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
        let (_, cache) = embedding_fixture();
        let mut flags = embedding_flags();
        flags.application.cache_max_age_seconds = Some(0);
        let config = config(dir.path(), flags);
        let mut previous = source_fixture();
        for assignment in &mut previous.assignments {
            if assignment.subject_id == 3 {
                assignment.hidden = true;
            }
        }
        assert!(
            LearnerInventory::from_wanikani(&previous, &config.pipeline.knowledge_policy)
                .unwrap()
                .vocabulary
                .is_empty()
        );
        store
            .begin_sync()
            .unwrap()
            .replace(previous.into())
            .unwrap();
        for call in 1..=2 {
            let mut events = Vec::new();
            let report = run_story(
                &config,
                embedding_fixture().0,
                Some(&store),
                Some(&mut source_client),
                &client,
                None,
                Some(&cache),
                SystemTime::now().into(),
                7,
                |event| events.push(event),
            )
            .await
            .unwrap();
            assert_eq!(server.received_requests().await.unwrap().len(), call * 5);
            let published = store.load().unwrap();
            published.validate().unwrap();
            assert_eq!(published.subjects.len(), 4);
            assert_eq!(
                report.generated.passages()[0].text,
                "猫です。寝ます。朝です。"
            );
            assert!(
                report
                    .selection
                    .vocabulary_ids
                    .contains(&"wanikani:3".into())
            );
            assert_eq!(report.selection.selector_revision, "builtin-v2");
            assert_eq!(report.warnings.len(), 1);
            let missing = yomibu_core::domain::embedding::EmbeddingError::Missing;
            assert_eq!(
                report.warnings[0].message,
                format!("{missing} Using built-in selection.")
            );
            let embedding = report
                .modules
                .iter()
                .find(|m| m.metadata.id == ModuleId::Embeddings)
                .unwrap();
            assert!(matches!(embedding.state, ModuleState::Unavailable { .. }));
            assert_completed_steps(&events);
            store.begin_sync().unwrap();
        }
        for request in server.received_requests().await.unwrap() {
            if request.method.as_str() == "GET" {
                assert!(request.url.path().starts_with("/v2/"));
                assert_eq!(request.headers["authorization"], "Bearer supplied-wk");
            }
        }
    });
}

#[tokio::test]
async fn local_generation_acquisition_failure_prevents_source_refresh() {
    let server = MockServer::start().await;
    for credential_fails in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("fresh");
        let store = FileLearningStore::new(&data);
        let lookups = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&lookups);
        let mut credentials = supplied_credentials(Some("wk".into()), None);
        credentials.supply_with(GENERATION_KEY, move || {
            observed.fetch_add(1, Ordering::SeqCst);
            if credential_fails {
                Err(CredentialError::StoreUnavailable)
            } else {
                Ok(Some("ai".into()))
            }
        });
        let app = LocalApp::new(config(&data, ProcessOverrides::default()), credentials)
            .with_endpoints(ServiceEndpoints {
                openai: "unusable endpoint".into(),
                ..endpoints(&server)
            });
        assert_eq!(lookups.load(Ordering::SeqCst), 0);
        let mut events = Vec::new();
        let error = unsafe {
            app.story(&store, SystemTime::now().into(), 7, |event| {
                events.push(event)
            })
        }
        .await
        .unwrap_err();
        assert!(matches!(
            (credential_fails, error),
            (
                true,
                ApplicationError::Credential(CredentialError::StoreUnavailable)
            ) | (
                false,
                ApplicationError::Provider(ProviderError::InvalidBaseUrl)
            )
        ));
        assert_eq!(lookups.load(Ordering::SeqCst), 1);
        assert!(matches!(
            events.as_slice(),
            [ProgressEvent::Started { step: Step::Inputs }]
        ));
        assert!(server.received_requests().await.unwrap().is_empty());
        assert!(!data.exists());
    }
}

#[tokio::test]
async fn supplied_story_inputs_ignore_local_paths_and_choose_source_participation() {
    let dir = tempfile::tempdir().unwrap();
    let store = InMemoryLearningStore::new();
    let completed = seed_store(&store);
    let server = MockServer::start().await;
    mount_generation(&server, 7).await;
    let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
    let mut source_client =
        wanikani_source::Client::with_base_url("wk", &format!("{}/v2/", server.uri())).unwrap();
    for (use_manual, use_store, use_client, age_minutes, sync) in [
        (true, false, false, 10, true),
        (true, false, true, 120, true),
        (true, true, false, 120, true),
        (false, true, false, 120, true),
        (false, true, false, 10, true),
        (false, true, true, 10, true),
        (false, true, true, 120, false),
    ] {
        let now = completed + chrono::Duration::minutes(age_minutes);
        let mut flags = ProcessOverrides {
            request: Some(dir.path().join("missing-request.json")),
            ..Default::default()
        };
        flags.application.inventory = Some(dir.path().join("missing-inventory.json"));
        flags.application.dictionary_dir = Some(dir.path().join("missing-dictionary"));
        flags.application.wanikani_cache = (!use_store).then(|| dir.path().join("missing-cache"));
        flags.application.sync = Some(sync);
        let base = config(dir.path(), flags);
        let mut invocation = Invocation::default();
        invocation.story.select = Some(16);
        invocation.story.seed = Patch::Set(7);
        let config = base.for_invocation(invocation, &Operation::Story).unwrap();
        let manual = use_manual.then(|| {
            LearnerInventory::from_manual(
                serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json"))
                    .unwrap(),
            )
            .unwrap()
        });
        let mut request: StoryRequest =
            serde_json::from_slice(include_bytes!("../../../tests/fixtures/story/request.json"))
                .unwrap();
        if !use_manual {
            request.targets.vocabulary.clear();
            request.targets.grammar.clear();
        }
        let mut events = Vec::new();
        let report = run_story(
            &config,
            StoryInputs { request, manual },
            use_store.then_some(&store),
            use_client.then_some(&mut source_client),
            &client,
            None,
            None,
            now,
            1,
            |event| events.push(event),
        )
        .await
        .unwrap();
        assert_eq!(
            report.generated.passages()[0].text,
            "猫です。寝ます。朝です。"
        );
        assert_eq!(report.selection.seed, 7);
        assert_eq!(*store.load().unwrap(), source_fixture());
        let reason = if !use_store {
            "Manual inventory selected"
        } else if age_minutes < 60 {
            "Cache is fresh"
        } else {
            "Sync disabled or no WaniKani client supplied"
        };
        assert!(events.iter().any(|event| matches!(event,
            ProgressEvent::Skipped { step: Step::Sync, reason: actual } if actual == reason)));
        if use_store && age_minutes >= 60 {
            assert!(
                report
                    .warnings
                    .iter()
                    .any(|warning| warning.module == ModuleId::Sync
                        && warning.message.contains("source client"))
            );
        }
        let selected = &report.selection.vocabulary_ids;
        assert_eq!(
            selected.iter().any(|id| id.starts_with("wanikani:")),
            use_store
        );
        if use_manual {
            assert_eq!(report.request.targets.vocabulary, ["sleep", "cat"]);
            assert_eq!(&selected[..2], ["sleep", "cat"]);
        }
        assert_completed_steps(&events);
        assert!(
            matches!(events.last(), Some(ProgressEvent::Skipped { step: Step::Assessment, reason }) if reason == "No analyzer supplied")
        );
        assert!(
            !report
                .warnings
                .iter()
                .any(|warning| warning.module == ModuleId::Assessment)
        );
        assert!(matches!(
            report
                .modules
                .iter()
                .find(|m| m.metadata.id == ModuleId::Assessment)
                .unwrap()
                .state,
            ModuleState::NotConfigured
        ));
        assert!(
            report
                .assessments
                .iter()
                .flat_map(|passage| &passage.sentences)
                .all(|sentence| matches!(
                    sentence.assessment.assessment,
                    CandidateAssessment::NotRun
                ))
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, ProgressEvent::Started { step: Step::Inputs }))
                .count(),
            1
        );
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 7);
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[tokio::test]
async fn invalid_supplied_inputs_do_not_access_stores_or_http() {
    use std::cell::Cell;
    use yomibu_components::in_memory_learning_store::{InMemoryStoreError, InMemorySyncWriter};

    #[derive(Debug)]
    enum InvalidInput {
        RequestVersion,
        SelectionLimit,
        ManualInventory,
        MissingKnowledge,
    }

    struct ObservedStore(InMemoryLearningStore, Cell<usize>);
    impl LearningStore for ObservedStore {
        type ReadError = InMemoryStoreError;
        type WriteError = InMemoryStoreError;
        type Writer = InMemorySyncWriter;
        fn load(&self) -> Result<Arc<WaniKaniSyncData>, Self::ReadError> {
            self.1.set(self.1.get() + 1);
            self.0.load()
        }
        fn begin_sync(&self) -> Result<Self::Writer, Self::WriteError> {
            self.1.set(self.1.get() + 1);
            self.0.begin_sync()
        }
        fn is_missing(&self, error: &Self::ReadError) -> bool {
            self.0.is_missing(error)
        }
        fn is_locked(&self, error: &Self::WriteError) -> bool {
            self.0.is_locked(error)
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
    let mut source_client =
        wanikani_source::Client::with_base_url("wk", &format!("{}/v2/", server.uri())).unwrap();
    for invalid in [
        InvalidInput::RequestVersion,
        InvalidInput::SelectionLimit,
        InvalidInput::ManualInventory,
        InvalidInput::MissingKnowledge,
    ] {
        let store = ObservedStore(InMemoryLearningStore::new(), Cell::new(0));
        let base = config(dir.path(), ProcessOverrides::default());
        let mut invocation = Invocation::default();
        invocation.story.select = Some(match invalid {
            InvalidInput::SelectionLimit => 1,
            _ => 16,
        });
        let config = base.for_invocation(invocation, &Operation::Story).unwrap();
        let mut request: StoryRequest =
            serde_json::from_slice(include_bytes!("../../../tests/fixtures/story/request.json"))
                .unwrap();
        if matches!(invalid, InvalidInput::RequestVersion) {
            request.version = 0;
        }
        let manual = if matches!(invalid, InvalidInput::ManualInventory) {
            let mut manual = LearnerInventory::from_manual(
                serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json"))
                    .unwrap(),
            )
            .unwrap();
            manual.vocabulary[0].id.clear();
            Some(manual)
        } else {
            None
        };
        let error = run_story(
            &config,
            StoryInputs { request, manual },
            (!matches!(invalid, InvalidInput::MissingKnowledge)).then_some(&store),
            Some(&mut source_client),
            &client,
            None,
            None,
            SystemTime::now().into(),
            1,
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(store.1.get(), 0, "{invalid:?}: {error:?}");
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "{invalid:?}"
        );
        assert!(store.0.load().is_err(), "{invalid:?}");
        assert!(match invalid {
            InvalidInput::RequestVersion | InvalidInput::SelectionLimit =>
                matches!(error, ApplicationError::Story(StoryError::Invalid(_))),
            InvalidInput::ManualInventory => matches!(error, ApplicationError::Inventory(_)),
            InvalidInput::MissingKnowledge => matches!(error, ApplicationError::Setup { issues }
                if issues.len() == 1 && issues[0].module == ModuleId::Knowledge),
        });
    }
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[tokio::test]
async fn cancelling_supplied_source_refresh_preserves_data_and_releases_the_writer() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    source_stores!(dir, store, {
        let completed = seed_store(&store);
        let before_bytes = std::fs::read(dir.path().join("wanikani.json")).ok();
        let server = MockServer::start().await;
        mount_generation(&server, 0).await;
        let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
        let mut source_client =
            wanikani_source::Client::with_base_url("wk", &format!("{}/v2/", server.uri())).unwrap();
        let config = config(dir.path(), ProcessOverrides::default());
        let mut operation = Box::pin(run_story(
            &config,
            source_story_inputs(),
            Some(&store),
            Some(&mut source_client),
            &client,
            None,
            None,
            completed + chrono::Duration::hours(2),
            7,
            |_| {},
        ));
        assert!(matches!(
            operation
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        assert!(store.is_locked(&store.begin_sync().err().unwrap()));
        assert_original_source(&store, dir.path(), before_bytes.clone());
        drop(operation);
        store.begin_sync().unwrap();
        assert_original_source(&store, dir.path(), before_bytes);
    });
}

#[tokio::test]
async fn two_keys_generate_once_without_optional_resources_and_the_second_run_uses_cache() {
    source_stores!(dir, store, {
        let server = MockServer::start().await;
        mount_source(&server, 1).await;
        mount_generation(&server, 2).await;
        let app = LocalApp::new(
            config(dir.path(), ProcessOverrides::default()),
            supplied_credentials(Some("synthetic-wk".into()), Some("synthetic-ai".into())),
        )
        .with_endpoints(ServiceEndpoints {
            wanikani: format!("{}/v2/", server.uri()),
            openai: format!("{}/v1/", server.uri()),
        });
        let now = SystemTime::now().into();
        let mut events = Vec::new();
        let report = unsafe { app.story(&store, now, 7, |event| events.push(event)) }
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
        let cached_report = unsafe { app.story(&store, SystemTime::now().into(), 7, |_| {}) }
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
    });
}

fn write_cache(dir: &std::path::Path) -> chrono::DateTime<chrono::Utc> {
    std::fs::write(
        dir.join("wanikani.json"),
        include_bytes!("../../../tests/fixtures/mixed.json"),
    )
    .unwrap();
    yomibu_components::file_learning_store::cache::load(dir)
        .unwrap()
        .sync_completed_at
}
fn endpoints(server: &MockServer) -> ServiceEndpoints {
    ServiceEndpoints {
        wanikani: format!("{}/v2/", server.uri()),
        openai: format!("{}/v1/", server.uri()),
    }
}

fn source_fixture() -> WaniKaniSyncData {
    let envelope: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../tests/fixtures/mixed.json")).unwrap();
    serde_json::from_value(envelope["snapshot"].clone()).unwrap()
}

fn source_story_inputs() -> StoryInputs {
    let mut request: StoryRequest =
        serde_json::from_str(include_str!("../../../tests/fixtures/story/request.json")).unwrap();
    request.targets.vocabulary.clear();
    request.targets.grammar.clear();
    StoryInputs {
        request,
        manual: None,
    }
}

fn seed_store(store: &impl LearningStore) -> chrono::DateTime<chrono::Utc> {
    let data = source_fixture();
    let completed = data.sync_completed_at;
    store.begin_sync().unwrap().replace(data.into()).unwrap();
    completed
}

fn assert_original_source(
    store: &impl LearningStore,
    dir: &std::path::Path,
    bytes: Option<Vec<u8>>,
) {
    assert_eq!(*store.load().unwrap(), source_fixture());
    assert_eq!(std::fs::read(dir.join("wanikani.json")).ok(), bytes);
}

#[tokio::test]
async fn cache_is_fresh_until_the_one_hour_boundary_then_refreshes() {
    source_stores!(dir, store, {
        let completed = seed_store(&store);
        let server = MockServer::start().await;
        mount_source(&server, 1).await;
        mount_generation(&server, 2).await;
        let app = LocalApp::new(
            config(dir.path(), ProcessOverrides::default()),
            supplied_credentials(Some("wk".into()), Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        unsafe {
            app.story(
                &store,
                completed + chrono::Duration::seconds(3599),
                1,
                |_| {},
            )
        }
        .await
        .unwrap();
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
        unsafe { app.story(&store, completed + chrono::Duration::hours(1), 1, |_| {}) }
            .await
            .unwrap();
        assert_eq!(server.received_requests().await.unwrap().len(), 6);
        assert!(store.load().unwrap().sync_completed_at > completed);
    });
}

#[tokio::test]
async fn cache_from_a_future_completion_time_is_refreshed() {
    source_stores!(dir, store, {
        let completed = seed_store(&store);
        let server = MockServer::start().await;
        mount_source(&server, 1).await;
        mount_generation(&server, 1).await;
        let app = LocalApp::new(
            config(dir.path(), ProcessOverrides::default()),
            supplied_credentials(Some("wk".into()), Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        unsafe { app.story(&store, completed - chrono::Duration::seconds(1), 1, |_| {}) }
            .await
            .unwrap();
        assert_eq!(server.received_requests().await.unwrap().len(), 5);
    });
}

#[tokio::test]
async fn a_recent_cache_without_usable_vocabulary_is_refreshed_even_under_the_lock() {
    source_stores!(dir, store, {
        for replaced_by_another_writer in [false, true] {
            let completed = seed_store(&store);
            let now = completed + chrono::Duration::hours(2);
            let mut empty = source_fixture();
            for assignment in &mut empty.assignments {
                assignment.started_at = None;
                assignment.passed_at = None;
                assignment.burned_at = None;
                assignment.srs_stage = 0;
            }
            empty.sync_completed_at = now;
            let empty = Arc::new(empty);
            if !replaced_by_another_writer {
                store
                    .begin_sync()
                    .unwrap()
                    .replace(Arc::clone(&empty))
                    .unwrap();
            }
            let server = MockServer::start().await;
            mount_source(&server, 1).await;
            mount_generation(&server, 1).await;
            let app = LocalApp::new(
                config(dir.path(), ProcessOverrides::default()),
                supplied_credentials(Some("wk".into()), Some("ai".into())),
            )
            .with_endpoints(endpoints(&server));
            let report = unsafe {
                app.story(&store, now, 1, |event| {
                    if replaced_by_another_writer
                        && matches!(event, ProgressEvent::Started { step: Step::Sync })
                    {
                        store
                            .begin_sync()
                            .unwrap()
                            .replace(Arc::clone(&empty))
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
    });
}

#[tokio::test]
async fn usable_old_cache_needs_no_source_key_and_disabled_sync_never_initializes_it() {
    source_stores!(dir, store, {
        let completed = seed_store(&store);
        let before_bytes = std::fs::read(dir.path().join("wanikani.json")).ok();
        let server = MockServer::start().await;
        mount_generation(&server, 2).await;
        for (flags, key) in [
            (ProcessOverrides::default(), None),
            (
                ProcessOverrides {
                    disable: vec![ModuleId::Sync],
                    ..Default::default()
                },
                Some("invalid\ncredential".into()),
            ),
        ] {
            let app = LocalApp::new(
                config(dir.path(), flags),
                supplied_credentials(key, Some("ai".into())),
            )
            .with_endpoints(endpoints(&server));
            let report =
                unsafe { app.story(&store, completed + chrono::Duration::hours(2), 1, |_| {}) }
                    .await
                    .unwrap();
            assert_eq!(report.warnings.len(), 1);
        }
        assert_original_source(&store, dir.path(), before_bytes);
    });
}

#[tokio::test]
async fn invalid_source_credentials_are_ignored_until_a_refresh_needs_them() {
    source_stores!(dir, store, {
        use yomibu::{application::Secret, configuration::components::SOURCE_KEY};
        let server = MockServer::start().await;
        mount_generation(&server, 4).await;
        for cli in [false, true] {
            for (age_hours, sync, succeeds) in [(0, true, true), (2, false, true), (2, true, false)]
            {
                let completed = seed_store(&store);
                let before_bytes = std::fs::read(dir.path().join("wanikani.json")).ok();
                let before = store.load().unwrap();
                let lock = dir.path().join("wanikani.json.lock");
                if lock.exists() {
                    std::fs::remove_file(&lock).unwrap();
                }
                let mut credentials = supplied_credentials(None, Some("ai".into()));
                if cli {
                    credentials.set_cli(SOURCE_KEY, Secret::invalid_encoding());
                    credentials.set_environment(SOURCE_KEY, "unused-key".into());
                } else {
                    credentials.set_environment(SOURCE_KEY, Secret::invalid_encoding());
                    credentials.supply(SOURCE_KEY, "unused-key".into());
                }
                let mut flags = ProcessOverrides::default();
                flags.application.sync = Some(sync);
                let app = LocalApp::new(config(dir.path(), flags), credentials)
                    .with_endpoints(endpoints(&server));
                let result = unsafe {
                    app.story(
                        &store,
                        completed + chrono::Duration::hours(age_hours),
                        1,
                        |_| {},
                    )
                }
                .await;
                if succeeds {
                    let report = result.unwrap();
                    assert_eq!(
                        report.generated.passages()[0].text,
                        "猫です。寝ます。朝です。"
                    );
                } else {
                    assert!(
                        matches!(result, Err(ApplicationError::Credential(_))),
                        "{result:?}"
                    );
                }
                assert_eq!(store.load().unwrap(), before);
                assert_original_source(&store, dir.path(), before_bytes);
                assert!(!dir.path().join("wanikani.json.lock").exists());
            }
        }
        assert_eq!(server.received_requests().await.unwrap().len(), 4);
    });
}

#[tokio::test]
async fn temporary_refresh_failure_uses_cache_but_authentication_is_fatal_and_preserves_bytes() {
    source_stores!(dir, store, {
        for (status, supplied) in [(500, false), (401, false), (500, true), (401, true)] {
            let completed = seed_store(&store);
            let before_bytes = std::fs::read(dir.path().join("wanikani.json")).ok();
            let server = MockServer::start().await;
            Mock::given(path("/v2/user"))
                .respond_with(ResponseTemplate::new(status))
                .expect(if status == 500 { 3 } else { 1 })
                .mount(&server)
                .await;
            if status == 500 {
                mount_generation(&server, 1).await;
            }
            let config = config(dir.path(), ProcessOverrides::default());
            let now = completed + chrono::Duration::hours(2);
            let result = if supplied {
                let client = Client::with_base_url("ai", &endpoints(&server).openai).unwrap();
                let mut source_client =
                    wanikani_source::Client::with_base_url("wk", &endpoints(&server).wanikani)
                        .unwrap();
                run_story(
                    &config,
                    source_story_inputs(),
                    Some(&store),
                    Some(&mut source_client),
                    &client,
                    None,
                    None,
                    now,
                    1,
                    |_| {},
                )
                .await
            } else {
                let app = LocalApp::new(
                    config,
                    supplied_credentials(Some("wk".into()), Some("ai".into())),
                )
                .with_endpoints(endpoints(&server));
                unsafe { app.story(&store, now, 1, |_| {}) }.await
            };
            if status == 500 {
                assert_eq!(result.unwrap().warnings.len(), 1);
            } else {
                assert!(matches!(
                    result,
                    Err(ApplicationError::Source(
                        yomibu_components::wanikani_source::Error::Authentication
                    ))
                ));
            }
            assert_original_source(&store, dir.path(), before_bytes);
            store.begin_sync().unwrap();
        }
    });
}

#[tokio::test]
async fn writer_contention_can_use_valid_cache_but_expired_access_cannot() {
    source_stores!(dir, store, {
        let completed = seed_store(&store);
        let server = MockServer::start().await;
        mount_generation(&server, 1).await;
        let app = LocalApp::new(
            config(dir.path(), ProcessOverrides::default()),
            supplied_credentials(Some("wk".into()), Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        let guard = store.begin_sync().unwrap();
        let mut events = Vec::new();
        let report = unsafe {
            app.story(&store, completed + chrono::Duration::hours(2), 1, |event| {
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
        let mut expired = source_fixture();
        expired.learner.subscription.period_ends_at =
            Some(completed + chrono::Duration::minutes(10));
        expired.learner.subscription.active = true;
        store.begin_sync().unwrap().replace(expired.into()).unwrap();
        let guard = store.begin_sync().unwrap();
        assert!(matches!(
            unsafe { app.story(&store, completed + chrono::Duration::minutes(10), 1, |_| {}) }
                .await,
            Err(ApplicationError::Persistence(
                yomibu_components::file_learning_store::cache::WriteError::Locked
            ) | ApplicationError::InMemoryStore(
                yomibu_components::in_memory_learning_store::InMemoryStoreError::Locked
            ))
        ));
        drop(guard);
        let app = LocalApp::new(
            config(dir.path(), ProcessOverrides::default()),
            supplied_credentials(None, Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        assert!(
            unsafe { app.story(&store, completed + chrono::Duration::minutes(10), 1, |_| {}) }
                .await
                .is_err()
        );
    });
}

#[tokio::test]
async fn explicit_missing_dictionary_warns_but_absent_default_skips_assessment() {
    let server = MockServer::start().await;
    for source in ["flag", "environment", "file", "default", "broken_default"] {
        let dir = tempfile::tempdir().unwrap();
        let store = FileLearningStore::new(dir.path());
        let inventory = dir.path().join("inventory.json");
        std::fs::write(
            &inventory,
            include_bytes!("../../../tests/fixtures/story/inventory.json"),
        )
        .unwrap();
        let missing = dir.path().join("missing");
        let mut flags = ProcessOverrides::default();
        flags.application.inventory = Some(inventory);
        let mut input = ConfigurationInput {
            data_dir: Some(dir.path().into()),
            flags,
            config: None,
            home: None,
            environment: BTreeMap::new(),
        };
        match source {
            "flag" => input.flags.application.dictionary_dir = Some(missing),
            "environment" => {
                input.environment.insert(
                    "YOMIBU_DICTIONARY_DIR".into(),
                    missing.to_str().unwrap().into(),
                );
            }
            "file" => {
                let file = dir.path().join("config.toml");
                std::fs::write(&file, "[application]\ndictionary_dir = 'missing'\n").unwrap();
                input.config = Some(file);
            }
            "broken_default" => {
                std::os::unix::fs::symlink(&missing, dir.path().join("dictionaries")).unwrap();
            }
            _ => {}
        }
        let app = LocalApp::new(
            Configuration::load(input, &yomibu::application::Operation::Story).unwrap(),
            supplied_credentials(None, Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        if source == "flag" {
            Mock::given(method("POST"))
                .and(path("/v1/responses"))
                .respond_with(ResponseTemplate::new(401))
                .expect(1)
                .mount(&server)
                .await;
            let mut events = Vec::new();
            let error = unsafe {
                app.story(&store, SystemTime::now().into(), 1, |event| {
                    events.push(event)
                })
            }
            .await
            .unwrap_err();
            assert!(matches!(error, ApplicationError::Generation(_)));
            // Local acquisition starts Assessment before opening the dictionary.
            assert!(!events.iter().any(|event| matches!(
                event,
                ProgressEvent::Started {
                    step: Step::Assessment
                } | ProgressEvent::Completed {
                    step: Step::Assessment,
                    ..
                } | ProgressEvent::Skipped {
                    step: Step::Assessment,
                    ..
                }
            )));
            server.reset().await;
            mount_generation(&server, 5).await;
        }
        let report = unsafe { app.story(&store, SystemTime::now().into(), 1, |_| {}) }
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
            yomibu_core::domain::candidate::CandidateAssessment::NotRun
        ));
    }
}

#[tokio::test]
async fn optional_resources_enhance_when_available_and_failures_preserve_generation() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(dir.path());
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 2).await;
    for assessment in [true, false] {
        let mut flags = ProcessOverrides {
            enable: vec![ModuleId::Embeddings],
            disable: if assessment {
                Vec::new()
            } else {
                vec![ModuleId::Assessment]
            },
            ..Default::default()
        };
        flags.application.inventory = Some(inventory.clone());
        flags.application.dictionary_dir = Some(dir.path().join("managed"));
        flags.invocation.story.topic = Patch::Set("cat".into());
        flags.invocation.pipeline.embedding.provider =
            Patch::Set(yomibu::configuration::EmbeddingProvider::LexicalBaseline);
        let app = LocalApp::new(
            config(dir.path(), flags),
            supplied_credentials(None, Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        let report = unsafe { app.story(&store, SystemTime::now().into(), 1, |_| {}) }
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
            yomibu_core::domain::candidate::CandidateAssessment::NotRun
        ));
    }
    assert!(dir.path().join("embeddings.json").exists());
}

#[tokio::test]
async fn no_topic_skips_embeddings_and_the_explicit_retrieval_command_requires_a_provider() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(dir.path());
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let mut flags = ProcessOverrides {
        enable: vec![ModuleId::Embeddings],
        ..Default::default()
    };
    flags.application.inventory = Some(inventory);
    let app = LocalApp::new(
        config(dir.path(), flags),
        supplied_credentials(None, Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    let report = unsafe { app.story(&store, SystemTime::now().into(), 1, |_| {}) }
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
async fn invalid_component_options_report_unavailable_instead_of_missing_setup() {
    use yomibu_components::http_embeddings as http;
    use yomibu_core::component::options::OptionKey;

    let dir = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(dir.path());
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 2).await;
    for name in [http::MODEL.name, http::ENDPOINT.name] {
        let mut flags = ProcessOverrides {
            enable: vec![ModuleId::Embeddings],
            ..Default::default()
        };
        flags.application.inventory = Some(inventory.clone());
        flags.invocation.story.topic = Patch::Set("cat".into());
        flags.invocation.pipeline.embedding.provider =
            Patch::Set(yomibu::configuration::EmbeddingProvider::Local);
        let mut config = config(dir.path(), flags);
        config
            .pipeline
            .options
            .set(http::MODEL, "test-model".into())
            .unwrap();
        config
            .pipeline
            .options
            .set(http::REVISION, "pinned".into())
            .unwrap();
        config.pipeline.options.set(http::DIMENSIONS, 2).unwrap();
        config
            .pipeline
            .options
            .set(
                OptionKey::<usize>::new(name.component, name.option, "Wrong type"),
                1,
            )
            .unwrap();
        let app = LocalApp::new(config, supplied_credentials(None, Some("ai".into())))
            .with_endpoints(endpoints(&server));
        let report = unsafe { app.story(&store, SystemTime::now().into(), 1, |_| {}) }
            .await
            .unwrap();
        let state = &report
            .modules
            .iter()
            .find(|module| module.metadata.id == ModuleId::Embeddings)
            .unwrap()
            .state;
        assert!(
            matches!(state, ModuleState::Unavailable { error } if error.contains("Wrong component option type")),
            "{state:?}"
        );
        assert_eq!(report.warnings.len(), 1);
        assert!(!dir.path().join("embeddings.json").exists());
    }
}

#[tokio::test]
async fn optional_embedding_failure_warns_while_explicit_retrieval_returns_the_error() {
    use yomibu::{
        application::Secret,
        configuration::components::{EMBEDDING_KEY, GENERATION_KEY},
    };
    let dir = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(dir.path());
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 3).await;
    for (authorized, credential) in [
        (false, Some(Secret::invalid_encoding())),
        (true, Some(Secret::invalid_encoding())),
        (true, None),
    ] {
        let missing = credential.is_none();
        let mut flags = ProcessOverrides {
            enable: vec![ModuleId::Embeddings],
            ..Default::default()
        };
        flags.application.inventory = Some(inventory.clone());
        flags.application.allow_embedding_call = Some(authorized);
        flags.invocation.story.topic = Patch::Set("cat".into());
        flags.invocation.pipeline.embedding.provider =
            Patch::Set(yomibu::configuration::EmbeddingProvider::Openai);
        flags
            .invocation
            .pipeline
            .options
            .set(EMBEDDING_MODEL, "embedding-model".into())
            .unwrap();
        flags
            .invocation
            .pipeline
            .options
            .set(EMBEDDING_REVISION, "pinned".into())
            .unwrap();
        flags
            .invocation
            .pipeline
            .options
            .set(EMBEDDING_DIMENSIONS, 2)
            .unwrap();
        let mut credentials = Credentials::default();
        credentials.set_cli(GENERATION_KEY, "ai".into());
        if let Some(credential) = credential {
            credentials.set_environment(EMBEDDING_KEY, credential);
        }
        let app = LocalApp::new(config(dir.path(), flags), credentials)
            .with_endpoints(endpoints(&server));
        let now = SystemTime::now().into();
        let error = app.prepare_retrieval(now).await.unwrap_err();
        if missing {
            assert!(
                matches!(error, ApplicationError::MissingCredential(_)),
                "{error:?}"
            );
        } else if authorized {
            assert!(
                matches!(error, ApplicationError::Credential(_)),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(
                    error,
                    ApplicationError::ResourceConfiguration(
                        "Hosted embeddings require --allow-embedding-call."
                    )
                ),
                "{error:?}"
            );
        }
        let report = unsafe { app.story(&store, now, 1, |_| {}) }.await.unwrap();
        assert_eq!(report.selection.selector_revision, "builtin-v2");
        assert_eq!(report.warnings.len(), 1);
        let state = &report
            .modules
            .iter()
            .find(|report| report.metadata.id == ModuleId::Embeddings)
            .unwrap()
            .state;
        if missing || !authorized {
            assert!(matches!(state, ModuleState::NotConfigured));
        } else {
            assert!(matches!(state, ModuleState::Unavailable { .. }));
        }
        assert!(report.warnings[0].message.contains(&error.to_string()));
        assert!(!dir.path().join("embeddings.json").exists());
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn partial_embedding_settings_do_not_silently_reuse_another_cached_model() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(dir.path());
    let inventory = dir.path().join("inventory.json");
    std::fs::write(
        &inventory,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let mut flags = ProcessOverrides::default();
    flags.application.inventory = Some(inventory.clone());
    flags.invocation.story.topic = Patch::Set("cat".into());
    flags.invocation.pipeline.embedding.provider =
        Patch::Set(yomibu::configuration::EmbeddingProvider::LexicalBaseline);
    LocalApp::new(config(dir.path(), flags), Credentials::default())
        .prepare_retrieval(std::time::SystemTime::now().into())
        .await
        .unwrap();
    let before = std::fs::read(dir.path().join("embeddings.json")).unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let mut flags = ProcessOverrides {
        enable: vec![ModuleId::Embeddings],
        ..Default::default()
    };
    flags.application.inventory = Some(inventory);
    flags.invocation.story.topic = Patch::Set("cat".into());
    flags
        .invocation
        .pipeline
        .options
        .set(EMBEDDING_MODEL, "another-model".into())
        .unwrap();
    let app = LocalApp::new(
        config(dir.path(), flags),
        supplied_credentials(None, Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    let report = unsafe { app.story(&store, SystemTime::now().into(), 1, |_| {}) }
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
    use yomibu_components::file_embedding_cache::EmbeddingCacheFile;
    let (StoryInputs { request, manual }, cache) = embedding_fixture();
    let inventory = manual.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let inventory_path = dir.path().join("inventory.json");
    std::fs::write(
        &inventory_path,
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let request_path = dir.path().join("request.json");
    let request_bytes = include_bytes!("../../../tests/fixtures/story/request.json");
    std::fs::write(&request_path, request_bytes).unwrap();
    let cache_path = dir.path().join("embeddings.json");
    EmbeddingCacheFile::new(&cache_path).save(&cache).unwrap();
    let before = std::fs::read(&cache_path).unwrap();
    let mut flags = embedding_flags();
    flags.request = Some(request_path.clone());
    flags.application.inventory = Some(inventory_path);
    let config = config(dir.path(), flags);
    let app = LocalApp::new(config.clone(), Credentials::default());
    let now = SystemTime::now().into();
    let reused = app.prepare_retrieval(now).await.unwrap();
    assert_eq!(
        serde_json::to_value(reused).unwrap(),
        serde_json::to_value(&cache).unwrap()
    );
    let server = MockServer::start().await;
    mount_generation(&server, 1).await;
    let client = Client::with_base_url("ai", &endpoints(&server).openai).unwrap();
    let mut events = Vec::new();
    let supplied = run_story::<InMemoryLearningStore>(
        &config,
        StoryInputs {
            request,
            manual: Some(inventory),
        },
        None,
        None,
        &client,
        None,
        None,
        now,
        7,
        |event| events.push(event),
    )
    .await
    .unwrap();
    assert_eq!(supplied.selection.selector_revision, "builtin-v2");
    assert!(supplied.selection.embedding_model.is_none());
    assert_eq!(supplied.warnings.len(), 1);
    assert_eq!(
        supplied.warnings[0].message,
        "No embedding cache supplied. Using built-in selection."
    );
    assert_completed_steps(&events);
    assert_eq!(std::fs::read(&cache_path).unwrap(), before);
    let preview = app.preview(now, 7).unwrap();
    assert_eq!(preview.selection.embedding_model, Some(cache.model));
    assert!(preview.warnings.is_empty());
    let mut invocation = yomibu::configuration::Invocation::default();
    invocation.pipeline.selection.embedding_steps = Some(vec![
        yomibu::configuration::SelectionStep::EmbeddingRank,
        yomibu::configuration::SelectionStep::SeededOrder,
    ]);
    let composed = app
        .for_invocation(invocation, &yomibu::application::Operation::Preview)
        .unwrap()
        .preview(now, 7)
        .unwrap();
    assert_eq!(
        composed.selection.selector_revision,
        "inventory-similarity-seeded-v1"
    );
    assert_eq!(
        composed.selection.vocabulary_ids,
        ["sleep", "cat", "dog", "walk"]
    );
    assert_eq!(
        composed.selection.embedding_model,
        preview.selection.embedding_model
    );
    assert!(composed.warnings.is_empty());

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
    let store = FileLearningStore::at_path(&cache);
    let completed = write_cache(dir.path());
    std::fs::rename(dir.path().join("wanikani.json"), &cache).unwrap();
    let server = MockServer::start().await;
    mount_source(&server, 1).await;
    mount_generation(&server, 1).await;
    let mut flags = ProcessOverrides::default();
    flags.application.wanikani_cache = Some(cache.clone());
    let app = LocalApp::new(
        config(&dir.path().join("data"), flags),
        supplied_credentials(Some("wk".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    unsafe { app.story(&store, completed + chrono::Duration::hours(2), 1, |_| {}) }
        .await
        .unwrap();
    assert_ne!(
        std::fs::read(&cache).unwrap(),
        include_bytes!("../../../tests/fixtures/mixed.json")
    );
    assert!(!dir.path().join("data/wanikani.json").exists());
    let inventory = dir.path().join("inventory.json");
    std::fs::write(&inventory, r#"{"version":1,"vocabulary":[{"id":"","written_form":"猫","readings":[],"meanings":[],"direct_object":null}],"grammar_declarations":[],"grammar_bindings":[]}"#).unwrap();
    let mut flags = ProcessOverrides::default();
    flags.application.inventory = Some(inventory);
    flags.application.wanikani_cache = Some(dir.path().join("absent/wanikani.json"));
    let store = FileLearningStore::new(dir.path().join("absent"));
    let app = LocalApp::new(
        config(&dir.path().join("absent"), flags),
        supplied_credentials(Some("wk".into()), Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    assert!(matches!(
        unsafe { app.story(&store, SystemTime::now().into(), 1, |_| {}) }.await,
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
        let store = FileLearningStore::new(&data);
        let app = LocalApp::new(
            config(
                &data,
                ProcessOverrides {
                    request: Some(request.clone()),
                    ..Default::default()
                },
            ),
            supplied_credentials(Some("wk".into()), Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        assert!(matches!(
            unsafe { app.story(&store, SystemTime::now().into(), 1, |_| {}) }.await,
            Err(ApplicationError::InvalidJson { .. } | ApplicationError::Story(_))
        ));
        assert!(server.received_requests().await.unwrap().is_empty());
        assert!(!data.exists());
    }
}

#[tokio::test]
async fn refreshed_expired_access_is_rejected_before_replacing_a_usable_cache() {
    source_stores!(dir, store, {
        let completed = seed_store(&store);
        let before_bytes = std::fs::read(dir.path().join("wanikani.json")).ok();
        let now = completed + chrono::Duration::hours(2);
        let mut user: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/wanikani/user.json"))
                .unwrap();
        user["data"]["subscription"]["active"] = json!(true);
        user["data"]["subscription"]["period_ends_at"] = json!(now);
        let server = MockServer::start().await;
        mount_source_with_user(&server, &user.to_string(), 1).await;
        let app = LocalApp::new(
            config(dir.path(), ProcessOverrides::default()),
            supplied_credentials(Some("wk".into()), Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        assert!(matches!(
            unsafe { app.story(&store, now, 1, |_| {}) }.await,
            Err(ApplicationError::AccessExpired)
        ));
        assert_original_source(&store, dir.path(), before_bytes);
    });
}

#[tokio::test]
async fn concurrent_story_runs_refresh_once_and_keep_a_complete_usable_cache() {
    source_stores!(dir, store, {
        let completed = seed_store(&store);
        let server = MockServer::start().await;
        mount_source(&server, 1).await;
        mount_generation(&server, 2).await;
        let app = LocalApp::new(
            config(dir.path(), ProcessOverrides::default()),
            supplied_credentials(Some("wk".into()), Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        let now = completed + chrono::Duration::hours(2);
        let (first, second) = tokio::join!(unsafe { app.story(&store, now, 1, |_| {}) }, unsafe {
            app.story(&store, now, 2, |_| {})
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
        assert!(store.load().unwrap().sync_completed_at > completed);
    });
}

#[tokio::test]
async fn supplied_source_freshness_is_rechecked_under_the_lock_after_another_writer_refreshes() {
    source_stores!(dir, store, {
        let completed = seed_store(&store);
        let server = MockServer::start().await;
        mount_generation(&server, 1).await;
        let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
        let mut source_client =
            wanikani_source::Client::with_base_url("wk", &format!("{}/v2/", server.uri())).unwrap();
        let config = config(dir.path(), ProcessOverrides::default());
        let now = completed + chrono::Duration::hours(2);
        let report = run_story(
            &config,
            source_story_inputs(),
            Some(&store),
            Some(&mut source_client),
            &client,
            None,
            None,
            now,
            1,
            |event| {
                if matches!(event, ProgressEvent::Started { step: Step::Sync }) {
                    let writer = store.begin_sync().unwrap();
                    let mut data = source_fixture();
                    data.sync_completed_at = now;
                    writer.replace(data.into()).unwrap();
                }
            },
        )
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
    });
}

#[tokio::test]
async fn an_inactive_subscription_retains_its_recorded_free_content_access() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(dir.path());
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
        config(dir.path(), ProcessOverrides::default()),
        supplied_credentials(None, Some("ai".into())),
    )
    .with_endpoints(endpoints(&server));
    unsafe { app.story(&store, completed + chrono::Duration::minutes(10), 1, |_| {}) }
        .await
        .unwrap();
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
