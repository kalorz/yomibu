use super::{Component, credentials::Secret};
use serde::Deserialize;
use std::{collections::BTreeMap, marker::PhantomData};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OptionName {
    pub component: &'static str,
    pub option: &'static str,
}

impl OptionName {
    pub const fn new(component: &'static str, option: &'static str) -> Self {
        Self { component, option }
    }

    pub fn cli(self) -> String {
        format!("{}-{}", self.component, self.option)
    }

    pub fn environment(self) -> String {
        format!(
            "YOMIBU_{}",
            self.cli().replace('-', "_").to_ascii_uppercase()
        )
    }

    pub fn key(self) -> String {
        format!("{}.{}", self.component, self.option)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OptionError {
    #[error("Supply --{flag}. {description}", flag = .name.cli())]
    Missing {
        name: OptionName,
        description: &'static str,
    },
    #[error("Invalid {option}: {reason}", option = .name.key())]
    Invalid {
        name: OptionName,
        reason: &'static str,
    },
}

impl OptionName {
    fn invalid(self, reason: &'static str) -> OptionError {
        OptionError::Invalid { name: self, reason }
    }
}

#[derive(Debug)]
pub struct OptionKey<T> {
    pub name: OptionName,
    pub description: &'static str,
    value_type: PhantomData<fn() -> T>,
}
impl<T> Copy for OptionKey<T> {}
impl<T> Clone for OptionKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> OptionKey<T> {
    pub const fn new(
        component: &'static str,
        option: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            name: OptionName::new(component, option),
            description,
            value_type: PhantomData,
        }
    }
}
impl<T: std::str::FromStr> OptionKey<T> {
    pub fn parse(self, text: &str) -> Result<T, OptionError> {
        text.parse()
            .map_err(|_| self.name.invalid("Invalid component option value."))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Setting {
    String(OptionKey<String>),
    Secret(OptionKey<Secret>),
}
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Value {
    String(String),
}

mod sealed {
    pub trait Sealed {}
}
pub trait OptionType: sealed::Sealed + Sized {
    fn value(self) -> Value;
    fn read(value: &Value) -> Option<&Self>;
    fn matches(setting: Setting) -> bool;
}
impl sealed::Sealed for String {}
impl OptionType for String {
    fn value(self) -> Value {
        Value::String(self)
    }
    fn read(value: &Value) -> Option<&Self> {
        let Value::String(value) = value;
        Some(value)
    }
    fn matches(setting: Setting) -> bool {
        matches!(setting, Setting::String(_))
    }
}
impl OptionKey<String> {
    pub const fn setting(self) -> Setting {
        Setting::String(self)
    }
}
impl OptionKey<Secret> {
    pub const fn setting(self) -> Setting {
        Setting::Secret(self)
    }
}

impl Setting {
    pub fn name(self) -> OptionName {
        match self {
            Self::String(key) => key.name,
            Self::Secret(key) => key.name,
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::String(key) => key.description,
            Self::Secret(key) => key.description,
        }
    }
    pub fn secret(self) -> Option<OptionKey<Secret>> {
        if let Self::Secret(key) = self {
            Some(key)
        } else {
            None
        }
    }
    pub fn parse(self, text: &str) -> Result<Value, OptionError> {
        match self {
            Self::String(key) => key.parse(text).map(Value::String),
            Self::Secret(key) => Err(key
                .name
                .invalid("Credentials cannot be stored in ordinary options.")),
        }
    }
    pub fn check(self, value: &Value) -> Result<(), OptionError> {
        match (self, value) {
            (Self::String(_), Value::String(_)) => Ok(()),
            _ => Err(self.name().invalid("Wrong component option type.")),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options(BTreeMap<OptionName, Value>);
impl Options {
    pub fn get<T: OptionType>(&self, key: OptionKey<T>) -> Result<Option<&T>, OptionError> {
        self.0
            .get(&key.name)
            .map(|value| {
                T::read(value).ok_or_else(|| key.name.invalid("Wrong component option type."))
            })
            .transpose()
    }
    pub fn set<T: OptionType>(&mut self, key: OptionKey<T>, value: T) {
        self.0.insert(key.name, value.value());
    }
    pub fn insert(&mut self, setting: Setting, value: Value) -> Result<(), OptionError> {
        setting.check(&value)?;
        self.0.insert(setting.name(), value);
        Ok(())
    }
    pub fn clear(&mut self, setting: Setting) {
        self.0.remove(&setting.name());
    }
    pub fn for_component<'a>(&'a self, component: &'a Component) -> ComponentOptions<'a> {
        ComponentOptions {
            component,
            options: self,
        }
    }
}
#[derive(Clone, Copy)]
pub struct ComponentOptions<'a> {
    component: &'a Component,
    options: &'a Options,
}
impl<'a> ComponentOptions<'a> {
    pub fn required<T: OptionType>(self, key: OptionKey<T>) -> Result<&'a T, OptionError> {
        self.get(key)?.ok_or(OptionError::Missing {
            name: key.name,
            description: key.description,
        })
    }

    pub fn get<T: OptionType>(self, key: OptionKey<T>) -> Result<Option<&'a T>, OptionError> {
        if key.name.component != self.component.id
            || !self
                .component
                .setting(key.name.option)
                .is_some_and(T::matches)
        {
            return Err(key
                .name
                .invalid("Option does not belong to this component or has the wrong type."));
        }
        self.options.get(key)
    }
}
