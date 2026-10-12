use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use yomibu::{
    application::Operation,
    configuration::{
        ConfigError, Configuration, ConfigurationInput, Invocation, Patch,
        components::GENERATION_MODEL,
    },
};

fn load_config(
    dir: &Path,
    operation: &Operation,
    flags: Invocation,
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
fn typed_construction_ignores_malformed_local_defaults_and_preserves_files() {
    let dir = tempfile::tempdir().unwrap();
    let names = ["config.toml", "default-pipeline.toml", "default-story.toml"];
    for name in names {
        std::fs::write(dir.path().join(name), "[").unwrap();
    }
    let mut overrides = Invocation::default();
    let options = &mut overrides.pipeline.options;
    options.set(GENERATION_MODEL, "generation".into()).unwrap();
    overrides.story.topic = Patch::Set("typed topic".into());
    overrides.story.select = Some(4);
    let config =
        Configuration::from_overrides(dir.path().into(), overrides, &Operation::Story).unwrap();
    assert_eq!(config.generation().unwrap().model, "generation");
    assert_eq!(config.story.topic.as_deref(), Some("typed topic"));
    assert_eq!(config.story.select, 4);
    for name in names {
        assert_eq!(std::fs::read(dir.path().join(name)).unwrap(), b"[");
    }
}

#[test]
fn typed_construction_keeps_defaults_without_creating_the_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("nonexistent");
    let config =
        Configuration::from_overrides(data_dir.clone(), Invocation::default(), &Operation::Story)
            .unwrap();
    assert_eq!(config.data_dir, data_dir);
    assert_eq!(config.story.select, 12);
    assert!(!data_dir.exists());
}

#[test]
fn typed_construction_keeps_operation_filtering_and_validation_order() {
    let dir = tempfile::tempdir().unwrap();
    for model in ["", "typed"] {
        let overrides = || {
            let mut input = Invocation::default();
            input
                .pipeline
                .options
                .set(GENERATION_MODEL, model.into())
                .unwrap();
            input.story.select = Some(0);
            input
        };
        let error =
            Configuration::from_overrides(dir.path().into(), overrides(), &Operation::Story)
                .unwrap_err();
        let expected =
            load_config(dir.path(), &Operation::Story, overrides(), BTreeMap::new()).unwrap_err();
        assert_eq!(format!("{error:?}"), format!("{expected:?}"));
        let ignored =
            Configuration::from_overrides(dir.path().into(), overrides(), &Operation::Sync)
                .unwrap();
        assert_eq!(ignored.story.select, 12);
    }
}

#[test]
fn consumed_lower_precedence_values_must_have_valid_types() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("default-story.toml");
    let load = |environment| {
        let mut flags = Invocation::default();
        flags.story.select = Some(4);
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
    ] {
        let environment = BTreeMap::from([(name.into(), "wrong-type".into())]);
        assert!(
            matches!(load(environment.clone()), Err(ConfigError::Environment { name: invalid } | ConfigError::ComponentEnvironment { name: invalid, .. }) if invalid == name)
        );
        assert!(
            load_config(
                dir.path(),
                &Operation::Sync,
                Invocation::default(),
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
            &Operation::Sync,
            Invocation::default(),
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
            Invocation::default(),
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
fn scoped_defaults_resolve_component_options_and_story_settings() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), "").unwrap();
    std::fs::write(
        dir.path().join("default-pipeline.toml"),
        "[pipeline.options.openai]\nmodel = 'generation'\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("default-story.toml"),
        "[story]\nselect = 8\nseed = 7\nformat = 'sentence'\n",
    )
    .unwrap();
    let mut flags = Invocation::default();
    flags.story.select = Some(4);
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
    assert_eq!(config.generation().unwrap().model, "environment-generation");
    assert_eq!(config.story.select, 4);
    assert_eq!(config.story.seed, Some(7));
    assert_eq!(
        config.generation().unwrap().format,
        yomibu::configuration::StoryFormat::Sentence
    );
}

#[test]
fn sync_validates_its_resources_without_parsing_unused_story_settings() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), "").unwrap();
    std::fs::write(
        dir.path().join("default-pipeline.toml"),
        "[pipeline.options.openai]\nmodel=7\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("default-story.toml"),
        "[story]\nformat='unused-invalid'\nselect=2\ncandidates=0\n",
    )
    .unwrap();
    let _config = load_config(
        dir.path(),
        &Operation::Sync,
        Invocation::default(),
        BTreeMap::new(),
    )
    .unwrap();
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
            flags: Invocation::default(),
        },
        &yomibu::application::Operation::Story,
    )
    .unwrap();
    assert_eq!(config.data_dir, PathBuf::from(&data_dir).join(".yomibu"));
    assert!(!data_dir.exists());
}

#[test]
fn flags_override_environment_and_environment_overrides_file_for_each_setting() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("default-pipeline.toml"),
        "[pipeline.options.openai]\nmodel='file-generation'\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("default-story.toml"), "[story]\nselect=2\n").unwrap();
    let mut flags = Invocation::default();
    flags
        .pipeline
        .options
        .set(GENERATION_MODEL, "flag-generation".into())
        .unwrap();
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
    assert_eq!(config.generation().unwrap().model, "flag-generation");
    assert_eq!(config.story.select, 3);
}

#[test]
fn component_choices_belong_to_pipeline_scope() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), "[pipeline.components]\nsource='wanikani'\nlearning_store='file-learning-store'\npreparation='story-prompt-preparation'\ngeneration='openai'\n").unwrap();
    let load = || {
        load_config(
            dir.path(),
            &Operation::Story,
            Invocation::default(),
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
        for operation in [Operation::Story, Operation::Sync] {
            let error = load_config(
                dir.path(),
                &operation,
                Invocation::default(),
                BTreeMap::new(),
            )
            .unwrap_err();
            assert!(matches!(&error, ConfigError::Invalid { path: invalid } if invalid == &path));
            assert!(!format!("{error:?} {error}").contains("synthetic-secret"));
        }
    }
}

#[test]
fn sync_does_not_open_story_defaults_but_story_requires_bounded_valid_documents() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("default-story.toml")).unwrap();
    let load = |operation, config| {
        Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                config,
                home: None,
                environment: BTreeMap::new(),
                flags: Invocation::default(),
            },
            &operation,
        )
    };
    assert!(load(Operation::Sync, None).is_ok());
    assert!(load(Operation::Story, None).is_err());
    std::fs::remove_dir(dir.path().join("default-story.toml")).unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), " ".repeat(65537)).unwrap();
    assert!(load(Operation::Sync, None).is_err());
    std::fs::remove_file(dir.path().join("default-pipeline.toml")).unwrap();
    assert!(load(Operation::Story, None).is_ok());
    assert!(load(Operation::Story, Some(dir.path().join("missing.toml"))).is_err());
}

#[test]
fn shipped_references_preserve_implicit_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let load = || {
        load_config(
            dir.path(),
            &Operation::Story,
            Invocation::default(),
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
            Invocation::default(),
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
        r#"{"pipeline":{"options":{"unknown":{"model":"test"}}}}"#,
        r#"{"pipeline":{"options":{"openai":{"unknown":"test"}}}}"#,
        r#"{"story":{"inventory":"input.json"}}"#,
    ] {
        assert!(serde_json::from_str::<Invocation>(input).is_err());
    }
    let dir = tempfile::tempdir().unwrap();
    let mut flags = Invocation::default();
    flags
        .pipeline
        .options
        .set(GENERATION_MODEL, "specific".into())
        .unwrap();
    let config = load_config(dir.path(), &Operation::Story, flags, BTreeMap::new()).unwrap();
    let input: Invocation =
        serde_json::from_str(r#"{"pipeline":{"options":{"openai":{"model":{"clear":true}}}}}"#)
            .unwrap();
    let changed = config.for_invocation(input, &Operation::Story).unwrap();
    assert_eq!(changed.generation().unwrap().model, "gpt-6-luna");
    assert_eq!(config.generation().unwrap().model, "specific");
    assert_eq!(config.data_dir, changed.data_dir);
}

#[test]
fn candidate_count_still_resolves_flags_then_environment_then_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("default-story.toml"),
        "[story]\ncandidates=2\n",
    )
    .unwrap();
    for (flag, env, expected) in [
        (None, None, 2),
        (None, Some("3"), 3),
        (Some(4), Some("3"), 4),
    ] {
        let mut flags = Invocation::default();
        flags.story.candidates = flag;
        let environment = env
            .map(|value| ("YOMIBU_CANDIDATES".into(), value.into()))
            .into_iter()
            .collect();
        let config = load_config(dir.path(), &Operation::Story, flags, environment).unwrap();
        assert_eq!(config.generation().unwrap().candidate_count, expected);
    }
}

#[test]
fn removed_settings_are_rejected_in_files_and_ignored_in_environment() {
    let dir = tempfile::tempdir().unwrap();
    let load = |environment| {
        Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                config: None,
                home: None,
                environment,
                flags: Invocation::default(),
            },
            &Operation::Story,
        )
    };
    for (file, text) in [
        ("config.toml", "[application]\nsync=false"),
        ("config.toml", "[application]\ninventory='unused'"),
        ("config.toml", "[application]\nwanikani_cache='unused'"),
        ("config.toml", "[application]\ncache_max_age_seconds=60"),
        ("default-pipeline.toml", "[pipeline]\nmodel='unused'"),
        (
            "default-pipeline.toml",
            "[pipeline]\nknowledge_policy='recorded-pass'",
        ),
        ("default-story.toml", "[story]\nrequest='unused'"),
        ("default-story.toml", "[story.targets]\ngrammar=['polite']"),
    ] {
        let path = dir.path().join(file);
        std::fs::write(&path, text).unwrap();
        assert!(load(BTreeMap::new()).is_err(), "accepted {text}");
        std::fs::remove_file(path).unwrap();
    }
    let config = load(
        [
            "SYNC",
            "INVENTORY",
            "WANIKANI_CACHE",
            "CACHE_MAX_AGE_SECONDS",
            "MODEL",
            "KNOWLEDGE_POLICY",
            "REQUEST",
        ]
        .map(|name| (format!("YOMIBU_{name}"), "invalid-unused".into()))
        .into(),
    )
    .unwrap();
    assert_eq!(config.generation().unwrap().model, "gpt-6-luna");
    assert_eq!(config.story.candidates, 1);
}
