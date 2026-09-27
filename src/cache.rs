use crate::domain::{Snapshot, ValidationError};
use serde::Deserialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error(
        "Invalid cache at {path}: {source}; preserve the file and restore a valid backup or select another --data-dir PATH."
    )]
    Invalid {
        path: PathBuf,
        #[source]
        source: ValidationError,
    },
    #[error(
        "No cache at {path}; select an existing cache with --data-dir PATH. Synchronization is not available in this version."
    )]
    Missing { path: PathBuf },
    #[error("Cannot read cache at {path}; check the path and file permissions.")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "Corrupt cache at {path}; preserve the file and restore a valid backup or select another --data-dir PATH."
    )]
    Corrupt {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "Unsupported cache schema {version} at {path}; upgrade Yomibu or select a supported cache with --data-dir PATH."
    )]
    UnsupportedVersion { path: PathBuf, version: u32 },
}

#[derive(Deserialize)]
struct Version {
    schema_version: u32,
}

#[derive(Deserialize)]
struct Envelope {
    snapshot: Snapshot,
}

/// Read a schema-1 cache without creating files, locking, or accessing the environment.
pub fn load(data_dir: &Path) -> Result<Snapshot, CacheError> {
    let path = data_dir.join("wanikani.json");
    let bytes = fs::read(&path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            CacheError::Missing { path: path.clone() }
        } else {
            CacheError::Read {
                path: path.clone(),
                source,
            }
        }
    })?;
    let corrupt = |source| CacheError::Corrupt {
        path: path.clone(),
        source,
    };
    let version: Version = serde_json::from_slice(&bytes).map_err(corrupt)?;
    if version.schema_version != 1 {
        return Err(CacheError::UnsupportedVersion {
            path,
            version: version.schema_version,
        });
    }
    let envelope: Envelope = serde_json::from_slice(&bytes).map_err(corrupt)?;
    envelope
        .snapshot
        .validate()
        .map_err(|source| CacheError::Invalid { path, source })?;
    Ok(envelope.snapshot)
}
