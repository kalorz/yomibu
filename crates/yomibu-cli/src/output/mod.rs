//! Terminal-safe rendering; shared serializable report data belongs to the library.
use crate::composition::{
    ApplicationError, SetupIssue, configuration::components::credential_guidance,
};
use anyhow::anyhow;
use clap::error::{ContextKind, ContextValue};
use yomibu_core::extension::options::OptionError;
pub(crate) mod candidate;
pub(crate) mod status;
pub(crate) mod story;

pub(crate) const INVENTORY_GUIDANCE: &str = "Run yomibu sync to load learner vocabulary before generating a story. Use the same --data-dir for sync and story.";

pub(crate) fn escape_argument_error(mut error: clap::Error) -> clap::Error {
    // Escape supplied values before Clap adds its diagnostic layout.
    for kind in [
        ContextKind::InvalidArg,
        ContextKind::InvalidValue,
        ContextKind::InvalidSubcommand,
    ] {
        if let Some(ContextValue::String(value)) = error.get(kind) {
            let escaped = value.escape_debug().to_string();
            error.insert(kind, ContextValue::String(escaped));
        }
    }
    error
}

pub(crate) fn command_error(error: anyhow::Error) -> anyhow::Error {
    match error.downcast_ref::<ApplicationError>() {
        Some(ApplicationError::Setup { issues }) => {
            let mut message = String::from("Missing required setup:\n");
            for issue in issues {
                let (name, guidance) = match issue {
                    SetupIssue::Inventory => {
                        ("Learner vocabulary".into(), INVENTORY_GUIDANCE.into())
                    }
                    SetupIssue::Credential(key) => (key.name.key(), credential_guidance(*key)),
                };
                message.push_str(&format!("  {name}:\n    {guidance}\n"));
            }
            anyhow!("{}", message.trim_end())
        }
        Some(ApplicationError::Options(OptionError::Missing { name, description })) => anyhow!(
            "{}",
            format!(
                "Supply --{}. {description}",
                crate::composition::options::cli_name(*name)
            )
            .escape_debug()
        ),
        Some(ApplicationError::Credential(error)) => anyhow!("{}", credential_error(error)),
        _ => anyhow!("{}", format!("{error:#}").escape_debug()),
    }
}

pub(crate) fn credential_error(
    error: &yomibu_core::ports::infrastructure::credentials::CredentialError,
) -> String {
    match error {
        yomibu_core::ports::infrastructure::credentials::CredentialError::StoreUnavailable => {
            "Cannot access the credential store; unlock it or supply a CLI/environment credential."
                .into()
        }
        _ => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{ApplicationError, configuration::components::OptionKey};

    #[test]
    fn core_errors_keep_cli_remedies_through_the_command_error_wrapper() {
        use yomibu_core::{
            extension::options::OptionError, ports::infrastructure::credentials::CredentialError,
        };
        let key: OptionKey<String> = OptionKey::new("example", "model", "Model");
        let missing = ApplicationError::Options(OptionError::Missing {
            name: key.name,
            description: key.description,
        });
        assert_eq!(
            command_error(missing.into()).to_string(),
            "Supply --example-model. Model"
        );
        let unavailable = ApplicationError::Credential(CredentialError::StoreUnavailable);
        assert_eq!(
            command_error(unavailable.into()).to_string(),
            "Cannot access the credential store; unlock it or supply a CLI/environment credential."
        );
    }

    #[test]
    fn setup_guidance_preserves_trusted_punctuation_and_layout() {
        let error = ApplicationError::Setup {
            issues: vec![SetupIssue::Credential(OptionKey::new(
                "example",
                "api-key",
                "Learner's 日本語 key.\n    Use the \"reading\" key.",
            ))],
        };
        assert_eq!(
            command_error(error.into()).to_string(),
            concat!(
                "Missing required setup:\n",
                "  example.api-key:\n",
                "    Learner's 日本語 key.\n",
                "    Use the \"reading\" key. Supply --example-api-key or YOMIBU_EXAMPLE_API_KEY. ",
                "On macOS, run yomibu auth example.",
            )
        );
    }
}
