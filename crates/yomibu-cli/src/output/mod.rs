//! Terminal-safe rendering; shared serializable report data belongs to the library.
use anyhow::anyhow;
use clap::error::{ContextKind, ContextValue};
use yomibu::{application::SetupIssue, configuration::components::credential_guidance};
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
    if let Some(yomibu::application::ApplicationError::Setup { issues }) = error.downcast_ref() {
        let mut message = String::from("Missing required setup:\n");
        for issue in issues {
            let (name, guidance) = match issue {
                SetupIssue::Inventory => ("Learner vocabulary".into(), INVENTORY_GUIDANCE.into()),
                SetupIssue::Credential(key) => (key.name.key(), credential_guidance(*key)),
            };
            message.push_str(&format!("  {name}:\n    {guidance}\n"));
        }
        return anyhow!("{}", message.trim_end());
    }
    anyhow!("{}", format!("{error:#}").escape_debug())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yomibu::{application::ApplicationError, configuration::components::OptionKey};

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
