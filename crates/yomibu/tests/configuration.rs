use std::{collections::BTreeMap, path::PathBuf};
use yomibu::{
    application::Operation,
    configuration::{Configuration, ConfigurationInput, Settings},
};

#[test]
fn scoped_defaults_resolve_component_options_and_file_relative_resources() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "[application]\ninventory = 'inventory.json'\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), "[pipeline]\nmodel = 'shared'\n[pipeline.options.openai-story-generation]\nmodel = 'generation'\n").unwrap();
    std::fs::write(
        dir.path().join("default-story.toml"),
        "[story]\nselect = 8\nseed = 7\nformat = 'sentence'\n",
    )
    .unwrap();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::from([(
                "YOMIBU_OPENAI_STORY_GENERATION_MODEL".into(),
                "environment-generation".into(),
            )]),
            flags: Settings {
                select: Some(4),
                ..Default::default()
            },
        },
        &Operation::Story,
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
        Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                config: None,
                home: None,
                environment: BTreeMap::new(),
                flags: Settings::default(),
            },
            &Operation::Analyze("input.json".into())
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
        let config = Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                config: None,
                home: None,
                environment: BTreeMap::new(),
                flags: Settings::default(),
            },
            &operation,
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
            Operation::Story => unreachable!(),
        }
    }
}

#[test]
fn resolves_each_model_setting_before_job_fallback_and_paths_relative_to_config() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.toml");
    std::fs::write(&file, "[application]\ninventory='inventory.json'\n").unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), "[pipeline]\nmodel='file-default'\n[pipeline.options.openai-story-generation]\nmodel='file-generation'\n").unwrap();
    let env = BTreeMap::from([("YOMIBU_MODEL".into(), "environment-default".into())]);
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: env,
            flags: Settings {
                model: Some("flag-default".into()),
                ..Default::default()
            },
        },
        &yomibu::application::Operation::Story,
    )
    .unwrap();
    assert_eq!(config.generation().model, "file-generation");
    assert_eq!(
        config.application.inventory,
        Some(dir.path().join("inventory.json"))
    );
    assert_eq!(config.application.cache_max_age.as_secs(), 3600);
}

#[test]
fn malformed_config_does_not_reflect_its_contents_or_credentials() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "model = 'synthetic-secret\n",
    )
    .unwrap();
    let error = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: Settings::default(),
        },
        &yomibu::application::Operation::Story,
    )
    .unwrap_err();
    assert!(!format!("{error:?} {error}").contains("synthetic-secret"));
    assert!(error.to_string().contains("config.toml"));
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
            flags: Settings::default(),
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
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: Settings {
                enable: vec![ModuleId::Sync],
                disable: vec![ModuleId::Embeddings],
                ..Default::default()
            },
        },
        &yomibu::application::Operation::Story,
    )
    .unwrap();
    assert!(config.enabled(ModuleId::Sync));
    assert!(!config.enabled(ModuleId::Embeddings));
    assert!(config.enabled(ModuleId::Assessment));
    assert!(
        Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                config: None,
                home: None,
                environment: BTreeMap::new(),
                flags: Settings {
                    enable: vec![ModuleId::Assessment],
                    disable: vec![ModuleId::Assessment],
                    ..Default::default()
                }
            },
            &yomibu::application::Operation::Story
        )
        .is_err()
    );
}

#[test]
fn flags_override_environment_and_environment_overrides_file_for_each_setting() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"),"[pipeline]\nmodel='file-model'\n[pipeline.options.openai-story-generation]\nmodel='file-generation'\n").unwrap();
    std::fs::write(dir.path().join("default-story.toml"), "[story]\nselect=2\n").unwrap();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::from([
                (
                    "YOMIBU_OPENAI_STORY_GENERATION_MODEL".into(),
                    "env-generation".into(),
                ),
                ("YOMIBU_SELECT".into(), "3".into()),
            ]),
            flags: Settings {
                generation_model: Some("flag-generation".into()),
                ..Default::default()
            },
        },
        &yomibu::application::Operation::Story,
    )
    .unwrap();
    assert_eq!(config.generation().model, "flag-generation");
    assert_eq!(config.story.select, 3);
}

#[test]
fn component_choices_and_credential_bindings_belong_to_separate_scopes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), "[application.credentials]\n\"openai-story-generation.api-key\"={provider='supplied',key='openai'}\n\"http-embeddings.api-key\"={clear=true}\n").unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), "[pipeline.components]\nsource='wanikani-source'\nlearning_store='file-learning-store'\nembedding_cache='file-embedding-cache'\npreparation='story-prompt-preparation'\ngeneration='openai-story-generation'\nanalysis='sudachi-dictionary'\nassessment='japanese-constraint-checks'\n").unwrap();
    let load = || {
        Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                config: None,
                home: None,
                environment: BTreeMap::new(),
                flags: Settings::default(),
            },
            &Operation::Story,
        )
    };
    assert!(load().is_ok());
    for invalid in [
        "[application.components]\nsource='wanikani-source'",
        "[application.credentials]\n\"openai-story-generation.api-key\"={provider='supplied',key='wanikani'}",
        "[application.credentials]\n\"openai-story-generation.api-key\"={provider='keychain',key='openai'}",
        "[application.credentials]\n\"openai-story-generation.api-key\"={provider='supplied',key='openai',value='synthetic-secret'}",
    ] {
        std::fs::write(dir.path().join("config.toml"), invalid).unwrap();
        let error = load().unwrap_err();
        assert!(!format!("{error:?}").contains("synthetic-secret"));
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
                flags: Settings::default(),
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
fn shipped_references_equal_omission_without_making_resource_paths_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let load = || {
        Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                config: None,
                home: None,
                environment: BTreeMap::new(),
                flags: Settings::default(),
            },
            &Operation::Story,
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
        "[pipeline.options.openai-story-generation]\napi-key='synthetic-secret'",
        "[pipeline]\nresources={inventory='input.json'}",
        "[pipeline.options.openai-story-generation]\nmodel={clear=false}",
    ] {
        std::fs::write(dir.path().join("default-pipeline.toml"), text).unwrap();
        let error = Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                config: None,
                home: None,
                environment: BTreeMap::new(),
                flags: Settings::default(),
            },
            &Operation::Story,
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
        r#"{"pipeline":{"options":{"openai-story-generation":{"api-key":"secret"}}}}"#,
        r#"{"story":{"inventory":"input.json"}}"#,
    ] {
        assert!(serde_json::from_str::<Invocation>(input).is_err());
    }
    let dir = tempfile::tempdir().unwrap();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: Settings {
                model: Some("shared".into()),
                generation_model: Some("specific".into()),
                ..Default::default()
            },
        },
        &Operation::Story,
    )
    .unwrap();
    let input:Invocation=serde_json::from_str(r#"{"pipeline":{"model":"new-shared","options":{"openai-story-generation":{"model":{"clear":true}}}}}"#).unwrap();
    let changed = config.for_invocation(input, &Operation::Story).unwrap();
    assert_eq!(changed.generation().model, "new-shared");
    assert_eq!(config.generation().model, "specific");
    assert!(std::sync::Arc::ptr_eq(
        &config.application,
        &changed.application
    ));
}
