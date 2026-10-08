use super::{ConfigError, Patch, components};
use serde::Deserialize;
use std::collections::BTreeMap;

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
        Self(BTreeMap::from([
            (
                components::SOURCE_KEY.name.key(),
                CredentialBinding {
                    provider: CredentialProvider::Supplied,
                    key: "wanikani".into(),
                },
            ),
            (
                components::GENERATION_KEY.name.key(),
                CredentialBinding {
                    provider: CredentialProvider::Supplied,
                    key: "openai".into(),
                },
            ),
            (
                components::EMBEDDING_KEY.name.key(),
                CredentialBinding {
                    provider: CredentialProvider::Supplied,
                    key: "openai".into(),
                },
            ),
        ]))
    }
}
impl CredentialBindings {
    pub fn get(
        &self,
        requirement: yomibu_core::capabilities::options::CredentialRequirement,
    ) -> Option<&CredentialBinding> {
        self.0.get(&requirement.name.key())
    }
    pub(super) fn resolve(
        patches: BTreeMap<String, Patch<CredentialBinding>>,
    ) -> Result<Self, ConfigError> {
        let mut bindings = Self::default();
        for (target, patch) in patches {
            let shared = match target.as_str() {
                "wanikani-source.api-key" => "wanikani",
                "openai-story-generation.api-key" | "http-embeddings.api-key" => "openai",
                _ => {
                    return Err(ConfigError::InvalidSetting(
                        "Unknown credential requirement.",
                    ));
                }
            };
            match patch {
                Patch::Set(binding) if binding.key == shared || binding.key == target => {
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
