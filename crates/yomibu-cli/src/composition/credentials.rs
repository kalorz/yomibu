use crate::composition::configuration::components;
use std::{collections::BTreeMap, sync::OnceLock};
use yomibu_core::extension::options::{OptionKey, OptionName};
use yomibu_core::ports::infrastructure::credentials::SecretStore;
pub use yomibu_core::ports::infrastructure::credentials::{CredentialError, Secret};
use yomibu_ext::macos_keychain::MacosKeychain;

#[derive(Default, Debug)]
pub struct Credentials {
    cli: BTreeMap<OptionName, Secret>,
    environment: BTreeMap<OptionName, Secret>,
    supplied: BTreeMap<OptionName, SuppliedCredential>,
}
struct SuppliedCredential {
    read: Box<dyn Fn() -> Result<Option<Secret>, CredentialError> + Send + Sync>,
    cached: OnceLock<Result<Option<Secret>, CredentialError>>,
}
impl std::fmt::Debug for SuppliedCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}
impl SuppliedCredential {
    fn resolve(&self) -> Result<Option<&Secret>, CredentialError> {
        self.cached
            .get_or_init(&self.read)
            .as_ref()
            .map(Option::as_ref)
            .map_err(|error| *error)
    }
}
impl Credentials {
    #[cfg(test)]
    pub fn supply(&mut self, requirement: OptionKey<Secret>, value: Secret) {
        self.supply_with(requirement, move || Ok(Some(value.clone())));
    }
    /// Read a component credential only when needed, caching its value, absence, or error.
    pub fn supply_with(
        &mut self,
        requirement: OptionKey<Secret>,
        read: impl Fn() -> Result<Option<Secret>, CredentialError> + Send + Sync + 'static,
    ) {
        self.supplied.insert(
            requirement.name,
            SuppliedCredential {
                read: Box::new(read),
                cached: OnceLock::new(),
            },
        );
    }
    pub fn set_cli(&mut self, requirement: OptionKey<Secret>, value: Secret) {
        self.cli.insert(requirement.name, value);
    }
    pub fn set_environment(&mut self, requirement: OptionKey<Secret>, value: Secret) {
        self.environment.insert(requirement.name, value);
    }
    pub fn has_override(&self, requirement: OptionKey<Secret>) -> bool {
        let target = requirement.name;
        self.cli.contains_key(&target) || self.environment.contains_key(&target)
    }
    pub fn resolve(&self, requirement: OptionKey<Secret>) -> Result<Option<&str>, CredentialError> {
        let target = requirement.name;
        let input = self
            .cli
            .get(&target)
            .or_else(|| self.environment.get(&target));
        let value = match input {
            Some(value) => Some(value),
            None => match self.supplied.get(&target) {
                Some(supplied) => supplied.resolve()?,
                None => None,
            },
        };
        value
            .map(Secret::expose)
            .transpose()
            .map(|value| value.filter(|value| !value.trim().is_empty()))
    }
    pub(crate) fn source(&self) -> Result<Option<&str>, CredentialError> {
        self.resolve(components::SOURCE_KEY)
    }
    pub(crate) fn generation(&self) -> Result<Option<&str>, CredentialError> {
        self.resolve(components::GENERATION_KEY)
    }
}

pub(crate) fn install_keychain(credentials: &mut Credentials, requirements: &[OptionKey<Secret>]) {
    if cfg!(target_os = "macos") {
        for &requirement in requirements {
            let (service, account) = identity(requirement);
            credentials.supply_with(requirement, move || {
                MacosKeychain
                    .read(&service, account)
                    .map_err(|_| CredentialError::StoreUnavailable)
            });
        }
    }
}

pub(crate) fn identity(requirement: OptionKey<Secret>) -> (String, &'static str) {
    (
        format!("yomibu:{}", requirement.name.component),
        requirement.name.option,
    )
}
