use crate::configuration::components;
use std::{collections::BTreeMap, sync::OnceLock};
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
        self.0.as_deref().ok_or(CredentialError::InvalidEncoding)
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
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum CredentialError {
    #[error("Invalid credential input; supply a UTF-8 credential.")]
    InvalidEncoding,
    #[error(
        "Cannot access the credential store; unlock it or supply a CLI/environment credential."
    )]
    StoreUnavailable,
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Default, Debug)]
pub struct Credentials {
    cli: BTreeMap<String, Secret>,
    environment: BTreeMap<String, Secret>,
    supplied: BTreeMap<String, SuppliedCredential>,
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
    pub fn supply(&mut self, requirement: CredentialRequirement, value: Secret) {
        self.supplied
            .insert(requirement.name.key(), SuppliedCredential::Value(value));
    }
    /// Read a component credential only when needed, caching its value, absence, or error.
    pub fn supply_with(
        &mut self,
        requirement: CredentialRequirement,
        read: impl Fn() -> Result<Option<Secret>, CredentialError> + Send + Sync + 'static,
    ) {
        self.supplied.insert(
            requirement.name.key(),
            SuppliedCredential::Lookup {
                read: Box::new(read),
                cached: OnceLock::new(),
            },
        );
    }
    pub fn set_cli(&mut self, requirement: CredentialRequirement, value: Secret) {
        self.cli.insert(requirement.name.key(), value);
    }
    pub fn set_environment(&mut self, requirement: CredentialRequirement, value: Secret) {
        self.environment.insert(requirement.name.key(), value);
    }
    pub fn has_override(&self, requirement: CredentialRequirement) -> bool {
        let target = requirement.name.key();
        self.cli.contains_key(&target) || self.environment.contains_key(&target)
    }
    pub fn resolve(
        &self,
        requirement: CredentialRequirement,
    ) -> Result<Option<&str>, CredentialError> {
        let target = requirement.name.key();
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

    pub(crate) fn is_missing(&self, requirement: CredentialRequirement) -> bool {
        matches!(self.resolve(requirement), Ok(None))
    }
}
