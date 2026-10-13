use crate::composition::{
    CredentialError, Credentials, Secret,
    configuration::components::{GENERATION_KEY, SOURCE_KEY},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use yomibu_core::ports::infrastructure::credentials::SecretStore;

struct SuppliedStore {
    reads: Arc<AtomicUsize>,
    result: Result<Option<Secret>, CredentialError>,
}
impl SecretStore for SuppliedStore {
    fn read(&self, service: &str, account: &str) -> Result<Option<Secret>, CredentialError> {
        assert_eq!((service, account), ("yomibu:openai", "api-key"));
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }

    fn write(&self, _: &str, _: &str, _: &Secret) -> Result<(), CredentialError> {
        panic!("credential lookup wrote to the store")
    }
}

#[test]
fn component_credentials_follow_precedence_without_cross_component_fallback() {
    let mut credentials = Credentials::default();
    credentials.supply(GENERATION_KEY, "stored-generation".into());
    credentials.supply(SOURCE_KEY, "stored-source".into());
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
        credentials.resolve(SOURCE_KEY).unwrap(),
        Some("stored-source")
    );
    credentials.set_cli(GENERATION_KEY, "".into());
    assert!(credentials.resolve(GENERATION_KEY).unwrap().is_none());
    credentials.set_environment(SOURCE_KEY, " ".into());
    assert!(credentials.resolve(SOURCE_KEY).unwrap().is_none());
    let debug = format!("{credentials:?}");
    for secret in [
        "stored-generation",
        "stored-source",
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
        let store = SuppliedStore {
            reads: Arc::clone(&reads),
            result,
        };
        let mut credentials = Credentials::default();
        let (service, account) = crate::composition::credentials::identity(GENERATION_KEY);
        credentials.supply_with(GENERATION_KEY, move || store.read(&service, account));
        assert!(credentials.resolve(SOURCE_KEY).unwrap().is_none());
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
