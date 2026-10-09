use super::*;
use yomibu::{
    application::Operation,
    configuration::{ConfigurationInput, ProcessOverrides},
};

fn configuration(application: &str, pipeline: &str) -> Configuration {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), application).unwrap();
    std::fs::write(dir.path().join("default-pipeline.toml"), pipeline).unwrap();
    std::fs::write(dir.path().join("default-story.toml"), "broken story TOML").unwrap();
    Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: ProcessOverrides::default(),
        },
        &Operation::Auth,
    )
    .unwrap()
}

#[test]
fn setup_deduplicates_shared_slots_and_promotes_required_uses() {
    let config = configuration(
        "",
        "[pipeline.selection]\nembeddings=true\n[pipeline.embedding]\nprovider='openai'\n",
    );
    let needs = missing_inputs(&config, &Credentials::default()).unwrap();
    assert_eq!(needs.len(), 2);
    assert_eq!(needs[0].key, "openai");
    assert!(needs[0].required());
    assert_eq!(needs[0].requirements.len(), 2);
    assert_eq!(needs[1].key, "wanikani");
    assert!(!needs[1].required());
}

#[test]
fn disabled_or_local_integrations_do_not_request_credentials() {
    for (enabled, provider) in [
        (false, "openai"),
        (true, "local"),
        (true, "lexical-baseline"),
    ] {
        let config = configuration(
            "[application]\nsync=false\n[application.credentials]\n\"http-embeddings.api-key\"={provider='supplied',key='http-embeddings.api-key'}\n",
            &format!(
                "[pipeline.selection]\nembeddings={enabled}\n[pipeline.embedding]\nprovider='{provider}'\n"
            ),
        );
        let mut credentials = Credentials::default();
        for key in ["wanikani", "http-embeddings.api-key"] {
            credentials.supply_with(key, || {
                panic!("Disabled or local integration read a credential")
            });
        }
        let needs = missing_inputs(&config, &credentials).unwrap();
        assert_eq!(needs.len(), 1);
        assert_eq!(needs[0].key, "openai");
    }
}

#[test]
fn setup_skips_environment_and_stored_keys_and_propagates_store_failure() {
    let config = configuration("", "");
    let mut credentials = Credentials::default();
    credentials.set_environment(components::GENERATION_KEY, "synthetic-env".into());
    credentials.supply_with("openai", || panic!("Environment override read the store"));
    credentials.supply_with("wanikani", || Ok(Some("synthetic-stored".into())));
    assert!(missing_inputs(&config, &credentials).unwrap().is_empty());
    credentials.supply_with("wanikani", || Err(CredentialError::StoreUnavailable));
    assert_eq!(
        missing_inputs(&config, &credentials)
            .err()
            .unwrap()
            .to_string(),
        CredentialError::StoreUnavailable.to_string()
    );
}

#[test]
fn private_binding_never_uses_shared_credentials() {
    let config = configuration(
        "[application]\nsync=false\n[application.credentials]\n\"openai-story-generation.api-key\"={provider='supplied',key='openai-story-generation.api-key'}\n",
        "",
    );
    let mut credentials = Credentials::default();
    credentials.set_shared_environment("openai", "synthetic-shared-env".into());
    credentials.supply_with("openai", || panic!("Private binding read the shared store"));
    let needs = missing_inputs(&config, &credentials).unwrap();
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0].key, "openai-story-generation.api-key");
    credentials.supply(
        "openai-story-generation.api-key",
        "synthetic-private".into(),
    );
    assert!(missing_inputs(&config, &credentials).unwrap().is_empty());
}

#[test]
fn blank_override_does_not_fall_back_to_a_stored_key_during_setup() {
    let config = configuration("[application]\nsync=false\n", "");
    let mut credentials = Credentials::default();
    credentials.set_shared_environment("openai", "".into());
    credentials.supply_with("openai", || panic!("Blank override read the store"));
    let needs = missing_inputs(&config, &credentials).unwrap();
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0].key, "openai");
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
fn keychain_names_preserve_shared_and_component_boundaries() {
    for (slot, service) in [
        ("openai", "yomibu:openai"),
        ("wanikani", "yomibu:wanikani"),
        (
            "openai-story-generation.api-key",
            "yomibu:openai-story-generation",
        ),
        ("http-embeddings.api-key", "yomibu:http-embeddings"),
    ] {
        assert_eq!(identity(slot), (service.into(), "api-key"));
        assert_eq!(target(slot).unwrap().key, slot);
    }
    assert!(target("unknown").is_err());
}

#[test]
fn auth_configuration_ignores_story_inputs_and_unused_model_options() {
    let config = configuration(
        "[application]\nsync=false\n",
        "[pipeline]\nmodel=7\n[pipeline.selection]\nsteps=[]\nembeddings=true\n[pipeline.embedding]\nprovider='openai'\n",
    );
    let needs = missing_inputs(&config, &Credentials::default()).unwrap();
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0].requirements.len(), 2);
}
