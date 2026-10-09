use crate::configuration::components;
use std::{collections::BTreeMap, sync::OnceLock};
pub use yomibu_core::capabilities::options::{CredentialError, Secret};
use yomibu_core::capabilities::options::{OptionKey, OptionName};

#[derive(Default, Debug)]
pub struct Credentials {
    cli: BTreeMap<OptionName, Secret>,
    environment: BTreeMap<OptionName, Secret>,
    supplied: BTreeMap<OptionName, SuppliedCredential>,
}
enum SuppliedCredential {
    Value(Secret),
    Lookup {
        read: Box<dyn Fn() -> Result<Option<Secret>, CredentialError> + Send + Sync>,
        cached: OnceLock<Result<Option<Secret>, CredentialError>>,
    },
}
impl std::fmt::Debug for SuppliedCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}
impl SuppliedCredential {
    fn resolve(&self) -> Result<Option<&Secret>, CredentialError> {
        match self {
            Self::Value(value) => Ok(Some(value)),
            Self::Lookup { read, cached } => cached
                .get_or_init(read)
                .as_ref()
                .map(Option::as_ref)
                .map_err(|error| *error),
        }
    }
}
impl Credentials {
    pub fn supply(&mut self, requirement: OptionKey<Secret>, value: Secret) {
        self.supplied
            .insert(requirement.name, SuppliedCredential::Value(value));
    }
    /// Read a component credential only when needed, caching its value, absence, or error.
    pub fn supply_with(
        &mut self,
        requirement: OptionKey<Secret>,
        read: impl Fn() -> Result<Option<Secret>, CredentialError> + Send + Sync + 'static,
    ) {
        self.supplied.insert(
            requirement.name,
            SuppliedCredential::Lookup {
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

    pub(crate) fn is_missing(&self, requirement: OptionKey<Secret>) -> bool {
        matches!(self.resolve(requirement), Ok(None))
    }
}
