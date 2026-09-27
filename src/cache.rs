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
        "No cache at {path}; run yomibu sync with WANIKANI_API_TOKEN set, or select an existing cache with --data-dir PATH."
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

#[derive(Debug, Error)]
pub enum WriteError {
    #[error(transparent)]
    ExistingCache(#[from] CacheError),
    #[error("Invalid new snapshot; the previous cache has not been replaced: {0}")]
    InvalidSnapshot(#[from] ValidationError),
    #[error("This cache belongs to a different WaniKani account; use another --data-dir PATH.")]
    AccountMismatch,
    #[error("Cannot prepare or replace cache; the previous cache has not been replaced: {0}")]
    BeforeReplacement(#[from] io::Error),
    #[error("Cannot encode snapshot; the previous cache has not been replaced: {0}")]
    Encode(#[from] serde_json::Error),
    #[error(
        "Cache was replaced, but synchronizing its directory failed; durability is uncertain: {0}"
    )]
    DurabilityUncertain(io::Error),
    #[error("Another sync holds the lock; wait for it to finish or use another --data-dir PATH.")]
    Locked,
}

/// Owns the advisory lock for the entire fetch-and-replace operation.
/// Dropping the guard releases the lock without removing the shared lock file.
#[derive(Debug)]
pub struct SyncGuard {
    data_dir: PathBuf,
    _lock: fs::File,
}

impl SyncGuard {
    pub fn acquire(data_dir: &Path) -> Result<Self, WriteError> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(data_dir)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(data_dir.join("wanikani.lock"))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(WriteError::Locked),
            Err(fs::TryLockError::Error(error)) => return Err(error.into()),
        }
        existing(data_dir)?;
        Ok(Self {
            data_dir: data_dir.to_path_buf(),
            _lock: lock,
        })
    }

    pub fn replace(&self, snapshot: &Snapshot) -> Result<(), WriteError> {
        use std::io::Write;
        snapshot.validate()?;
        if let Some(previous) = existing(&self.data_dir)?
            && previous.learner.id != snapshot.learner.id
        {
            return Err(WriteError::AccountMismatch);
        }
        #[derive(serde::Serialize)]
        struct WritableEnvelope<'a> {
            schema_version: u32,
            snapshot: &'a Snapshot,
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&self.data_dir)?;
        serde_json::to_writer(
            &mut temporary,
            &WritableEnvelope {
                schema_version: 1,
                snapshot,
            },
        )?;
        temporary.flush()?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(self.data_dir.join("wanikani.json"))
            .map_err(|error| WriteError::BeforeReplacement(error.error))?;
        fs::File::open(&self.data_dir)
            .and_then(|directory| directory.sync_all())
            .map_err(WriteError::DurabilityUncertain)
    }
}

fn existing(data_dir: &Path) -> Result<Option<Snapshot>, CacheError> {
    match load(data_dir) {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(CacheError::Missing { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}
