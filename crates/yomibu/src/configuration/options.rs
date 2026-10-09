use super::{ConfigError, Operation, Patch, components};
use components::{OptionError, OptionKey, OptionType, Options, Setting, Value};
use serde::{Deserialize, Deserializer, de::Error};
use std::collections::BTreeMap;
use yomibu_core::component::options::OptionName;

#[derive(Debug, Default)]
pub struct OptionOverrides(BTreeMap<OptionName, Patch<Value>>);
impl OptionOverrides {
    /// ```compile_fail
    /// use yomibu::configuration::{OptionOverrides, components::EMBEDDING_DIMENSIONS};
    /// let mut options = OptionOverrides::default();
    /// options.set(EMBEDDING_DIMENSIONS, "not an integer".to_string());
    /// ```
    pub fn set<T: OptionType>(&mut self, key: OptionKey<T>, value: T) -> Result<(), OptionError> {
        self.insert(key.name, Patch::Set(value.value()))
    }
    pub fn clear<T>(&mut self, key: OptionKey<T>) -> Result<(), OptionError> {
        self.insert(key.name, Patch::Clear)
    }
    pub fn insert(&mut self, name: OptionName, value: Patch<Value>) -> Result<(), OptionError> {
        let setting = components::options()
            .find(|setting| setting.name() == name)
            .ok_or(OptionError::Invalid {
                name,
                reason: "Unknown option or credential in non-secret settings.",
            })?;
        if let Patch::Set(value) = &value {
            setting.check(value)?;
        }
        self.0.insert(name, value);
        Ok(())
    }
    pub(super) fn apply(
        mut self,
        options: &mut Options,
        operation: &Operation,
    ) -> Result<(), ConfigError> {
        for setting in components::options() {
            if !operation.uses_setting(&path(setting)) {
                continue;
            }
            match self.0.remove(&setting.name()) {
                Some(Patch::Set(value)) => options.insert(setting, value),
                Some(Patch::Clear) => options.clear(setting),
                _ => Ok(()),
            }?;
        }
        Ok(())
    }
}
impl<'de> Deserialize<'de> for OptionOverrides {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = BTreeMap::<String, BTreeMap<String, Patch<Value>>>::deserialize(deserializer)?;
        let mut options = Self::default();
        for (id, values) in input {
            let component = components::COMPONENTS
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| D::Error::custom("Unknown component."))?;
            for (name, value) in values {
                let setting = component
                    .setting(&name)
                    .ok_or_else(|| D::Error::custom("Unknown component option."))?;
                options
                    .insert(setting.name(), value)
                    .map_err(D::Error::custom)?;
            }
        }
        Ok(options)
    }
}
pub(super) fn path(setting: Setting) -> String {
    format!("pipeline.options.{}", setting.name().key())
}
