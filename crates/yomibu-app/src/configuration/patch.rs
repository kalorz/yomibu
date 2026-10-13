use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Patch<T> {
    #[default]
    Inherit,
    Set(T),
    Clear,
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Clear {
            clear: bool,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value<T> {
            Clear(Clear),
            Set(T),
        }
        match Value::deserialize(deserializer)? {
            Value::Clear(Clear { clear: true }) => Ok(Self::Clear),
            Value::Clear(_) => Err(serde::de::Error::custom("clear must be true")),
            Value::Set(value) => Ok(Self::Set(value)),
        }
    }
}
impl<T> Patch<T> {
    pub fn apply(self, value: &mut Option<T>) {
        match self {
            Self::Inherit => {}
            Self::Clear => *value = None,
            Self::Set(new) => *value = Some(new),
        }
    }
}
impl<T> From<Option<T>> for Patch<T> {
    fn from(value: Option<T>) -> Self {
        value.map(Self::Set).unwrap_or_default()
    }
}
