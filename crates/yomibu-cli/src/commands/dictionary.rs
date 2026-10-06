//! Dictionary selection and explicitly requested offline import/verification.
use crate::args::{DictionaryArgs, DictionaryCommand};
use anyhow::{Context, Result, anyhow};
use std::{io::Write, path::PathBuf};
use yomibu::adapters::{
    dictionary::{ManagedInstallation, import_bundle, verify},
    sudachi::SudachiAnalyzer,
};

impl DictionaryArgs {
    pub(crate) fn load(&self) -> Result<SudachiAnalyzer> {
        if let Some(path) = &self.dictionary {
            return SudachiAnalyzer::load(path).context("External dictionary");
        }
        let root = resolve_root(self.dictionary_dir.as_ref())?;
        let installation =
            ManagedInstallation::open(&root).context("Managed dictionary installation")?;
        // CLI managed selection opts into the documented store contract: import
        // only publishes new fully verified generations and never edits old ones.
        // External writes/truncation are unsupported; arbitrary --dictionary
        // paths use owned snapshots. Checks/permissions cannot prove immutability.
        unsafe { SudachiAnalyzer::load_managed(installation) }.context("Managed dictionary mapping")
    }
}

fn resolve_root(explicit: Option<&PathBuf>) -> Result<PathBuf> {
    explicit
        .cloned()
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(|home| PathBuf::from(home).join(".yomibu/dictionaries"))
        })
        .ok_or_else(|| anyhow!("HOME is unavailable; specify --dictionary-dir PATH."))
}

pub(crate) fn run(command: DictionaryCommand, out: &mut impl Write) -> Result<()> {
    match command {
        DictionaryCommand::Import {
            bundle,
            dictionary_dir,
        } => {
            let root = resolve_root(dictionary_dir.as_ref())?;
            let generation = import_bundle(&root, &bundle)?;
            writeln!(
                out,
                "Verified dictionary installed: {}",
                generation.escape_debug()
            )?;
            writeln!(
                out,
                "Managed files must remain unchanged; reimport publishes a new generation."
            )?;
        }
        DictionaryCommand::Verify { dictionary_dir } => {
            verify(resolve_root(dictionary_dir.as_ref())?)?;
            writeln!(
                out,
                "Full pinned verification passed for the dictionary and both publisher notices."
            )?;
        }
    }
    Ok(())
}
