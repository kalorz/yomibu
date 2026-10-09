use crate::configuration::{
    components,
    credentials::{CredentialBindings, CredentialProvider},
};
use std::collections::BTreeMap;
use yomibu_core::capabilities::options::CredentialRequirement;

#[derive(Clone)]
pub struct Secret(Option<String>);
impl Secret {
    pub fn new(value: String) -> Self {
        Self(Some(value))
    }
    pub fn invalid_encoding() -> Self {
        Self(None)
    }
    pub fn expose(&self) -> Result<&str, CredentialError> {
        self.0.as_deref().ok_or(CredentialError)
    }
}
impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self::new(value.into())
    }
}
#[derive(Debug, thiserror::Error)]
#[error("Invalid credential input; supply a UTF-8 credential.")]
pub struct CredentialError;

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Default, Debug)]
pub struct Credentials {
    cli: BTreeMap<String, Secret>,
    environment: BTreeMap<String, Secret>,
    supplied: BTreeMap<String, Secret>,
}
impl Credentials {
    pub fn supply(&mut self, key: &str, value: Secret) {
        self.supplied.insert(key.into(), value);
    }
    pub fn set_cli(&mut self, requirement: CredentialRequirement, value: Secret) {
        self.cli.insert(requirement.name.key(), value);
    }
    pub fn set_environment(&mut self, requirement: CredentialRequirement, value: Secret) {
        self.environment.insert(requirement.name.key(), value);
    }
    pub fn set_shared_cli(&mut self, key: &str, value: Secret) {
        self.cli.insert(key.into(), value);
    }
    pub fn set_shared_environment(&mut self, key: &str, value: Secret) {
        self.environment.insert(key.into(), value);
    }
    pub fn resolve(
        &self,
        requirement: CredentialRequirement,
        bindings: &CredentialBindings,
    ) -> Result<Option<&str>, CredentialError> {
        let target = requirement.name.key();
        let binding = bindings.get(requirement);
        self.cli
            .get(&target)
            .or_else(|| binding.and_then(|binding| self.cli.get(&binding.key)))
            .or_else(|| self.environment.get(&target))
            .or_else(|| binding.and_then(|binding| self.environment.get(&binding.key)))
            .or_else(|| {
                binding.and_then(|binding| match binding.provider {
                    CredentialProvider::Supplied => self.supplied.get(&binding.key),
                })
            })
            .map(Secret::expose)
            .transpose()
            .map(|value| value.filter(|value| !value.trim().is_empty()))
    }
    pub(crate) fn source<'a>(
        &'a self,
        bindings: &CredentialBindings,
    ) -> Result<Option<&'a str>, CredentialError> {
        self.resolve(components::SOURCE_KEY, bindings)
    }
    pub(crate) fn generation<'a>(
        &'a self,
        bindings: &CredentialBindings,
    ) -> Result<Option<&'a str>, CredentialError> {
        self.resolve(components::GENERATION_KEY, bindings)
    }

    pub(crate) fn is_missing(
        &self,
        requirement: CredentialRequirement,
        bindings: &CredentialBindings,
    ) -> bool {
        matches!(self.resolve(requirement, bindings), Ok(None))
    }
}
