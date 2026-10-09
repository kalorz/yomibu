use anyhow::{Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, IsTerminal, Write},
};
use yomibu::configuration::components::CredentialRequirement;
use yomibu::{
    application::{CredentialError, Credentials, Secret},
    configuration::{
        Configuration, EmbeddingProvider, components, credentials::DEFAULT_CREDENTIAL_BINDINGS,
    },
};

struct Need {
    key: String,
    requirements: Vec<CredentialRequirement>,
}
impl Need {
    fn required(&self) -> bool {
        self.requirements
            .iter()
            .any(|requirement| requirement.name == components::GENERATION_KEY.name)
    }
}

pub(super) fn slots() -> Vec<String> {
    DEFAULT_CREDENTIAL_BINDINGS
        .iter()
        .flat_map(|(requirement, shared)| [requirement.name.key(), (*shared).into()])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn target(key: &str) -> Result<Need> {
    let requirements: Vec<_> = DEFAULT_CREDENTIAL_BINDINGS
        .iter()
        .filter(|(requirement, shared)| requirement.name.key() == key || *shared == key)
        .map(|(requirement, _)| *requirement)
        .collect();
    if requirements.is_empty() {
        bail!("Unknown credential slot; see yomibu auth --help.");
    }
    Ok(Need {
        key: key.into(),
        requirements,
    })
}

fn missing_inputs(config: &Configuration, credentials: &Credentials) -> Result<Vec<Need>> {
    let bindings = &config.application.credential_bindings;
    let mut needs = BTreeMap::<&str, Vec<CredentialRequirement>>::new();
    for (requirement, enabled) in [
        (components::GENERATION_KEY, true),
        (components::SOURCE_KEY, config.application.sync),
        (
            components::EMBEDDING_KEY,
            config.pipeline.embeddings
                && config.pipeline.embedding_provider == Some(EmbeddingProvider::Openai),
        ),
    ] {
        if !enabled || credentials.resolve(requirement, bindings)?.is_some() {
            continue;
        }
        let Some(binding) = bindings.get(requirement) else {
            continue;
        };
        needs.entry(&binding.key).or_default().push(requirement);
    }
    Ok(needs
        .into_iter()
        .map(|(key, requirements)| Need {
            key: key.into(),
            requirements,
        })
        .collect())
}

#[cfg(any(target_os = "macos", test))]
fn api_key(token: String) -> Result<Option<String>> {
    if token.is_empty() {
        return Ok(None);
    }
    if !token.bytes().all(|byte| byte.is_ascii_graphic()) {
        bail!("API key must be nonblank printable ASCII without spaces.");
    }
    Ok(Some(token))
}

pub(super) fn require_interactive(json: bool) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!(
            "Credential storage is supported only on macOS; use CLI/environment credentials on this platform."
        );
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("Credential setup requires an interactive terminal.");
    }
    if json {
        bail!("Credential setup is interactive; omit --json.");
    }
    Ok(())
}

fn save(need: &Need) -> Result<()> {
    let kind = if need.required() {
        "required for story generation"
    } else {
        "optional"
    };
    eprintln!("{} ({kind})", need.key);
    for requirement in &need.requirements {
        let purpose = if requirement.name == components::SOURCE_KEY.name {
            "WaniKani synchronization: read access, no write permissions"
        } else if requirement.name == components::GENERATION_KEY.name {
            "Story generation: OpenAI response creation and model access"
        } else {
            "Hosted embeddings: OpenAI embedding access"
        };
        eprintln!("  {purpose}");
    }
    #[cfg(target_os = "macos")]
    {
        let token = rpassword::prompt_password("API key (Enter to skip): ")
            .map_err(|_| anyhow::anyhow!("Cannot read API key from the terminal."))?;
        let mut out = io::stdout().lock();
        match api_key(token)? {
            Some(value) => {
                let (service, account) = identity(&need.key);
                super::keychain::write(&service, account, &value)?;
                writeln!(out, "Saved {} in macOS Keychain.", need.key)
            }
            None if need.required() => writeln!(
                out,
                "Skipped {}; story generation still needs a credential.",
                need.key
            ),
            None => writeln!(out, "Skipped {}.", need.key),
        }?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        bail!("Credential setup requires macOS Keychain.")
    }
}

pub(super) fn configure(config: &Configuration, credentials: &Credentials) -> Result<()> {
    let needs = missing_inputs(config, credentials)?;
    if needs.is_empty() {
        writeln!(io::stdout().lock(), "No missing credentials to save.")?;
    }
    for need in needs {
        save(&need)?;
    }
    Ok(())
}

pub(super) fn configure_target(key: &str) -> Result<()> {
    save(&target(key)?)
}

pub(super) fn install(credentials: &mut Credentials) {
    if cfg!(target_os = "macos") {
        for key in slots() {
            let (service, account) = identity(&key);
            credentials.supply_with(&key, move || {
                super::keychain::read(&service, account)
                    .map(|value| value.map(Secret::from))
                    .map_err(|_| CredentialError::StoreUnavailable)
            });
        }
    }
}

fn identity(key: &str) -> (String, &'static str) {
    (
        format!("yomibu:{}", key.strip_suffix(".api-key").unwrap_or(key)),
        "api-key",
    )
}

#[cfg(test)]
mod tests;
