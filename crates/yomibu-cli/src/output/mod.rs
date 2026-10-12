//! Terminal-safe rendering; shared serializable report data belongs to the library.
use anyhow::anyhow;
use clap::error::{ContextKind, ContextValue};
use yomibu::{application::SetupIssue, configuration::components::credential_guidance};
pub(crate) mod candidate;
pub(crate) mod status;
pub(crate) mod story;

pub(crate) const INVENTORY_GUIDANCE: &str =
    "Supply --inventory PATH or a usable cache; use --sync true with a WaniKani key to refresh it.";

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
            message.push_str(&format!(
                "  {}:\n    {}\n",
                name.escape_debug(),
                guidance.escape_debug()
            ));
        }
        return anyhow!("{}", message.trim_end());
    }
    anyhow!("{}", format!("{error:#}").escape_debug())
}
