//! Explicit, read-only loading of manual grammar assertions.

use crate::grammar::{GrammarDeclarations, GrammarError};
use serde::Deserialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Cannot read grammar file {path:?}; check the path and permissions.")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Invalid grammar JSON; expected version and a declarations array of strings.")]
    InvalidJson {
        #[source]
        source: serde_json::Error,
    },
    #[error("Unsupported grammar input version {version}; expected version 1.")]
    UnsupportedVersion { version: u32 },
    #[error(transparent)]
    InvalidDeclaration(#[from] GrammarError),
}

/// Read the explicit file without creating directories, locking, or writing back.
pub fn load(path: &Path) -> Result<GrammarDeclarations, Error> {
    let text = fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse(&text)
}

/// Decode assertions without interpreting or recognizing the described grammar.
pub fn parse(text: &str) -> Result<GrammarDeclarations, Error> {
    #[derive(Deserialize)]
    struct Version {
        version: u32,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        #[serde(rename = "version")]
        _version: u32,
        declarations: Vec<String>,
    }
    let invalid_json = |source| Error::InvalidJson { source };
    let version: Version = serde_json::from_str(text).map_err(invalid_json)?;
    if version.version != 1 {
        return Err(Error::UnsupportedVersion {
            version: version.version,
        });
    }
    let input: Input = serde_json::from_str(text).map_err(invalid_json)?;
    Ok(GrammarDeclarations::from_descriptions(input.declarations)?)
}
