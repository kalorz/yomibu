use std::ffi::OsString;
use yomibu::application::{Credentials, Secret};
use yomibu::configuration::components::CREDENTIALS;

pub(crate) fn arguments() -> impl Iterator<Item = clap::Arg> {
    CREDENTIALS.iter().map(|requirement| {
        clap::Arg::new(requirement.name.cli())
            .long(requirement.name.cli())
            .global(true)
            .help_heading("Credentials")
            .value_name("KEY")
            .help(format!(
                "Secret credential; env: {}",
                requirement.name.environment()
            ))
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
        if let Some(requirement) = CREDENTIALS.iter().find(|requirement| {
            text == format!("--{}", requirement.name.cli())
                || text.starts_with(&format!("--{}=", requirement.name.cli()))
        }) {
            if let Some((_, value)) = text.split_once('=') {
                let secret = if args[index].to_str().is_some() {
                    Secret::new(value.to_string())
                } else {
                    Secret::invalid_encoding()
                };
                credentials.set_cli(*requirement, secret);
                args[index] = format!("--{}=[REDACTED]", requirement.name.cli()).into();
            } else if index + 1 < args.len() {
                let value = args[index + 1].to_string_lossy();
                if !value.starts_with('-') {
                    let secret = if args[index + 1].to_str().is_some() {
                        Secret::new(value.to_string())
                    } else {
                        Secret::invalid_encoding()
                    };
                    credentials.set_cli(*requirement, secret);
                    args[index + 1] = "[REDACTED]".into();
                    index += 1;
                } else if !matches!(value.as_ref(), "--help" | "-h" | "--version" | "-V")
                    && !CREDENTIALS
                        .iter()
                        .any(|requirement| value == format!("--{}", requirement.name.cli()))
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
    for requirement in CREDENTIALS {
        if let Some(value) = std::env::var_os(requirement.name.environment()) {
            let value = value
                .into_string()
                .map(Secret::new)
                .unwrap_or_else(|_| Secret::invalid_encoding());
            credentials.set_environment(*requirement, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yomibu::configuration::components;

    #[test]
    fn component_flags_do_not_supply_other_components() {
        let (args, credentials) = capture([
            "yomibu",
            "--openai-api-key",
            "synthetic-openai",
            "--wanikani-api-key=synthetic-wanikani",
        ]);
        assert_eq!(
            args,
            [
                "yomibu",
                "--openai-api-key",
                "[REDACTED]",
                "--wanikani-api-key=[REDACTED]"
            ]
            .map(OsString::from)
        );
        assert_eq!(
            credentials.resolve(components::GENERATION_KEY).unwrap(),
            Some("synthetic-openai")
        );
        assert!(
            credentials
                .resolve(components::EMBEDDING_KEY)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            credentials.resolve(components::SOURCE_KEY).unwrap(),
            Some("synthetic-wanikani")
        );
    }
}
