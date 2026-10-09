use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use yomibu::{
    application::Operation,
    configuration::{ConfigError, Configuration, ConfigurationInput, Patch, ProcessOverrides},
};

fn load_config(
    dir: &Path,
    operation: &Operation,
    flags: ProcessOverrides,
    environment: BTreeMap<String, String>,
) -> Result<Configuration, ConfigError> {
    Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.into()),
            config: None,
            home: None,
            environment,
            flags,
        },
        operation,
    )
}

#[test]
fn consumed_lower_precedence_values_must_have_valid_types() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("default-story.toml");
    let load = |environment| {
        let mut flags = ProcessOverrides::default();
        flags.invocation.story.select = Some(4);
        load_config(dir.path(), &Operation::Story, flags, environment)
    };
    std::fs::write(&path, "[story]\nselect='wrong-type'\n").unwrap();
    assert!(
        matches!(load(BTreeMap::new()), Err(ConfigError::Invalid { path: invalid }) if invalid == path)
    );
    std::fs::write(&path, "[story]\nselect=2\n").unwrap();
    for name in [
        "YOMIBU_SELECT",
        "YOMIBU_SEED",
        "YOMIBU_FORMAT",
        "YOMIBU_CANDIDATES",
        "YOMIBU_KNOWLEDGE_POLICY",
        "YOMIBU_ALLOW_EMBEDDING_CALL",
        "YOMIBU_CACHE_MAX_AGE_SECONDS",
        "YOMIBU_EMBEDDING_PROVIDER",
        "YOMIBU_HTTP_EMBEDDINGS_DIMENSIONS",
        "YOMIBU_ENABLE",
        "YOMIBU_DISABLE",
    ] {
        let environment = BTreeMap::from([(name.into(), "wrong-type".into())]);
        assert!(
            matches!(load(environment.clone()), Err(ConfigError::Environment { name: invalid }) if invalid == name)
        );
        assert!(
            load_config(
                dir.path(),
                &Operation::Status,
                ProcessOverrides::default(),
                environment
            )
            .is_ok()
        );
    }
    std::fs::write(&path, "[story]\nselect=0\n").unwrap();
    assert_eq!(load(BTreeMap::new()).unwrap().story.select, 4);
}

#[test]
fn unknown_fields_in_loaded_unused_pipeline_scopes_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("default-pipeline.toml");
    for text in [
        "[pipeline.options.http-embeddings]\nunknown='synthetic-secret'",
        "[pipeline.components]\nunknown='synthetic-secret'",
        "[pipeline.selection]\nunknown='synthetic-secret'",
        "\"application.credentials\"={\"openai.api-key\"={clear=true}}",
        "\"pipeline.options\"={}",
    ] {
        std::fs::write(&path, text).unwrap();
        let error = load_config(
            dir.path(),
            &Operation::Status,
            ProcessOverrides::default(),
            BTreeMap::new(),
        )
        .unwrap_err();
        assert!(matches!(&error, ConfigError::Invalid { path: invalid } if invalid == &path));
        assert!(!format!("{error:?} {error}").contains("synthetic-secret"));
    }
}

#[test]
fn configuration_errors_preserve_diagnostic_order_without_reflecting_contents() {
    let dir = tempfile::tempdir().unwrap();
    let pipeline = dir.path().join("default-pipeline.toml");
    let config = dir.path().join("config.toml");
    let load = || {
        load_config(
            dir.path(),
            &Operation::Story,
            ProcessOverrides::default(),
            BTreeMap::from([
                ("YOMIBU_SELECT".into(), "wrong-type".into()),
                ("YOMIBU_CANDIDATES".into(), "wrong-type".into()),
            ]),
        )
    };
    std::fs::write(&config, "model = 'synthetic-secret\n").unwrap();
    let error = load().unwrap_err();
    assert!(!format!("{error:?} {error}").contains("synthetic-secret"));
    assert!(error.to_string().contains("config.toml"));
    std::fs::write(&pipeline, "[pipeline]\nmodel=7\n").unwrap();
    assert!(matches!(load(), Err(ConfigError::Invalid { path }) if path == config));
    std::fs::write(&config, "").unwrap();
    assert!(matches!(load(), Err(ConfigError::Invalid { path }) if path == pipeline));
    std::fs::remove_file(&pipeline).unwrap();
    let story = dir.path().join("default-story.toml");
    std::fs::write(&story, "[story]\ncandidates='wrong-type'\n").unwrap();
    assert!(matches!(load(), Err(ConfigError::Invalid { path }) if path == story));
    std::fs::remove_file(&story).unwrap();
    assert!(
        matches!(load(), Err(ConfigError::Environment { name }) if name == "YOMIBU_CANDIDATES")
    );
}

#[test]
fn scoped_defaults_resolve_component_options_and_file_relative_resources() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "[application]\ninventory = 'inventory.json'\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("default-pipeline.toml"),
        "[pipeline]\nmodel = 'shared'\n[pipeline.options.openai]\nmodel = 'generation'\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("default-story.toml"),
        "[story]\nselect = 8\nseed = 7\nformat = 'sentence'\n",
    )
    .unwrap();
    let mut flags = ProcessOverrides::default();
    flags.invocation.story.select = Some(4);
    let config = load_config(
        dir.path(),
        &Operation::Story,
        flags,
        BTreeMap::from([(
            "YOMIBU_OPENAI_MODEL".into(),
            "environment-generation".into(),
        )]),
    )
    .unwrap();
    assert_eq!(
        config.application.inventory,
        Some(dir.path().join("inventory.json"))
    );
    assert_eq!(config.generation().model, "environment-generation");
    assert_eq!(config.story.select, 4);
    assert_eq!(config.story.seed, Some(7));
    assert_eq!(
        config.generation().format,
        yomibu::configuration::StoryFormat::Sentence
    );
}

#[test]
fn configuration_rejects_the_removed_arbitrary_dictionary_setting() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "dictionary = 'external.dic'\n",
    )
    .unwrap();
    assert!(matches!(
        load_config(
            dir.path(),
            &Operation::Analyze("input.json".into()),
            ProcessOverrides::default(),
            BTreeMap::new()
        ),
        Err(yomibu::configuration::ConfigError::Invalid { .. })
    ));
}

#[test]
fn each_operation_validates_its_resources_without_parsing_unused_settings() {
    let dir = tempfile::tempdir().unwrap();
    for operation in [
        Operation::Status,
        Operation::Sync,
        Operation::Analyze("input.json".into()),
        Operation::Import("bundle".into()),
        Operation::Verify,
        Operation::Retrieval,
        Operation::Preview,
    ] {
        std::fs::write(
            dir.path().join("config.toml"),
            if matches!(operation, Operation::Preview) {
                "[application]\ndictionary_dir=false\n"
            } else {
                "[application]\nwanikani_cache='source.json'\ndictionary_dir='managed'\n"
            },
        )
        .unwrap();
        std::fs::write(
            dir.path().join("default-pipeline.toml"),
            if matches!(operation, Operation::Preview) {
                "[pipeline]\nmodel='preview'\n"
            } else {
                "[pipeline]\nmodel=7\n"
            },
        )
        .unwrap();
        std::fs::write(
            dir.path().join("default-story.toml"),
            if matches!(operation, Operation::Preview) {
                "[story]\nselect=2\n"
            } else {
                "[story]\nformat='unused-invalid'\nselect=2\ncandidates=0\n"
            },
        )
        .unwrap();
        let config = load_config(
            dir.path(),
            &operation,
            ProcessOverrides::default(),
            BTreeMap::new(),
        )
        .unwrap_or_else(|error| panic!("{operation:?}: {error}"));
        match operation {
            Operation::Status | Operation::Sync => assert_eq!(
                config.application.wanikani_cache,
                Some(dir.path().join("source.json"))
            ),
            Operation::Analyze(_) | Operation::Import(_) | Operation::Verify => assert_eq!(
                config.application.dictionary_dir,
                dir.path().join("managed")
            ),
            Operation::Preview => assert_eq!(
                config.application.dictionary_dir,
                dir.path().join("dictionaries")
            ),
            Operation::Retrieval => assert_eq!(config.story.select, 2),
            Operation::Story | Operation::Auth => unreachable!(),
        }
    }
}

#[test]
fn resolves_each_model_setting_before_job_fallback_and_paths_relative_to_config() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.toml");
    std::fs::write(&file, "[application]\ninventory='inventory.json'\n").unwrap();
    std::fs::write(
        dir.path().join("default-pipeline.toml"),
        "[pipeline]\nmodel='file-default'\n[pipeline.options.openai]\nmodel='file-generation'\n",
    )
    .unwrap();
    let env = BTreeMap::from([("YOMIBU_MODEL".into(), "environment-default".into())]);
    let mut flags = ProcessOverrides::default();
    flags.invocation.pipeline.model = Some("flag-default".into());
    let config = load_config(dir.path(), &Operation::Story, flags, env).unwrap();
    assert_eq!(config.generation().model, "file-generation");
    assert_eq!(
        config.application.inventory,
        Some(dir.path().join("inventory.json"))
    );
    assert_eq!(config.application.cache_max_age.as_secs(), 3600);
}

#[test]
fn construction_with_defaults_neither_creates_files_nor_reads_environment() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("new");
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: None,
            config: None,
            home: Some(data_dir.clone()),
            environment: BTreeMap::new(),
            flags: ProcessOverrides::default(),
        },
        &yomibu::application::Operation::Story,
    )
    .unwrap();
    assert_eq!(
        config.application.data_dir,
        PathBuf::from(&data_dir).join(".yomibu")
    );
    assert!(!data_dir.exists());
}

#[test]
fn module_controls_override_saved_choices_but_conflicts_remain_errors() {
    use yomibu::configuration::modules::ModuleId;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "[application]\nsync=false\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("default-pipeline.toml"),
        "[pipeline.selection]\nembeddings=true\n",
    )
    .unwrap();
    let config = load_config(
        dir.path(),
        &yomibu::application::Operation::Story,
        ProcessOverrides {
            enable: vec![ModuleId::Sync],
            disable: vec![ModuleId::Embeddings],
            ..Default::default()
        },
        BTreeMap::new(),
    )
    .unwrap();
    assert!(config.enabled(ModuleId::Sync));
    assert!(!config.enabled(ModuleId::Embeddings));
    assert!(config.enabled(ModuleId::Assessment));
    assert!(
        load_config(
            dir.path(),
            &yomibu::application::Operation::Story,
            ProcessOverrides {
                enable: vec![ModuleId::Assessment],
                disable: vec![ModuleId::Assessment],
                ..Default::default()
            },
            BTreeMap::new()
        )
        .is_err()
    );
}

#[test]
fn flags_override_environment_and_environment_overrides_file_for_each_setting() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("default-pipeline.toml"),
        "[pipeline]\nmodel='file-model'\n[pipeline.options.openai]\nmodel='file-generation'\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("default-story.toml"), "[story]\nselect=2\n").unwrap();
    let mut flags = ProcessOverrides::default();
    flags.invocation.pipeline.options.generation.model = Patch::Set("flag-generation".into());
    let config = load_config(
        dir.path(),
        &Operation::Story,
        flags,
        BTreeMap::from([
            ("YOMIBU_OPENAI_MODEL".into(), "env-generation".into()),
            ("YOMIBU_SELECT".into(), "3".into()),
        ]),
    )
    .unwrap();
    assert_eq!(config.generation().model, "flag-generation");
    assert_eq!(config.story.select, 3);
}

#[test]
fn component_choices_belong_to_pipeline_scope() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), "[pipeline.components]\nsource='wanikani'\nlearning_store='file-learning-store'\nembedding_cache='file-embedding-cache'\npreparation='story-prompt-preparation'\ngeneration='openai'\nanalysis='sudachi-dictionary'\nassessment='japanese-constraint-checks'\n").unwrap();
    let load = || {
        load_config(
            dir.path(),
            &Operation::Story,
            ProcessOverrides::default(),
            BTreeMap::new(),
        )
    };
    assert!(load().is_ok());
    std::fs::write(
        dir.path().join("config.toml"),
        "[application.components]\nsource='wanikani'",
    )
    .unwrap();
    assert!(load().is_err());
}

#[test]
fn credential_bindings_are_rejected_for_every_operation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    for value in ["{provider='supplied',key='openai'}", "'synthetic-secret'"] {
        std::fs::write(
            &path,
            format!("[application.credentials]\n\"openai.api-key\"={value}\n"),
        )
        .unwrap();
        for operation in [
            Operation::Auth,
            Operation::Story,
            Operation::Preview,
            Operation::Retrieval,
            Operation::Sync,
            Operation::Status,
            Operation::Verify,
            Operation::Analyze("input.json".into()),
            Operation::Import("bundle".into()),
        ] {
            let error = load_config(
                dir.path(),
                &operation,
                ProcessOverrides::default(),
                BTreeMap::new(),
            )
            .unwrap_err();
            assert!(matches!(&error, ConfigError::Invalid { path: invalid } if invalid == &path));
            assert!(!format!("{error:?} {error}").contains("synthetic-secret"));
        }
    }
}

#[test]
fn status_does_not_open_story_defaults_but_story_requires_bounded_valid_documents() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("default-story.toml")).unwrap();
    let load = |operation, config| {
        Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                config,
                home: None,
                environment: BTreeMap::new(),
                flags: ProcessOverrides::default(),
            },
            &operation,
        )
    };
    assert!(load(Operation::Status, None).is_ok());
    assert!(load(Operation::Story, None).is_err());
    std::fs::remove_dir(dir.path().join("default-story.toml")).unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), " ".repeat(65537)).unwrap();
    assert!(load(Operation::Status, None).is_err());
    std::fs::remove_file(dir.path().join("default-pipeline.toml")).unwrap();
    assert!(load(Operation::Story, None).is_ok());
    assert!(load(Operation::Story, Some(dir.path().join("missing.toml"))).is_err());
}

#[test]
fn shipped_references_and_resource_clears_preserve_implicit_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let load = || {
        load_config(
            dir.path(),
            &Operation::Story,
            ProcessOverrides::default(),
            BTreeMap::new(),
        )
        .unwrap()
    };
    let omitted = format!("{:?}", load());
    for (name, text) in [
        ("config.toml", include_str!("../../../config/config.toml")),
        (
            "default-pipeline.toml",
            include_str!("../../../config/default-pipeline.toml"),
        ),
        (
            "default-story.toml",
            include_str!("../../../config/default-story.toml"),
        ),
    ] {
        std::fs::write(dir.path().join(name), text).unwrap();
    }
    assert_eq!(format!("{:?}", load()), omitted);
    std::fs::write(dir.path().join("config.toml"), "[application]\ninventory={clear=true}\nwanikani_cache={clear=true}\ndictionary_dir={clear=true}\nembedding_cache={clear=true}\n").unwrap();
    assert_eq!(format!("{:?}", load()), omitted);
}

#[test]
fn invalid_compositions_unknown_paths_and_secrets_are_static_errors_even_when_disabled() {
    let dir = tempfile::tempdir().unwrap();
    for text in [
        "[pipeline.selection]\nsteps=[]",
        "[pipeline.selection]\nsteps=['seeded-order','seeded-order']",
        "[pipeline.selection]\nsteps=['embedding-rank']",
        "[pipeline.selection]\nembeddings=false\nembedding_steps=[]",
        "[pipeline.components]\ngeneration='unknown'",
        "[pipeline.options.openai]\napi-key='synthetic-secret'",
        "[pipeline]\nresources={inventory='input.json'}",
        "[pipeline.options.openai]\nmodel={clear=false}",
    ] {
        std::fs::write(dir.path().join("default-pipeline.toml"), text).unwrap();
        let error = load_config(
            dir.path(),
            &Operation::Story,
            ProcessOverrides::default(),
            BTreeMap::new(),
        )
        .unwrap_err();
        assert!(!format!("{error:?} {error}").contains("synthetic-secret"));
    }
}

#[test]
fn typed_pipeline_cannot_bind_credentials_or_resources_and_clear_restores_model_fallback() {
    use yomibu::configuration::Invocation;
    for input in [
        r#"{"pipeline":{"credentials":{}}}"#,
        r#"{"pipeline":{"options":{"openai":{"api-key":"secret"}}}}"#,
        r#"{"story":{"inventory":"input.json"}}"#,
    ] {
        assert!(serde_json::from_str::<Invocation>(input).is_err());
    }
    let dir = tempfile::tempdir().unwrap();
    let mut flags = ProcessOverrides::default();
    flags.invocation.pipeline.model = Some("shared".into());
    flags.invocation.pipeline.options.generation.model = Patch::Set("specific".into());
    let config = load_config(dir.path(), &Operation::Story, flags, BTreeMap::new()).unwrap();
    let input: Invocation = serde_json::from_str(
        r#"{"pipeline":{"model":"new-shared","options":{"openai":{"model":{"clear":true}}}}}"#,
    )
    .unwrap();
    let changed = config.for_invocation(input, &Operation::Story).unwrap();
    assert_eq!(changed.generation().model, "new-shared");
    assert_eq!(config.generation().model, "specific");
    assert!(std::sync::Arc::ptr_eq(
        &config.application,
        &changed.application
    ));
}

#[test]
fn typed_topic_overrides_conflict_with_request_files_after_option_validation() {
    use yomibu::configuration::Invocation;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("default-story.toml"),
        "[story]\ntopic='saved'\n",
    )
    .unwrap();
    for operation in [Operation::Story, Operation::Preview, Operation::Retrieval] {
        let config = load_config(
            dir.path(),
            &operation,
            ProcessOverrides {
                request: Some(dir.path().join("unopened.json")),
                ..Default::default()
            },
            BTreeMap::new(),
        )
        .unwrap();
        let inherited = config
            .for_invocation(Invocation::default(), &operation)
            .unwrap();
        assert_eq!(inherited.story.topic.as_deref(), Some("saved"));
        for topic in [Patch::Set("overridden".into()), Patch::Clear] {
            let mut invocation = Invocation::default();
            invocation.story.topic = topic;
            assert!(
                matches!(
                    config.for_invocation(invocation, &operation),
                    Err(ConfigError::InvalidSetting(
                        "--topic and --request conflict; put the topic in the request file."
                    ))
                ),
                "{operation:?}"
            );
        }
        let mut invalid = Invocation::default();
        invalid.story.topic = Patch::Clear;
        invalid.story.select = Some(0);
        assert!(matches!(
            config.for_invocation(invalid, &operation),
            Err(ConfigError::InvalidSetting(
                "--select must be between 1 and 16."
            ))
        ));
        assert_eq!(config.story.topic.as_deref(), Some("saved"));
    }
    assert!(!dir.path().join("unopened.json").exists());
}
