use super::*;
use yomibu::{
    application::Operation,
    configuration::{Configuration, ConfigurationInput, ProcessOverrides},
};

fn configuration(application: &str, pipeline: &str, environment: &[(&str, &str)]) -> Configuration {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), application).unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), pipeline).unwrap();
    std::fs::write(dir.path().join("default-story.toml"), "broken story TOML").unwrap();
    Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: environment
                .iter()
                .map(|(name, value)| ((*name).into(), (*value).into()))
                .collect(),
            flags: ProcessOverrides::default(),
        },
        &Operation::Auth,
    )
    .unwrap()
}

#[test]
fn setup_honors_environment_controls_and_keeps_component_keys_separate() {
    let config = configuration(
        "",
        "[pipeline.embedding]\nprovider='openai'\n",
        &[("YOMIBU_DISABLE", "sync"), ("YOMIBU_ENABLE", "embeddings")],
    );
    let needs = pending(
        &Target::Missing(config.credential_requirements().collect()),
        &Credentials::default(),
        &mut Vec::new(),
    )
    .unwrap();
    let keys: Vec<_> = needs.iter().map(|need| need.name.key()).collect();
    assert_eq!(keys, ["http-embeddings.api-key", "openai.api-key"]);
    let mut credentials = Credentials::default();
    credentials.supply(components::GENERATION_KEY, "generation-only".into());
    let needs = pending(
        &Target::Missing(config.credential_requirements().collect()),
        &credentials,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0].name, components::EMBEDDING_KEY.name);
}

#[test]
fn disabled_or_local_integrations_do_not_request_credentials() {
    for (enabled, provider) in [
        (false, "openai"),
        (true, "local"),
        (true, "lexical-baseline"),
    ] {
        let config = configuration(
            "[application]\nsync=false\n",
            &format!(
                "[pipeline.selection]\nembeddings={enabled}\n[pipeline.embedding]\nprovider='{provider}'\n"
            ),
            &[],
        );
        let mut credentials = Credentials::default();
        for requirement in [components::SOURCE_KEY, components::EMBEDDING_KEY] {
            credentials.supply_with(requirement, || {
                panic!("Disabled or local integration read a credential")
            });
        }
        let needs = pending(
            &Target::Missing(config.credential_requirements().collect()),
            &credentials,
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(needs.len(), 1);
        assert_eq!(needs[0].name, components::GENERATION_KEY.name);
    }
}

#[test]
fn namespace_skips_present_keys_while_exact_target_allows_replacement() {
    let mut credentials = Credentials::default();
    credentials.supply_with(components::GENERATION_KEY, || {
        panic!("Unselected component read the store")
    });
    let selected = select("http-embeddings").unwrap();
    let missing = pending(&selected, &credentials, &mut Vec::new()).unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].name, components::EMBEDDING_KEY.name);
    credentials.set_environment(components::EMBEDDING_KEY, "synthetic-env".into());
    assert!(
        pending(&selected, &credentials, &mut Vec::new())
            .unwrap()
            .is_empty()
    );
    credentials.supply_with(components::SOURCE_KEY, || {
        Err(CredentialError::StoreUnavailable)
    });
    assert_eq!(
        pending(&select("wanikani").unwrap(), &credentials, &mut Vec::new())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        pending(
            &select("wanikani.api-key").unwrap(),
            &credentials,
            &mut Vec::new()
        )
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        pending(
            &select("http-embeddings.api-key").unwrap(),
            &credentials,
            &mut Vec::new()
        )
        .unwrap()
        .len(),
        1
    );
    assert!(select("http").is_err());
}

#[test]
fn blank_override_does_not_fall_back_to_a_stored_key_during_setup() {
    let mut credentials = Credentials::default();
    credentials.set_environment(components::GENERATION_KEY, "".into());
    credentials.supply_with(components::GENERATION_KEY, || {
        panic!("Blank override read the store")
    });
    let needs = pending(&select("openai").unwrap(), &credentials, &mut Vec::new()).unwrap();
    assert_eq!(needs.len(), 1);
}

#[test]
fn replacement_warns_about_explicit_overrides_without_reading_the_store_or_exposing_keys() {
    for cli in [false, true] {
        for value in [
            Secret::from("synthetic-secret\n\u{1b}"),
            Secret::from(""),
            Secret::invalid_encoding(),
        ] {
            let mut credentials = Credentials::default();
            credentials.supply_with(components::GENERATION_KEY, || {
                panic!("Replacement read the store")
            });
            if cli {
                credentials.set_cli(components::GENERATION_KEY, value);
            } else {
                credentials.set_environment(components::GENERATION_KEY, value);
            }
            let mut warnings = Vec::new();
            let needs = pending(
                &select("openai.api-key").unwrap(),
                &credentials,
                &mut warnings,
            )
            .unwrap();
            assert_eq!(needs.len(), 1);
            assert_eq!(
                String::from_utf8(warnings).unwrap(),
                "warning: openai.api-key: CLI/environment input overrides the saved Keychain credential. Omit --openai-api-key and unset YOMIBU_OPENAI_API_KEY to use it.\n"
            );
            let mut warnings = Vec::new();
            pending(
                &select("wanikani.api-key").unwrap(),
                &credentials,
                &mut warnings,
            )
            .unwrap();
            assert!(warnings.is_empty());
        }
    }
}

#[test]
fn a_failed_credential_lookup_does_not_block_other_setup() {
    for error in [
        CredentialError::InvalidEncoding,
        CredentialError::StoreUnavailable,
    ] {
        let mut credentials = Credentials::default();
        match error {
            CredentialError::InvalidEncoding => {
                credentials.set_environment(components::EMBEDDING_KEY, Secret::invalid_encoding())
            }
            CredentialError::StoreUnavailable => {
                credentials.supply_with(components::EMBEDDING_KEY, move || Err(error))
            }
        }
        let mut warnings = Vec::new();
        let needs = pending(
            &Target::Missing(components::credentials().collect()),
            &credentials,
            &mut warnings,
        )
        .unwrap();
        let warnings = String::from_utf8(warnings).unwrap();
        assert!(
            warnings.starts_with(&format!(
                "warning: http-embeddings.api-key: {error} Continuing setup.\n"
            )),
            "{warnings}"
        );
        assert_eq!(
            needs.iter().map(|need| need.name.key()).collect::<Vec<_>>(),
            [
                "http-embeddings.api-key",
                "openai.api-key",
                "wanikani.api-key"
            ]
        );
    }
}

#[test]
fn empty_input_skips_and_invalid_keys_are_rejected_without_exposing_them() {
    assert!(api_key(String::new()).unwrap().is_none());
    assert_eq!(
        api_key("synthetic-valid".into()).unwrap().as_deref(),
        Some("synthetic-valid")
    );
    for value in ["synthetic-secret\nInjected", "synthetic-secret\u{1b}", " "] {
        assert_eq!(
            api_key(value.into()).unwrap_err().to_string(),
            "API key must be nonblank printable ASCII without spaces."
        );
    }
}

#[test]
fn keychain_names_follow_component_and_credential_names() {
    for (requirement, service) in [
        (components::EMBEDDING_KEY, "yomibu:http-embeddings"),
        (components::GENERATION_KEY, "yomibu:openai"),
        (components::SOURCE_KEY, "yomibu:wanikani"),
    ] {
        assert_eq!(identity(requirement), (service.into(), "api-key"));
        assert_eq!(
            select(&requirement.name.key()).unwrap().requirements()[0].name,
            requirement.name
        );
    }
}

#[test]
fn auth_configuration_ignores_story_inputs_and_unused_model_options() {
    let config = configuration(
        "[application]\nsync=false\n",
        "[pipeline]\nmodel=7\n[pipeline.selection]\nsteps=[]\nembeddings=true\n[pipeline.embedding]\nprovider='openai'\n",
        &[],
    );
    let needs = pending(
        &Target::Missing(config.credential_requirements().collect()),
        &Credentials::default(),
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(needs.len(), 2);
}
