use super::{
    ConfigError, Patch,
    components::{self, CredentialRequirement},
};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const DEFAULT_CREDENTIAL_BINDINGS: &[(CredentialRequirement, &str)] = &[
    (components::SOURCE_KEY, "wanikani"),
    (components::GENERATION_KEY, "openai"),
    (components::EMBEDDING_KEY, "openai"),
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialBinding {
    pub provider: CredentialProvider,
    pub key: String,
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialProvider {
    Supplied,
}

#[derive(Debug, Clone)]
pub struct CredentialBindings(BTreeMap<String, CredentialBinding>);
impl Default for CredentialBindings {
    fn default() -> Self {
        Self(
            DEFAULT_CREDENTIAL_BINDINGS
                .iter()
                .map(|(requirement, shared)| {
                    (
                        requirement.name.key(),
                        CredentialBinding {
                            provider: CredentialProvider::Supplied,
                            key: (*shared).into(),
                        },
                    )
                })
                .collect(),
        )
    }
}
impl CredentialBindings {
    pub fn get(&self, requirement: CredentialRequirement) -> Option<&CredentialBinding> {
        self.0.get(&requirement.name.key())
    }
    pub(super) fn resolve(
        patches: BTreeMap<String, Patch<CredentialBinding>>,
    ) -> Result<Self, ConfigError> {
        let mut bindings = Self::default();
        for (target, patch) in patches {
            let (_, shared) = DEFAULT_CREDENTIAL_BINDINGS
                .iter()
                .find(|(requirement, _)| requirement.name.key() == target)
                .ok_or(ConfigError::InvalidSetting(
                    "Unknown credential requirement.",
                ))?;
            match patch {
                Patch::Set(binding) if binding.key == *shared || binding.key == target => {
                    bindings.0.insert(target, binding);
                }
                Patch::Set(_) => {
                    return Err(ConfigError::InvalidSetting(
                        "Credential binding must use its own component slot or its explicit provider slot.",
                    ));
                }
                Patch::Clear => {
                    bindings.0.remove(&target);
                }
                Patch::Inherit => {}
            }
        }
        Ok(bindings)
    }
}
