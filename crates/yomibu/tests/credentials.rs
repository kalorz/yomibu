use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use yomibu::{
    application::{CredentialError, Credentials, Secret},
    configuration::components::{EMBEDDING_KEY, GENERATION_KEY, SOURCE_KEY},
};

#[test]
fn module_setup_names_match_the_component_credential_declarations() {
    use yomibu::configuration::modules::ModuleId;
    for (module, requirement) in [
        (ModuleId::Sync, SOURCE_KEY),
        (ModuleId::Generation, GENERATION_KEY),
        (ModuleId::Embeddings, EMBEDDING_KEY),
    ] {
        let metadata = module.metadata();
        for setting in [
            format!("--{}", requirement.name.cli()),
            requirement.name.environment(),
        ] {
            assert!(
                metadata.settings.contains(&setting),
                "Missing {setting} in {}",
                metadata.name
            );
        }
        assert!(
            metadata
                .guidance
                .contains(&format!("yomibu auth {}", requirement.name.component))
        );
    }
}

#[test]
fn component_credentials_follow_precedence_without_cross_component_fallback() {
    let mut credentials = Credentials::default();
    credentials.supply(GENERATION_KEY, "stored-generation".into());
    credentials.supply(EMBEDDING_KEY, "stored-embedding".into());
    assert_eq!(
        credentials.resolve(GENERATION_KEY).unwrap(),
        Some("stored-generation")
    );
    credentials.set_environment(GENERATION_KEY, "env-generation".into());
    assert_eq!(
        credentials.resolve(GENERATION_KEY).unwrap(),
        Some("env-generation")
    );
    credentials.set_cli(GENERATION_KEY, "cli-generation".into());
    assert_eq!(
        credentials.resolve(GENERATION_KEY).unwrap(),
        Some("cli-generation")
    );
    assert_eq!(
        credentials.resolve(EMBEDDING_KEY).unwrap(),
        Some("stored-embedding")
    );
    assert!(credentials.resolve(SOURCE_KEY).unwrap().is_none());
    credentials.set_cli(GENERATION_KEY, "".into());
    assert!(credentials.resolve(GENERATION_KEY).unwrap().is_none());
    credentials.set_environment(EMBEDDING_KEY, " ".into());
    assert!(credentials.resolve(EMBEDDING_KEY).unwrap().is_none());
    let debug = format!("{credentials:?}");
    for secret in [
        "stored-generation",
        "stored-embedding",
        "env-generation",
        "cli-generation",
    ] {
        assert!(!debug.contains(secret));
    }
}

#[test]
fn credential_lookup_is_lazy_and_caches_values_absence_and_errors() {
    for (result, expected) in [
        (
            Ok(Some("synthetic-stored".into())),
            Ok(Some("synthetic-stored")),
        ),
        (Ok(None), Ok(None)),
        (
            Err(CredentialError::StoreUnavailable),
            Err(CredentialError::StoreUnavailable.to_string()),
        ),
    ] {
        let reads = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&reads);
        let mut credentials = Credentials::default();
        credentials.supply_with(GENERATION_KEY, move || {
            observed.fetch_add(1, Ordering::SeqCst);
            result.clone()
        });
        assert!(credentials.resolve(EMBEDDING_KEY).unwrap().is_none());
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        let first = credentials
            .resolve(GENERATION_KEY)
            .map_err(|e| e.to_string());
        let second = credentials
            .resolve(GENERATION_KEY)
            .map_err(|e| e.to_string());
        assert_eq!(first, expected);
        assert_eq!(first, second);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert!(!format!("{credentials:?}").contains("synthetic-stored"));
    }
}

#[test]
fn explicit_inputs_bypass_the_store_including_blank_and_invalid_values() {
    for cli in [false, true] {
        for value in [
            Secret::from(""),
            Secret::from("synthetic-env"),
            Secret::invalid_encoding(),
        ] {
            let expected = value
                .expose()
                .map(|v| if v.is_empty() { None } else { Some(v) })
                .map_err(|e| e.to_string());
            let mut credentials = Credentials::default();
            credentials.supply_with(GENERATION_KEY, || panic!("Explicit input read the store"));
            if cli {
                credentials.set_cli(GENERATION_KEY, value.clone());
            } else {
                credentials.set_environment(GENERATION_KEY, value.clone());
            }
            assert_eq!(
                credentials
                    .resolve(GENERATION_KEY)
                    .map_err(|e| e.to_string()),
                expected
            );
        }
    }
}

#[test]
fn module_help_uses_all_declared_options_and_their_credential_guidance() {
    use yomibu::configuration::{components::COMPONENTS, modules::ModuleId};
    for (module, id) in [
        (ModuleId::Sync, "wanikani"),
        (ModuleId::Generation, "openai"),
        (ModuleId::Embeddings, "http-embeddings"),
    ] {
        let component = COMPONENTS
            .iter()
            .find(|component| component.id == id)
            .unwrap();
        let metadata = module.metadata();
        for setting in component.settings {
            assert!(
                metadata
                    .settings
                    .iter()
                    .any(|s| s == &format!("--{}", setting.name().cli())),
                "Missing {} in {}",
                setting.name().key(),
                metadata.name
            );
            if let Some(key) = setting.secret() {
                assert!(
                    metadata.guidance.contains(key.description),
                    "Missing component-owned guidance for {}",
                    key.name.key()
                );
            }
        }
        let json = serde_json::to_value(metadata).unwrap();
        assert!(
            json["settings"]
                .as_array()
                .unwrap()
                .iter()
                .all(|value| value.is_string())
        );
        assert_eq!(json["guidance"].as_str().unwrap(), metadata.guidance);
    }
}
