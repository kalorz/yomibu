use std::{collections::BTreeMap, path::PathBuf};
use yomibu::app::config::{Configuration, ConfigurationInput, Settings};

#[test]
fn resolves_each_model_setting_before_job_fallback_and_paths_relative_to_config() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.toml");
    std::fs::write(&file, "model = 'file-default'\ngeneration_model = 'file-generation'\ninventory = 'inventory.json'\n").unwrap();
    let env = BTreeMap::from([("YOMIBU_MODEL".into(), "environment-default".into())]);
    let config = Configuration::load(ConfigurationInput {
        data_dir: Some(dir.path().into()),
        config: None,
        home: None,
        environment: env,
        flags: Settings {
            model: Some("flag-default".into()),
            ..Default::default()
        },
    })
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
    let error = Configuration::load(ConfigurationInput {
        data_dir: Some(dir.path().into()),
        config: None,
        home: None,
        environment: BTreeMap::new(),
        flags: Settings::default(),
    })
    .unwrap_err();
    assert!(!format!("{error:?} {error}").contains("synthetic-secret"));
    assert!(error.to_string().contains("config.toml"));
}

#[test]
fn construction_with_defaults_neither_creates_files_nor_reads_environment() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("new");
    let config = Configuration::load(ConfigurationInput {
        data_dir: None,
        config: None,
        home: Some(data_dir.clone()),
        environment: BTreeMap::new(),
        flags: Settings::default(),
    })
    .unwrap();
    assert_eq!(config.data_dir, PathBuf::from(&data_dir).join(".yomibu"));
    assert!(!data_dir.exists());
}

#[test]
fn module_controls_override_saved_choices_but_conflicts_remain_errors() {
    use yomibu::app::modules::ModuleId;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "enable = ['embeddings']\ndisable = ['sync']\n",
    )
    .unwrap();
    let config = Configuration::load(ConfigurationInput {
        data_dir: Some(dir.path().into()),
        config: None,
        home: None,
        environment: BTreeMap::new(),
        flags: Settings {
            enable: vec![ModuleId::Sync],
            disable: vec![ModuleId::Embeddings],
            ..Default::default()
        },
    })
    .unwrap();
    assert!(config.enabled(ModuleId::Sync));
    assert!(!config.enabled(ModuleId::Embeddings));
    assert!(config.enabled(ModuleId::Assessment));
    assert!(
        Configuration::load(ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: Settings {
                enable: vec![ModuleId::Assessment],
                disable: vec![ModuleId::Assessment],
                ..Default::default()
            }
        })
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
    let config = Configuration::load(ConfigurationInput {
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
    })
    .unwrap();
    assert_eq!(config.generation.model, "flag-generation");
    assert_eq!(config.select, 3);
}
