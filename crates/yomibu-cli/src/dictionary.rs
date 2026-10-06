//! Executable-only dictionary selection, installation commands and presentation.

use anyhow::{Context, Result, anyhow};
use clap::{Args, Subcommand};
use std::{io::Write, path::PathBuf};
use yomibu::{
    adapters::{
        dictionary::{ManagedInstallation, import_bundle, verify},
        sudachi::SudachiAnalyzer,
    },
    analysis::AnalysisProvenance,
};

#[derive(Args)]
pub(super) struct DictionaryArgs {
    /// External pinned dictionary: full SHA-256 verification and owned bytes.
    #[arg(long, value_name = "PATH", conflicts_with = "dictionary_dir")]
    pub(super) dictionary: Option<PathBuf>,
    /// Managed installation (default: $HOME/.yomibu/dictionaries); published files must remain unchanged.
    #[arg(long, value_name = "PATH")]
    pub(super) dictionary_dir: Option<PathBuf>,
}

impl DictionaryArgs {
    pub(super) fn load(&self) -> Result<SudachiAnalyzer> {
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

#[derive(Subcommand)]
pub(super) enum DictionaryCommand {
    /// Copy and fully verify a dictionary and its publisher notices entirely offline.
    Import {
        /// Directory containing system_core.dic, LEGAL and LICENSE-2.0.txt.
        #[arg(long, value_name = "PATH")]
        bundle: PathBuf,
        /// Managed installation root (default: $HOME/.yomibu/dictionaries).
        #[arg(long, value_name = "PATH")]
        dictionary_dir: Option<PathBuf>,
    },
    /// Fully verify the current dictionary and both notices without changing files.
    Verify {
        #[arg(long, value_name = "PATH")]
        dictionary_dir: Option<PathBuf>,
    },
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

pub(super) fn run(command: DictionaryCommand, out: &mut impl Write) -> Result<()> {
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

pub(super) fn write_loading(
    out: &mut impl Write,
    provenance: &AnalysisProvenance,
) -> std::io::Result<()> {
    if let Some(loading) = &provenance.dictionary_loading {
        writeln!(
            out,
            "Dictionary loading: memory mapped; full SHA-256 verified at installation; startup checks records, size and header."
        )?;
        writeln!(
            out,
            "Dictionary stability: requires unchanged managed files for analyzer lifetime."
        )?;
        writeln!(
            out,
            "Dictionary generation: {}",
            loading.generation.escape_debug()
        )?;
        writeln!(
            out,
            "Expected dictionary SHA-256: {}",
            provenance.dictionary_sha256
        )?;
    }
    Ok(())
}
