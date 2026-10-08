use std::ffi::OsString;
use yomibu::configuration::components::CredentialRequirement;
use yomibu::{
    application::{Credentials, Secret},
    configuration::components,
};

pub(crate) enum Input {
    Component(CredentialRequirement),
    Shared(&'static str),
}
impl Input {
    fn cli(&self) -> String {
        match self {
            Self::Component(requirement) => requirement.name.cli(),
            Self::Shared(key) => format!("credential-{key}"),
        }
    }
    fn environment(&self) -> String {
        match self {
            Self::Component(requirement) => requirement.name.environment(),
            Self::Shared(key) => format!("YOMIBU_CREDENTIAL_{}", key.to_ascii_uppercase()),
        }
    }
    fn set(&self, credentials: &mut Credentials, value: Secret, cli: bool) {
        match (self, cli) {
            (Self::Component(requirement), true) => credentials.set_cli(*requirement, value),
            (Self::Component(requirement), false) => {
                credentials.set_environment(*requirement, value)
            }
            (Self::Shared(key), true) => credentials.set_shared_cli(key, value),
            (Self::Shared(key), false) => credentials.set_shared_environment(key, value),
        }
    }
}
fn inputs() -> [Input; 5] {
    [
        Input::Component(components::SOURCE_KEY),
        Input::Component(components::GENERATION_KEY),
        Input::Component(components::EMBEDDING_KEY),
        Input::Shared("wanikani"),
        Input::Shared("openai"),
    ]
}
pub(crate) fn arguments() -> impl Iterator<Item = clap::Arg> {
    inputs().into_iter().map(|input| {
        clap::Arg::new(input.cli())
            .long(input.cli())
            .global(true)
            .value_name("KEY")
            .value_parser(|value: &str| {
                if value == "[INVALID UTF8]" {
                    Err("Credential must be valid UTF-8.")
                } else {
                    Ok(())
                }
            })
            .help(format!("Secret credential; env: {}", input.environment()))
    })
}

// Clap only sees placeholders, including when it formats conflicts and usage.
pub(crate) fn capture(
    args: impl IntoIterator<Item = impl Into<OsString>>,
) -> (Vec<OsString>, Credentials) {
    let mut args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let mut credentials = Credentials::default();
    let mut index = 1;
    while index < args.len() {
        let text = args[index].to_string_lossy();
        if text == "--" {
            break;
        }
        if let Some(input) = inputs().into_iter().find(|input| {
            text == format!("--{}", input.cli()) || text.starts_with(&format!("--{}=", input.cli()))
        }) {
            if let Some((_, value)) = text.split_once('=') {
                let placeholder = if args[index].to_str().is_some() {
                    input.set(&mut credentials, Secret::new(value.to_string()), true);
                    "[REDACTED]"
                } else {
                    "[INVALID UTF8]"
                };
                args[index] = format!("--{}={placeholder}", input.cli()).into();
            } else if index + 1 < args.len() {
                let value = args[index + 1].to_string_lossy();
                if !value.starts_with('-') {
                    let placeholder = if args[index + 1].to_str().is_some() {
                        input.set(&mut credentials, Secret::new(value.to_string()), true);
                        "[REDACTED]"
                    } else {
                        "[INVALID UTF8]"
                    };
                    args[index + 1] = placeholder.into();
                    index += 1;
                } else if !matches!(value.as_ref(), "--help" | "-h" | "--version" | "-V")
                    && !inputs()
                        .iter()
                        .any(|input| value == format!("--{}", input.cli()))
                {
                    // Keep a missing-value diagnostic without exposing a dash-prefixed key.
                    args[index + 1] = "--REDACTED".into();
                }
            }
        }
        index += 1;
    }
    (args, credentials)
}
pub(crate) fn environment(credentials: &mut Credentials) {
    for input in inputs() {
        if let Some(value) = std::env::var_os(input.environment()) {
            let value = value
                .into_string()
                .map(Secret::new)
                .unwrap_or_else(|_| Secret::invalid_encoding());
            input.set(credentials, value, false);
        }
    }
}
