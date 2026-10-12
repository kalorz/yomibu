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
        StoryInputs, StoryResources,
        progress::{ProgressEvent, Step},
        run_story,
    },
    configuration::{
        Configuration, ConfigurationInput, Invocation, Patch, ProcessOverrides,
        components::{GENERATION_KEY, GENERATION_MODEL, SOURCE_KEY},
        modules::{ModuleId, ModuleState},
    },
};
use yomibu_components::{
    file_learning_store::FileLearningStore,
    openai_story_generation::{Client, ProviderError},
    wanikani_source,
};
use yomibu_core::{
    capabilities::{LearningStore, SourceSyncWriter},
    domain::{
        inventory::LearnerInventory,
        source::WaniKaniSyncData,
        story::{StoryError, StoryRequest, StoryTopic},
    },
};

macro_rules! source_stores {
    ($dir:ident, $store:ident, $body:block) => {{
        let $dir = tempfile::tempdir().unwrap();
        let $store = FileLearningStore::new($dir.path());
        $body
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
    let error = { app.story(&store, SystemTime::now().into(), 7, |_| {}) }
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
        { app.story(&store, SystemTime::now().into(), 1, |_| {}) }
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

#[tokio::test]
async fn concurrent_supplied_story_runs_share_one_client_and_keep_per_call_inputs_and_progress() {
    let dir = tempfile::tempdir().unwrap();
    let mut flags = ProcessOverrides {
        ..Default::default()
    };
    flags.invocation.pipeline.model = Some("base-model".into());
    flags.invocation.story.seed = Patch::Set(5);
    flags.invocation.story.topic = Patch::Set("base topic".into());
    let base = Configuration::from_overrides(dir.path().into(), flags, &Operation::Story).unwrap();
    let server = MockServer::start().await;
    mount_generation(&server, 2).await;
    let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
    let cases = [
        ("first-model", "A cat exploring a garden", 7),
        ("second-model", "A walk through a snowy town", 42),
    ];
    let steps = [
        Step::Inputs,
        Step::Knowledge,
        Step::Selection,
        Step::Generation,
    ];
    let [first, second] = cases.map(|(model, topic, seed)| {
        let mut invocation = Invocation::default();
        invocation
            .pipeline
            .options
            .set(GENERATION_MODEL, model.into())
            .unwrap();
        invocation.story.seed = Patch::Set(seed);
        let config = base.for_invocation(invocation, &Operation::Story).unwrap();
        let mut request: StoryRequest =
            serde_json::from_str(include_str!("../../../tests/fixtures/story/request.json"))
                .unwrap();
        request.topic = Some(StoryTopic::new(topic.into()).unwrap());
        let expected_request = serde_json::to_value(&request).unwrap();
        let manual = LearnerInventory::from_manual(
            serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json"))
                .unwrap(),
        )
        .unwrap();
        let inputs = StoryInputs {
            request,
            manual: Some(manual),
        };
        let client = &client;
        async move {
            let mut events = Vec::new();
            let report = run_story::<FileLearningStore>(
                &config,
                inputs,
                StoryResources {
                    store: None,
                    source_client: None,
                    generation_client: client,
                },
                chrono::DateTime::from_timestamp(0, 0).unwrap(),
                99,
                |event| events.push(event),
            )
            .await
            .unwrap();
            assert_eq!(
                serde_json::to_value(&report.request).unwrap(),
                expected_request
            );
            assert_eq!(report.generated.provenance().requested_model, model);
            assert_eq!(report.selection.seed, seed);
            assert_completed_steps(&events);
            let started: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    ProgressEvent::Started { step } => Some(*step),
                    _ => None,
                })
                .collect();
            assert_eq!(started, steps);
        }
    });
    tokio::join!(first, second);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let bodies: Vec<serde_json::Value> = requests
        .iter()
        .map(|request| serde_json::from_slice(&request.body).unwrap())
        .collect();
    for (model, topic, _) in cases {
        let body = bodies.iter().find(|body| body["model"] == model).unwrap();
        let text = body.to_string();
        assert!(text.contains(topic), "{body}");
        let other_topic = cases.iter().find(|case| case.0 != model).unwrap().1;
        assert!(!text.contains(other_topic), "{body}");
    }
    assert_eq!(base.generation().unwrap().model, "base-model");
    assert_eq!(base.story.seed, Some(5));
    assert_eq!(base.story.topic.as_deref(), Some("base topic"));
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
            StoryResources {
                store: Some(&store),
                source_client: None,
                generation_client: &client,
            },
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
async fn supplied_source_refresh_reuses_client_and_selects_from_refreshed_inventory() {
    source_stores!(dir, store, {
        let server = MockServer::start().await;
        mount_source(&server, 2).await;
        mount_generation(&server, 2).await;
        let mut source_client =
            wanikani_source::Client::with_base_url("supplied-wk", &format!("{}/v2/", server.uri()))
                .unwrap();
        let client = Client::with_base_url("ai", &format!("{}/v1/", server.uri())).unwrap();
        let mut flags = ProcessOverrides::default();
        flags.application.cache_max_age_seconds = Some(0);
        let config = config(dir.path(), flags);
        let mut previous = source_fixture();
        for assignment in &mut previous.assignments {
            if assignment.subject_id == 3 {
                assignment.hidden = true;
            }
        }
        assert!(
            LearnerInventory::from_mapped(
                wanikani_source::map_content_and_progress(&previous).unwrap(),
                &config.pipeline.knowledge_policy,
            )
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
                source_story_inputs(),
                StoryResources {
                    store: Some(&store),
                    source_client: Some(&mut source_client),
                    generation_client: &client,
                },
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
            assert!(report.warnings.is_empty());
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
        let error = {
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
    let storage = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(storage.path());
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
            StoryResources {
                store: use_store.then_some(&store),
                source_client: use_client.then_some(&mut source_client),
                generation_client: &client,
            },
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
    use yomibu_components::file_learning_store::{
        FileSyncWriter,
        cache::{CacheError, WriteError},
    };

    #[derive(Debug)]
    enum InvalidInput {
        RequestVersion,
        SelectionLimit,
        ManualInventory,
        MissingKnowledge,
    }

    struct ObservedStore(FileLearningStore, Cell<usize>);
    impl LearningStore for ObservedStore {
        type ReadError = CacheError;
        type WriteError = WriteError;
        type Writer = FileSyncWriter;
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
        let store = ObservedStore(FileLearningStore::new(dir.path()), Cell::new(0));
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
            StoryResources {
                store: (!matches!(invalid, InvalidInput::MissingKnowledge)).then_some(&store),
                source_client: Some(&mut source_client),
                generation_client: &client,
            },
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
            StoryResources {
                store: Some(&store),
                source_client: Some(&mut source_client),
                generation_client: &client,
            },
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
async fn invocation_overrides_leave_the_base_app_unchanged_and_reuse_its_cache() {
    source_stores!(dir, store, {
        let server = MockServer::start().await;
        mount_source(&server, 1).await;
        mount_generation(&server, 2).await;
        let base = config(dir.path(), ProcessOverrides::default());
        let base_model = base.generation().unwrap().model;
        let app = LocalApp::new(
            base,
            supplied_credentials(Some("synthetic-wk".into()), Some("synthetic-ai".into())),
        )
        .with_endpoints(ServiceEndpoints {
            wanikani: format!("{}/v2/", server.uri()),
            openai: format!("{}/v1/", server.uri()),
        });
        let now = SystemTime::now().into();
        let mut invocation = Invocation::default();
        invocation.story.topic = Patch::Set("猫".into());
        invocation.pipeline.model = Some("invocation-model".into());
        let changed = app.for_invocation(invocation, &Operation::Story).unwrap();
        let mut events = Vec::new();
        let report = { changed.story(&store, now, 7, |event| events.push(event)) }
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
        assert_eq!(report.request.topic.as_ref().unwrap().text(), "猫");
        assert_eq!(
            report.generated.provenance().requested_model,
            "invocation-model"
        );
        assert!(!events.is_empty());
        assert_completed_steps(&events);
        let cached_report = { app.story(&store, SystemTime::now().into(), 7, |_| {}) }
            .await
            .unwrap();
        assert!(cached_report.request.topic.is_none());
        assert_eq!(
            cached_report.generated.provenance().requested_model,
            base_model
        );
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
        for (request, report) in requests
            .iter()
            .filter(|r| r.method.as_str() == "POST")
            .zip([&report, &cached_report])
        {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            let input: serde_json::Value =
                serde_json::from_str(body["input"][1]["content"].as_str().unwrap()).unwrap();
            let topic = report
                .request
                .topic
                .as_ref()
                .map(|topic| json!(topic.text()));
            assert_eq!(input.get("topic"), topic.as_ref());
            assert_eq!(body["model"], report.generated.provenance().requested_model);
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
        {
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
        { app.story(&store, completed + chrono::Duration::hours(1), 1, |_| {}) }
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
        { app.story(&store, completed - chrono::Duration::seconds(1), 1, |_| {}) }
            .await
            .unwrap();
        assert_eq!(server.received_requests().await.unwrap().len(), 5);
    });
}

#[tokio::test]
async fn a_recent_unusable_cache_is_refreshed_even_under_the_lock() {
    source_stores!(dir, store, {
        for (replaced_by_another_writer, invalid_inventory) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let completed = seed_store(&store);
            let now = completed + chrono::Duration::hours(2);
            let mut unusable = source_fixture();
            if invalid_inventory {
                unusable.subjects[2].characters = "x".repeat(257);
            } else {
                for assignment in &mut unusable.assignments {
                    assignment.started_at = None;
                    assignment.passed_at = None;
                    assignment.burned_at = None;
                    assignment.srs_stage = 0;
                }
            }
            unusable.sync_completed_at = now;
            unusable.validate().unwrap();
            let unusable = Arc::new(unusable);
            if !replaced_by_another_writer {
                store
                    .begin_sync()
                    .unwrap()
                    .replace(Arc::clone(&unusable))
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
            let report = {
                app.story(&store, now, 1, |event| {
                    if replaced_by_another_writer
                        && matches!(event, ProgressEvent::Started { step: Step::Sync })
                    {
                        store
                            .begin_sync()
                            .unwrap()
                            .replace(Arc::clone(&unusable))
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
            let report = { app.story(&store, completed + chrono::Duration::hours(2), 1, |_| {}) }
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
                let result = {
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
                    StoryResources {
                        store: Some(&store),
                        source_client: Some(&mut source_client),
                        generation_client: &client,
                    },
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
                { app.story(&store, now, 1, |_| {}) }.await
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
        let report = {
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
            { app.story(&store, completed + chrono::Duration::minutes(10), 1, |_| {}) }.await,
            Err(ApplicationError::Persistence(
                yomibu_components::file_learning_store::cache::WriteError::Locked
            ))
        ));
        drop(guard);
        let app = LocalApp::new(
            config(dir.path(), ProcessOverrides::default()),
            supplied_credentials(None, Some("ai".into())),
        )
        .with_endpoints(endpoints(&server));
        assert!(
            { app.story(&store, completed + chrono::Duration::minutes(10), 1, |_| {}) }
                .await
                .is_err()
        );
    });
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
    { app.story(&store, completed + chrono::Duration::hours(2), 1, |_| {}) }
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
        { app.story(&store, SystemTime::now().into(), 1, |_| {}) }.await,
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
            { app.story(&store, SystemTime::now().into(), 1, |_| {}) }.await,
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
            { app.story(&store, now, 1, |_| {}) }.await,
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
        let (first, second) = tokio::join!({ app.story(&store, now, 1, |_| {}) }, {
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
            StoryResources {
                store: Some(&store),
                source_client: Some(&mut source_client),
                generation_client: &client,
            },
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
    { app.story(&store, completed + chrono::Duration::minutes(10), 1, |_| {}) }
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
