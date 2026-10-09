use std::ffi::OsString;
use yomibu::application::{Credentials, Secret};
use yomibu::configuration::credentials::DEFAULT_CREDENTIAL_BINDINGS;

struct Provider(&'static str);
impl Provider {
    fn cli(&self) -> String {
        format!("{}-api-key", self.0)
    }
    fn environment(&self) -> String {
        format!("YOMIBU_{}_API_KEY", self.0.to_ascii_uppercase())
    }
    fn set(&self, credentials: &mut Credentials, value: Secret, cli: bool) {
        if cli {
            credentials.set_shared_cli(self.0, value);
        } else {
            credentials.set_shared_environment(self.0, value);
        }
    }
}
fn providers() -> impl Iterator<Item = Provider> {
    let shared: std::collections::BTreeSet<_> = DEFAULT_CREDENTIAL_BINDINGS
        .iter()
        .map(|(_, shared)| *shared)
        .collect();
    shared.into_iter().map(Provider)
}
pub(crate) fn arguments() -> impl Iterator<Item = clap::Arg> {
    providers().map(|provider| {
        clap::Arg::new(provider.cli())
            .long(provider.cli())
            .global(true)
            .value_name("KEY")
            .help(format!(
                "Secret credential; env: {}",
                provider.environment()
            ))
    })
}

// Clap only sees placeholders, including when it formats conflicts and usage.
pub(crate) fn capture(
    args: impl IntoIterator<Item = impl Into<OsString>>,
) -> (Vec<OsString>, Credentials) {
    let mut args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let mut credentials = Credentials::default();
    let providers: Vec<_> = providers().collect();
    let mut index = 1;
    while index < args.len() {
        let text = args[index].to_string_lossy();
        if text == "--" {
            break;
        }
        if let Some(provider) = providers.iter().find(|provider| {
            text == format!("--{}", provider.cli())
                || text.starts_with(&format!("--{}=", provider.cli()))
        }) {
            if let Some((_, value)) = text.split_once('=') {
                let secret = if args[index].to_str().is_some() {
                    Secret::new(value.to_string())
                } else {
                    Secret::invalid_encoding()
                };
                provider.set(&mut credentials, secret, true);
                args[index] = format!("--{}=[REDACTED]", provider.cli()).into();
            } else if index + 1 < args.len() {
                let value = args[index + 1].to_string_lossy();
                if !value.starts_with('-') {
                    let secret = if args[index + 1].to_str().is_some() {
                        Secret::new(value.to_string())
                    } else {
                        Secret::invalid_encoding()
                    };
                    provider.set(&mut credentials, secret, true);
                    args[index + 1] = "[REDACTED]".into();
                    index += 1;
                } else if !matches!(value.as_ref(), "--help" | "-h" | "--version" | "-V")
                    && !providers
                        .iter()
                        .any(|provider| value == format!("--{}", provider.cli()))
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
    for provider in providers() {
        if let Some(value) = std::env::var_os(provider.environment()) {
            let value = value
                .into_string()
                .map(Secret::new)
                .unwrap_or_else(|_| Secret::invalid_encoding());
            provider.set(credentials, value, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yomibu::configuration::{components, credentials::CredentialBindings};

    #[test]
    fn provider_flags_supply_both_openai_uses_and_keep_wanikani_separate() {
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
        let bindings = CredentialBindings::default();
        for requirement in [components::GENERATION_KEY, components::EMBEDDING_KEY] {
            assert_eq!(
                credentials.resolve(requirement, &bindings).unwrap(),
                Some("synthetic-openai")
            );
        }
        assert_eq!(
            credentials
                .resolve(components::SOURCE_KEY, &bindings)
                .unwrap(),
            Some("synthetic-wanikani")
        );
    }
}
