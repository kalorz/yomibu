use std::collections::BTreeMap;
use yomibu::{
    application::Credentials,
    configuration::{Configuration, ConfigurationInput, ProcessOverrides, components},
};

#[test]
fn bound_credential_lookup_is_lazy_shared_and_cached() {
    let dir = tempfile::tempdir().unwrap();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: ProcessOverrides::default(),
        },
        &yomibu::application::Operation::Story,
    )
    .unwrap();
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = std::sync::Arc::clone(&reads);
    let mut credentials = Credentials::default();
    credentials.supply_with("openai", move || {
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Some("stored-secret".into()))
    });
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    credentials.set_shared_environment("openai", "".into());
    assert!(
        credentials
            .resolve(
                components::GENERATION_KEY,
                &config.application.credential_bindings
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(!format!("{credentials:?}").contains("stored-secret"));

    let mut credentials = Credentials::default();
    let observed = std::sync::Arc::clone(&reads);
    credentials.supply_with("openai", move || {
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Some("stored-secret".into()))
    });
    for requirement in [components::GENERATION_KEY, components::EMBEDDING_KEY] {
        assert_eq!(
            credentials
                .resolve(requirement, &config.application.credential_bindings)
                .unwrap(),
            Some("stored-secret")
        );
    }
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(!format!("{credentials:?}").contains("stored-secret"));
}

#[test]
fn private_binding_never_reads_the_shared_store_and_store_errors_are_cached() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), "[application.credentials]\n\"http-embeddings.api-key\"={provider='supplied',key='http-embeddings.api-key'}\n").unwrap();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: ProcessOverrides::default(),
        },
        &yomibu::application::Operation::Story,
    )
    .unwrap();
    let mut credentials = Credentials::default();
    credentials.supply_with("openai", || panic!("Private binding read the shared slot"));
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = std::sync::Arc::clone(&reads);
    credentials.supply_with("http-embeddings.api-key", move || {
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(yomibu::application::CredentialError::StoreUnavailable)
    });
    for _ in 0..2 {
        assert!(
            credentials
                .resolve(
                    components::EMBEDDING_KEY,
                    &config.application.credential_bindings
                )
                .is_err()
        );
    }
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn credentials_are_explicitly_shared_and_a_blank_override_does_not_fall_through() {
    let dir = tempfile::tempdir().unwrap();
    let config = Configuration::load(
        ConfigurationInput {
            data_dir: Some(dir.path().into()),
            config: None,
            home: None,
            environment: BTreeMap::new(),
            flags: ProcessOverrides::default(),
        },
        &yomibu::application::Operation::Story,
    )
    .unwrap();
    let mut credentials = Credentials::default();
    credentials.supply("openai", "shared-secret".into());
    credentials.set_environment(components::GENERATION_KEY, "environment-secret".into());
    credentials.set_cli(components::GENERATION_KEY, "".into());
    assert!(
        credentials
            .resolve(
                components::GENERATION_KEY,
                &config.application.credential_bindings
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(
        credentials
            .resolve(
                components::EMBEDDING_KEY,
                &config.application.credential_bindings
            )
            .unwrap(),
        Some("shared-secret")
    );
    assert!(
        credentials
            .resolve(
                components::SOURCE_KEY,
                &config.application.credential_bindings
            )
            .unwrap()
            .is_none()
    );
    let debug = format!("{credentials:?}");
    assert!(!debug.contains("shared-secret"));
    assert!(!debug.contains("environment-secret"));
}

#[test]
fn credential_bindings_control_sharing_and_input_precedence_without_cross_component_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let load = || {
        Configuration::load(
            ConfigurationInput {
                data_dir: Some(dir.path().into()),
                config: None,
                home: None,
                environment: BTreeMap::new(),
                flags: ProcessOverrides::default(),
            },
            &yomibu::application::Operation::Story,
        )
        .unwrap()
    };
    let config = load();
    let mut credentials = Credentials::default();
    credentials.supply("openai", "supplied".into());
    credentials.set_shared_environment("openai", "shared-env".into());
    assert_eq!(
        credentials
            .resolve(
                components::GENERATION_KEY,
                &config.application.credential_bindings
            )
            .unwrap(),
        Some("shared-env")
    );
    credentials.set_environment(components::GENERATION_KEY, "private-env".into());
    assert_eq!(
        credentials
            .resolve(
                components::GENERATION_KEY,
                &config.application.credential_bindings
            )
            .unwrap(),
        Some("private-env")
    );
    credentials.set_shared_cli("openai", "shared-cli".into());
    assert_eq!(
        credentials
            .resolve(
                components::GENERATION_KEY,
                &config.application.credential_bindings
            )
            .unwrap(),
        Some("shared-cli")
    );
    credentials.set_cli(components::GENERATION_KEY, "private-cli".into());
    assert_eq!(
        credentials
            .resolve(
                components::GENERATION_KEY,
                &config.application.credential_bindings
            )
            .unwrap(),
        Some("private-cli")
    );
    assert_eq!(
        credentials
            .resolve(
                components::EMBEDDING_KEY,
                &config.application.credential_bindings
            )
            .unwrap(),
        Some("shared-cli")
    );
    std::fs::write(dir.path().join("config.toml"),"[application.credentials]\n\"http-embeddings.api-key\"={provider='supplied',key='http-embeddings.api-key'}\n").unwrap();
    let private = load();
    assert!(
        credentials
            .resolve(
                components::EMBEDDING_KEY,
                &private.application.credential_bindings
            )
            .unwrap()
            .is_none()
    );
    credentials.supply("http-embeddings.api-key", "private-supplied".into());
    assert_eq!(
        credentials
            .resolve(
                components::EMBEDDING_KEY,
                &private.application.credential_bindings
            )
            .unwrap(),
        Some("private-supplied")
    );
    std::fs::write(
        dir.path().join("config.toml"),
        "[application.credentials]\n\"http-embeddings.api-key\"={clear=true}\n",
    )
    .unwrap();
    let cleared = load();
    assert!(
        credentials
            .resolve(
                components::EMBEDDING_KEY,
                &cleared.application.credential_bindings
            )
            .unwrap()
            .is_none()
    );
    credentials.set_environment(components::EMBEDDING_KEY, "direct-env".into());
    assert_eq!(
        credentials
            .resolve(
                components::EMBEDDING_KEY,
                &cleared.application.credential_bindings
            )
            .unwrap(),
        Some("direct-env")
    );
    assert!(
        credentials
            .resolve(
                components::SOURCE_KEY,
                &config.application.credential_bindings
            )
            .unwrap()
            .is_none()
    );
}
