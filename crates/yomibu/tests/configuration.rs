use std::{collections::BTreeMap, path::PathBuf};
use yomibu::{
    application::Operation,
    configuration::{Configuration, ConfigurationInput, Settings},
};

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
    let unused_story =
        "model = 7\nformat = 'unused-invalid'\nselect = 'unused-invalid'\ncandidates = 0\n";
    for (operation, text) in [
        (Operation::Status, format!("{unused_story}wanikani_cache = 'source.json'\n")),
        (Operation::Sync, format!("{unused_story}wanikani_cache = 'source.json'\n")),
        (Operation::Analyze("input.json".into()), format!("{unused_story}dictionary_dir = 'managed'\n")),
        (Operation::Import("bundle".into()), format!("{unused_story}dictionary_dir = 'managed'\n")),
        (Operation::Verify, format!("{unused_story}dictionary_dir = 'managed'\n")),
        (Operation::Retrieval, "model = 7\nformat = 'unused-invalid'\ncandidates = 0\nselect = 2\nembedding_provider = 'lexical-baseline'\n".into()),
        (Operation::Preview, "dictionary_dir = false\n".into()),
    ] {
        std::fs::write(dir.path().join("config.toml"), text).unwrap();
        let config = Configuration::load(ConfigurationInput {
            data_dir: Some(dir.path().into()), config: None, home: None,
            environment: BTreeMap::new(), flags: Settings::default(),
        }, &operation).unwrap_or_else(|error| panic!("{operation:?}: {error}"));
        match operation {
            Operation::Status | Operation::Sync => assert_eq!(config.wanikani_cache, Some(dir.path().join("source.json"))),
            Operation::Analyze(_) => assert_eq!(config.dictionary_dir, dir.path().join("managed")),
            Operation::Import(_) | Operation::Verify => {
                assert_eq!(config.dictionary_dir, dir.path().join("managed"));
            }
            Operation::Retrieval => assert_eq!(config.select, 2),
            Operation::Preview => assert_eq!(config.dictionary_dir, dir.path().join("dictionaries")),
            Operation::Story => unreachable!(),
        }
    }
    for text in [
        "select = 'invalid'",
        "misspelled_setting = 1",
        "bad syntax '",
    ] {
        std::fs::write(dir.path().join("config.toml"), text).unwrap();
        assert!(
            Configuration::load(
                ConfigurationInput {
                    data_dir: Some(dir.path().into()),
                    config: None,
                    home: None,
                    environment: BTreeMap::new(),
                    flags: Settings::default(),
                },
                &Operation::Story
            )
            .is_err()
        );
    }
}

#[test]
fn resolves_each_model_setting_before_job_fallback_and_paths_relative_to_config() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.toml");
    std::fs::write(&file, "model = 'file-default'\ngeneration_model = 'file-generation'\ninventory = 'inventory.json'\n").unwrap();
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
    assert_eq!(config.generation.model, "file-generation");
    assert_eq!(config.inventory, Some(dir.path().join("inventory.json")));
    assert_eq!(config.cache_max_age.as_secs(), 3600);
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
    assert_eq!(config.data_dir, PathBuf::from(&data_dir).join(".yomibu"));
    assert!(!data_dir.exists());
}

#[test]
fn module_controls_override_saved_choices_but_conflicts_remain_errors() {
    use yomibu::configuration::modules::ModuleId;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "enable = ['embeddings']\ndisable = ['sync']\n",
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
    std::fs::write(
        dir.path().join("config.toml"),
        "model = 'file-model'\ngeneration_model = 'file-generation'\nselect = 2\n",
    )
    .unwrap();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::from([
                ("YOMIBU_GENERATION_MODEL".into(), "env-generation".into()),
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
    assert_eq!(config.generation.model, "flag-generation");
    assert_eq!(config.select, 3);
}
