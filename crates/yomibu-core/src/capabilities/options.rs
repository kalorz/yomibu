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
    pub const fn default(mut self, value: &'static str) -> Self {
        self.default = Some(value);
        self
    }
    fn check(self, value: &T) -> Result<(), &'static str> {
        self.validator.map_or(Ok(()), |validate| validate(value))
    }
}
impl<T: std::str::FromStr> OptionKey<T> {
    pub fn parse(self, text: &str) -> Result<T, &'static str> {
        let value = text
            .parse()
            .map_err(|_| "Invalid component option value.")?;
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
    #[serde(skip)]
    Secret(Secret),
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
option_type!(Secret, Secret);

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
    pub fn parse(self, text: &str) -> Result<Value, &'static str> {
        match self {
            Self::String(key) => key.parse(text).map(Value::String),
            Self::Integer(key) => key.parse(text).map(Value::Integer),
            Self::Secret(key) => key.parse(text).map(Value::Secret),
        }
    }
    pub fn check(self, value: &Value) -> Result<(), &'static str> {
        match (self, value) {
            (Self::String(key), Value::String(value)) => key.check(value),
            (Self::Integer(key), Value::Integer(value)) => key.check(value),
            (Self::Secret(key), Value::Secret(value)) => key.check(value),
            _ => Err("Wrong component option type."),
        }
    }
    pub fn default_value(self) -> Result<Option<Value>, &'static str> {
        let text = match self {
            Self::String(key) => key.default,
            Self::Integer(key) => key.default,
            Self::Secret(key) => key.default,
        };
        if self.secret().is_some() && text.is_some() {
            return Err("Credentials cannot have defaults.");
        }
        text.map(|text| self.parse(text)).transpose()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Component {
    pub id: &'static str,
    pub settings: &'static [Setting],
}
impl Component {
    pub fn setting(self, name: &str) -> Option<Setting> {
        self.settings
            .iter()
            .copied()
            .find(|setting| setting.name().option == name)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options(BTreeMap<OptionName, Value>);
impl Options {
    pub fn get<T: OptionType>(&self, key: OptionKey<T>) -> Result<Option<&T>, &'static str> {
        self.0
            .get(&key.name)
            .map(|value| T::read(value).ok_or("Wrong component option type."))
            .transpose()
    }
    pub fn set<T: OptionType>(&mut self, key: OptionKey<T>, value: T) -> Result<(), &'static str> {
        key.check(&value)?;
        self.0.insert(key.name, value.value());
        Ok(())
    }
    pub fn insert(&mut self, setting: Setting, value: Value) -> Result<(), &'static str> {
        setting.check(&value)?;
        self.0.insert(setting.name(), value);
        Ok(())
    }
    pub fn clear(&mut self, setting: Setting) -> Result<(), &'static str> {
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
    pub fn get<T: OptionType>(self, key: OptionKey<T>) -> Result<Option<&'a T>, &'static str> {
        if key.name.component != self.component.id
            || !self
                .component
                .setting(key.name.option)
                .is_some_and(T::matches)
        {
            return Err("Option does not belong to this component or has the wrong type.");
        }
        self.options.get(key)
    }
}

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

impl std::str::FromStr for Secret {
    type Err = std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(value.into())
    }
}
