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
    required: bool,
    requirements: Vec<CredentialRequirement>,
}

trait Store {
    fn read(&self, key: &str) -> Result<Option<Secret>, CredentialError>;
    fn write(&self, key: &str, value: &Secret) -> Result<()>;
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
        required: requirements
            .iter()
            .any(|requirement| requirement.name == components::GENERATION_KEY.name),
        requirements,
    })
}

fn missing_inputs(config: &Configuration, credentials: &Credentials) -> Result<Vec<Need>> {
    let bindings = &config.application.credential_bindings;
    let mut needs = BTreeMap::<String, Need>::new();
    for (requirement, required, enabled) in [
        (components::GENERATION_KEY, true, true),
        (components::SOURCE_KEY, false, config.application.sync),
        (
            components::EMBEDDING_KEY,
            false,
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
        let need = needs.entry(binding.key.clone()).or_insert_with(|| Need {
            key: binding.key.clone(),
            required: false,
            requirements: Vec::new(),
        });
        need.required |= required;
        need.requirements.push(requirement);
    }
    Ok(needs.into_values().collect())
}

fn setup(
    needs: Vec<Need>,
    store: &impl Store,
    mut prompt: impl FnMut(&Need) -> Result<Option<Secret>>,
    out: &mut impl Write,
) -> Result<()> {
    let mut missing = false;
    for need in needs {
        if let Some(value) = store.read(&need.key)?
            && !value.expose()?.trim().is_empty()
        {
            continue;
        }
        missing = true;
        save(&need, store, &mut prompt, out)?;
    }
    if !missing {
        writeln!(out, "No missing credentials to save.")?;
    }
    Ok(())
}

fn save(
    need: &Need,
    store: &impl Store,
    mut prompt: impl FnMut(&Need) -> Result<Option<Secret>>,
    out: &mut impl Write,
) -> Result<()> {
    match prompt(need)? {
        Some(value) => {
            let token = value.expose()?;
            if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_graphic()) {
                bail!("API key must be nonblank printable ASCII without spaces.");
            }
            store.write(&need.key, &value)?;
            writeln!(out, "Saved {} in macOS Keychain.", need.key)?;
        }
        None if need.required => writeln!(
            out,
            "Skipped {}; story generation still needs a credential.",
            need.key
        )?,
        None => writeln!(out, "Skipped {}.", need.key)?,
    }
    Ok(())
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

fn prompt(need: &Need) -> Result<Option<Secret>> {
    let kind = if need.required {
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
        Ok(if token.is_empty() {
            None
        } else {
            Some(token.into())
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        bail!("Credential setup requires macOS Keychain.")
    }
}

pub(super) fn configure(config: &Configuration, credentials: &Credentials) -> Result<()> {
    setup(
        missing_inputs(config, credentials)?,
        &Keychain,
        prompt,
        &mut io::stdout().lock(),
    )
}

pub(super) fn configure_target(key: &str) -> Result<()> {
    save(&target(key)?, &Keychain, prompt, &mut io::stdout().lock())
}

pub(super) fn install(credentials: &mut Credentials) {
    if cfg!(target_os = "macos") {
        for key in slots() {
            let slot = key.clone();
            credentials.supply_with(&key, move || Keychain.read(&slot));
        }
    }
}

fn identity(key: &str) -> (String, &'static str) {
    (
        format!("yomibu:{}", key.strip_suffix(".api-key").unwrap_or(key)),
        "api-key",
    )
}

struct Keychain;
impl Store for Keychain {
    fn read(&self, key: &str) -> Result<Option<Secret>, CredentialError> {
        let (service, account) = identity(key);
        super::keychain::read(&service, account)
            .map(|value| value.map(Secret::from))
            .map_err(|_| CredentialError::StoreUnavailable)
    }
    fn write(&self, key: &str, value: &Secret) -> Result<()> {
        let (service, account) = identity(key);
        super::keychain::write(&service, account, value.expose()?)
    }
}

#[cfg(test)]
mod tests;
