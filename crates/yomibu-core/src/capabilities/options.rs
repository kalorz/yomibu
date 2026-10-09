use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug)]
pub struct OptionDeclaration<T> {
    pub name: OptionName,
    marker: PhantomData<fn() -> T>,
}

impl<T> OptionDeclaration<T> {
    pub const fn new(component: &'static str, option: &'static str) -> Self {
        Self {
            name: OptionName::new(component, option),
            marker: PhantomData,
        }
    }
}

impl<T: std::str::FromStr> OptionDeclaration<T> {
    pub fn parse(&self, value: &str) -> Result<T, &'static str> {
        value.parse().map_err(|_| "Invalid component option value.")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CredentialRequirement {
    pub name: OptionName,
}

impl CredentialRequirement {
    pub const fn new(component: &'static str, option: &'static str) -> Self {
        Self {
            name: OptionName::new(component, option),
        }
    }
}
