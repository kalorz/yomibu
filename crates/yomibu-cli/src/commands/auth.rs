use anyhow::{Result, bail};
use std::{
    collections::BTreeSet,
    io::{self, IsTerminal, Write},
};
use yomibu::configuration::components::OptionKey;
use yomibu::{
    application::{CredentialError, Credentials, Secret},
    configuration::components,
};

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
                    "warning: {}: {error} Continuing setup.",
                    requirement.name.key()
                )?,
            }
        }
        if credentials.has_override(requirement) {
            writeln!(
                warnings,
                "warning: {}: CLI/environment input overrides the saved Keychain credential. Omit --{} and unset {} to use it.",
                requirement.name.key(),
                requirement.name.cli(),
                requirement.name.environment()
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

fn save(requirement: OptionKey<Secret>) -> Result<()> {
    let required = requirement.name == components::GENERATION_KEY.name;
    let kind = if required {
        "required for story generation"
    } else {
        "optional"
    };
    eprintln!("{} ({kind})", requirement.name.key());
    eprintln!("  {}", requirement.description);
    #[cfg(target_os = "macos")]
    {
        let token = rpassword::prompt_password("API key (Enter to skip): ")
            .map_err(|_| anyhow::anyhow!("Cannot read API key from the terminal."))?;
        let mut out = io::stdout().lock();
        match api_key(token)? {
            Some(value) => {
                let (service, account) = identity(requirement);
                super::keychain::write(&service, account, &value)?;
                writeln!(out, "Saved {} in macOS Keychain.", requirement.name.key())
            }
            None if required => writeln!(
                out,
                "Skipped {}; story generation still needs a credential.",
                requirement.name.key()
            ),
            None => writeln!(out, "Skipped {}.", requirement.name.key()),
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

pub(super) fn install(credentials: &mut Credentials, requirements: &[OptionKey<Secret>]) {
    if cfg!(target_os = "macos") {
        for &requirement in requirements {
            let (service, account) = identity(requirement);
            credentials.supply_with(requirement, move || {
                super::keychain::read(&service, account)
                    .map(|value| value.map(Secret::from))
                    .map_err(|_| CredentialError::StoreUnavailable)
            });
        }
    }
}

fn identity(requirement: OptionKey<Secret>) -> (String, &'static str) {
    (
        format!("yomibu:{}", requirement.name.component),
        requirement.name.option,
    )
}

#[cfg(test)]
mod tests;
