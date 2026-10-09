use std::ffi::OsString;
use yomibu::application::{Credentials, Secret};
use yomibu::configuration::{
    components::CredentialRequirement, credentials::DEFAULT_CREDENTIAL_BINDINGS,
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
fn inputs() -> impl Iterator<Item = Input> {
    let shared: std::collections::BTreeSet<_> = DEFAULT_CREDENTIAL_BINDINGS
        .iter()
        .map(|(_, shared)| *shared)
        .collect();
    DEFAULT_CREDENTIAL_BINDINGS
        .iter()
        .map(|(requirement, _)| Input::Component(*requirement))
        .chain(shared.into_iter().map(Input::Shared))
}
pub(crate) fn arguments() -> impl Iterator<Item = clap::Arg> {
    inputs().map(|input| {
        clap::Arg::new(input.cli())
            .long(input.cli())
            .global(true)
            .value_name("KEY")
            .help(format!("Secret credential; env: {}", input.environment()))
    })
}

// Clap only sees placeholders, including when it formats conflicts and usage.
pub(crate) fn capture(
    args: impl IntoIterator<Item = impl Into<OsString>>,
) -> (Vec<OsString>, Credentials) {
    let mut args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let mut credentials = Credentials::default();
    let inputs: Vec<_> = inputs().collect();
    let mut index = 1;
    while index < args.len() {
        let text = args[index].to_string_lossy();
        if text == "--" {
            break;
        }
        if let Some(input) = inputs.iter().find(|input| {
            text == format!("--{}", input.cli()) || text.starts_with(&format!("--{}=", input.cli()))
        }) {
            if let Some((_, value)) = text.split_once('=') {
                let secret = if args[index].to_str().is_some() {
                    Secret::new(value.to_string())
                } else {
                    Secret::invalid_encoding()
                };
                input.set(&mut credentials, secret, true);
                args[index] = format!("--{}=[REDACTED]", input.cli()).into();
            } else if index + 1 < args.len() {
                let value = args[index + 1].to_string_lossy();
                if !value.starts_with('-') {
                    let secret = if args[index + 1].to_str().is_some() {
                        Secret::new(value.to_string())
                    } else {
                        Secret::invalid_encoding()
                    };
                    input.set(&mut credentials, secret, true);
                    args[index + 1] = "[REDACTED]".into();
                    index += 1;
                } else if !matches!(value.as_ref(), "--help" | "-h" | "--version" | "-V")
                    && !inputs
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
