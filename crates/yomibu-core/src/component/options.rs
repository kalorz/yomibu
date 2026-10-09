use super::{Component, credentials::Secret};
use serde::Deserialize;
use std::collections::BTreeMap;

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

type Validator<T> = fn(&T) -> Result<(), &'static str>;

#[derive(Debug)]
pub struct OptionKey<T> {
    pub name: OptionName,
    pub description: &'static str,
    default: Option<&'static str>,
    validator: Option<Validator<T>>,
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
            default: None,
            validator: None,
        }
    }
    pub const fn validate(mut self, validator: Validator<T>) -> Self {
        self.validator = Some(validator);
        self
    }
    fn check(self, value: &T) -> Result<(), OptionError> {
        self.validator
            .map_or(Ok(()), |validate| validate(value))
            .map_err(|reason| self.name.invalid(reason))
    }
}
impl<T: OptionType> OptionKey<T> {
    /// ```compile_fail
    /// use yomibu_core::component::{credentials::Secret, options::OptionKey};
    /// let key = OptionKey::<Secret>::new("test", "api-key", "API key");
    /// key.default("secret");
    /// ```
    pub const fn default(mut self, value: &'static str) -> Self {
        self.default = Some(value);
        self
    }
}
impl<T: std::str::FromStr> OptionKey<T> {
    pub fn parse(self, text: &str) -> Result<T, OptionError> {
        let value = text
            .parse()
            .map_err(|_| self.name.invalid("Invalid component option value."))?;
        self.check(&value)?;
        Ok(value)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Setting {
    String(OptionKey<String>),
    Integer(OptionKey<usize>),
    Secret(OptionKey<Secret>),
}
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Value {
    String(String),
    Integer(usize),
}

mod sealed {
    pub trait Sealed {}
}
pub trait OptionType: sealed::Sealed + Sized {
    fn value(self) -> Value;
    fn read(value: &Value) -> Option<&Self>;
    fn matches(setting: Setting) -> bool;
}
macro_rules! option_type {
    ($type:ty, $variant:ident) => {
        impl sealed::Sealed for $type {}
        impl OptionType for $type {
            fn value(self) -> Value {
                Value::$variant(self)
            }
            fn read(value: &Value) -> Option<&Self> {
                if let Value::$variant(value) = value {
                    Some(value)
                } else {
                    None
                }
            }
            fn matches(setting: Setting) -> bool {
                matches!(setting, Setting::$variant(_))
            }
        }
        impl OptionKey<$type> {
            pub const fn setting(self) -> Setting {
                Setting::$variant(self)
            }
        }
    };
}
option_type!(String, String);
option_type!(usize, Integer);
impl OptionKey<Secret> {
    pub const fn setting(self) -> Setting {
        Setting::Secret(self)
    }
}

impl Setting {
    pub fn name(self) -> OptionName {
        match self {
            Self::String(key) => key.name,
            Self::Integer(key) => key.name,
            Self::Secret(key) => key.name,
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::String(key) => key.description,
            Self::Integer(key) => key.description,
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
            Self::Integer(key) => key.parse(text).map(Value::Integer),
            Self::Secret(key) => Err(key
                .name
                .invalid("Credentials cannot be stored in ordinary options.")),
        }
    }
    pub fn check(self, value: &Value) -> Result<(), OptionError> {
        match (self, value) {
            (Self::String(key), Value::String(value)) => key.check(value),
            (Self::Integer(key), Value::Integer(value)) => key.check(value),
            _ => Err(self.name().invalid("Wrong component option type.")),
        }
    }
    pub fn default_value(self) -> Result<Option<Value>, OptionError> {
        let text = match self {
            Self::String(key) => key.default,
            Self::Integer(key) => key.default,
            Self::Secret(_) => None,
        };
        text.map(|text| self.parse(text)).transpose()
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
    pub fn set<T: OptionType>(&mut self, key: OptionKey<T>, value: T) -> Result<(), OptionError> {
        key.check(&value)?;
        self.0.insert(key.name, value.value());
        Ok(())
    }
    pub fn insert(&mut self, setting: Setting, value: Value) -> Result<(), OptionError> {
        setting.check(&value)?;
        self.0.insert(setting.name(), value);
        Ok(())
    }
    pub fn clear(&mut self, setting: Setting) -> Result<(), OptionError> {
        self.0.remove(&setting.name());
        if let Some(value) = setting.default_value()? {
            self.insert(setting, value)?;
        }
        Ok(())
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
