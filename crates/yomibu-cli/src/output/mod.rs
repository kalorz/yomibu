//! Terminal-safe rendering; shared serializable report data belongs to the library.
use crate::composition::{
    ApplicationError, SetupIssue,
    configuration::components::{SOURCE_KEY, credential_guidance},
    options::environment_name,
};
use anyhow::anyhow;
use clap::error::{ContextKind, ContextValue};
use yomibu_core::{
    domain::{generation::StoryError, knowledge::inventory::InventoryError},
    ports::infrastructure::credentials::CredentialError,
};
use yomibu_ext::{
    file_store::cache::{CacheError, WriteError},
    wanikani,
};
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
    let application_error = error.downcast_ref::<ApplicationError>();
    let cache_error = match application_error {
        Some(ApplicationError::Persistence(WriteError::ExistingCache(error))) => Some(error),
        _ => error.downcast_ref::<CacheError>(),
    };
    if let Some(message) = cache_error.and_then(cache_error_message) {
        return anyhow!("{}", message.escape_debug());
    }
    match application_error {
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
        Some(ApplicationError::Credential(error)) => anyhow!("{}", credential_error(error)),
        Some(ApplicationError::Story(StoryError::Inventory(InventoryError::AccessExpired))) => {
            anyhow!("Recorded content access has expired; run yomibu sync to refresh access.")
        }
        Some(ApplicationError::Persistence(WriteError::AccountMismatch)) => {
            anyhow!(
                "This cache belongs to a different WaniKani account; use another --data-dir PATH."
            )
        }
        Some(ApplicationError::Persistence(WriteError::Locked)) => {
            anyhow!(
                "Another sync holds the lock; wait for it to finish or use another --data-dir PATH."
            )
        }
        Some(ApplicationError::Source(wanikani::Error::RateLimitWait)) => {
            anyhow!("WaniKani rate-limit reset exceeds 120 seconds; retry sync later.")
        }
        _ => anyhow!("{}", format!("{error:#}").escape_debug()),
    }
}

fn cache_error_message(error: &CacheError) -> Option<String> {
    let guidance = match error {
        CacheError::Missing { .. } => format!(
            "run yomibu sync with {} set, or select an existing cache with --data-dir PATH.",
            environment_name(SOURCE_KEY.name)
        ),
        CacheError::Invalid { .. } | CacheError::Corrupt { .. } => {
            "preserve the file and restore a valid backup or select another --data-dir PATH.".into()
        }
        CacheError::UnsupportedVersion { .. } => {
            "upgrade Yomibu or select a supported cache with --data-dir PATH.".into()
        }
        _ => return None,
    };
    let failure = error.to_string();
    let mut message = format!(
        "{}; {guidance}",
        failure.strip_suffix('.').unwrap_or(&failure)
    );
    for cause in anyhow::Chain::new(error).skip(1) {
        message.push_str(&format!(": {cause}"));
    }
    Some(message)
}

pub(crate) fn credential_error(error: &CredentialError) -> &'static str {
    match error {
        CredentialError::StoreUnavailable => {
            "Cannot access the credential store; unlock it or supply a CLI/environment credential."
        }
        CredentialError::InvalidEncoding => "Invalid credential input; supply a UTF-8 credential.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{ApplicationError, configuration::components::OptionKey};

    #[test]
    fn cache_guidance_preserves_missing_errors_and_hostile_cause_chains_on_both_routes() {
        for wrapped in [false, true] {
            let path = std::path::PathBuf::from("日本語\n\u{1b}/wanikani.json");
            let source = serde_json::Error::io(std::io::Error::other(
                anyhow!("underlying\n\u{1b}").context("日本語\n\u{1b}"),
            ));
            for (error, diagnostic) in [
                (
                    CacheError::Missing { path: path.clone() },
                    format!(
                        "No cache at {}; run yomibu sync with YOMIBU_WANIKANI_API_KEY set, or select an existing cache with --data-dir PATH.",
                        path.display()
                    ),
                ),
                (
                    CacheError::Corrupt {
                        path: path.clone(),
                        source,
                    },
                    format!(
                        "Corrupt cache at {}; preserve the file and restore a valid backup or select another --data-dir PATH.: 日本語\n\u{1b}: underlying\n\u{1b}",
                        path.display()
                    ),
                ),
            ] {
                let error = if wrapped {
                    ApplicationError::Persistence(WriteError::ExistingCache(error)).into()
                } else {
                    error.into()
                };
                assert_eq!(
                    command_error(error).to_string(),
                    diagnostic.escape_debug().to_string(),
                    "wrapped={wrapped}"
                );
            }
        }
    }

    #[test]
    fn credential_errors_keep_cli_remedies_through_the_command_error_wrapper() {
        for (error, expected) in [
            (
                CredentialError::StoreUnavailable,
                "Cannot access the credential store; unlock it or supply a CLI/environment credential.",
            ),
            (
                CredentialError::InvalidEncoding,
                "Invalid credential input; supply a UTF-8 credential.",
            ),
        ] {
            assert_eq!(
                command_error(ApplicationError::Credential(error).into()).to_string(),
                expected
            );
        }
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
