use crate::composition::configuration::components::OptionKey;
#[cfg(any(target_os = "macos", test))]
use crate::composition::credentials::identity;
use crate::composition::{Credentials, Secret, configuration::components};
use anyhow::{Result, bail};
use std::{
    collections::BTreeSet,
    io::{self, IsTerminal, Write},
};
#[cfg(target_os = "macos")]
use yomibu_core::ports::infrastructure::credentials::SecretStore;
#[cfg(target_os = "macos")]
use yomibu_ext::macos_keychain::MacosKeychain;

pub(super) fn targets() -> Vec<String> {
    components::credentials()
        .flat_map(|requirement| [requirement.name.component.into(), requirement.name.key()])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) enum Target {
    Missing(Vec<OptionKey<Secret>>),
    Replace(OptionKey<Secret>),
}

impl Target {
    pub(super) fn requirements(&self) -> &[OptionKey<Secret>] {
        match self {
            Self::Missing(requirements) => requirements,
            Self::Replace(requirement) => std::slice::from_ref(requirement),
        }
    }
}

pub(super) fn select(target: &str) -> Result<Target> {
    if let Some(requirement) =
        components::credentials().find(|requirement| requirement.name.key() == target)
    {
        return Ok(Target::Replace(requirement));
    }
    let requirements: Vec<_> = components::credentials()
        .filter(|requirement| requirement.name.component == target)
        .collect();
    if requirements.is_empty() {
        bail!("Unknown credential target.");
    }
    Ok(Target::Missing(requirements))
}

fn pending(
    target: &Target,
    credentials: &Credentials,
    warnings: &mut impl Write,
) -> Result<Vec<OptionKey<Secret>>> {
    let mut needs = Vec::new();
    for &requirement in target.requirements() {
        if matches!(target, Target::Missing(_)) {
            match credentials.resolve(requirement) {
                Ok(Some(_)) => continue,
                Ok(None) => {}
                Err(error) => writeln!(
                    warnings,
                    "warning: {}: {} Continuing setup.",
                    requirement.name.key(),
                    crate::output::credential_error(&error)
                )?,
            }
        }
        if credentials.has_override(requirement) {
            writeln!(
                warnings,
                "warning: {}: CLI/environment input overrides the saved Keychain credential. Omit --{} and unset {} to use it.",
                requirement.name.key(),
                crate::composition::options::cli_name(requirement.name),
                crate::composition::options::environment_name(requirement.name)
            )?;
        }
        needs.push(requirement);
    }
    Ok(needs)
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

pub(super) fn require_interactive() -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!(
            "Credential storage is supported only on macOS; use CLI/environment credentials on this platform."
        );
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("Credential setup requires an interactive terminal.");
    }
    Ok(())
}

fn save(requirement: OptionKey<Secret>) -> Result<()> {
    let purpose = if requirement.name == components::GENERATION_KEY.name {
        "story generation"
    } else {
        "sync"
    };
    eprintln!("{} (required for {purpose})", requirement.name.key());
    eprintln!("  {}", requirement.description);
    #[cfg(target_os = "macos")]
    {
        let token = rpassword::prompt_password("API key (Enter to skip): ")
            .map_err(|_| anyhow::anyhow!("Cannot read API key from the terminal."))?;
        let mut out = io::stdout().lock();
        match api_key(token)? {
            Some(value) => {
                let (service, account) = identity(requirement);
                MacosKeychain
                    .write(&service, account, &value.into())
                    .map_err(|_| {
                        anyhow::anyhow!(
                            "Cannot access macOS Keychain; unlock it and check access permissions."
                        )
                    })?;
                writeln!(out, "Saved {} in macOS Keychain.", requirement.name.key())
            }
            None => writeln!(
                out,
                "Skipped {}; {purpose} still needs a credential.",
                requirement.name.key()
            ),
        }?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        bail!("Credential setup requires macOS Keychain.")
    }
}

pub(super) fn configure(target: &Target, credentials: &Credentials) -> Result<()> {
    let needs = pending(target, credentials, &mut io::stderr().lock())?;
    if needs.is_empty() {
        writeln!(io::stdout().lock(), "No missing credentials to save.")?;
    }
    for requirement in needs {
        save(requirement)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
