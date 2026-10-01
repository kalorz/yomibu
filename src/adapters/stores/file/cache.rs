//! Synchronous validated cache access and advisory writer locking on macOS/Linux.

use crate::domain::{ValidationError, WaniKaniSyncData};
use serde::Deserialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// Failures to read or validate an existing cache; loading never modifies it.
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
    // Schema 1 keeps this wire name independently of the public Rust type name.
    snapshot: WaniKaniSyncData,
}

/// Read a schema-1 cache without creating files, locking, or accessing the environment.
pub fn load(data_dir: &Path) -> Result<WaniKaniSyncData, CacheError> {
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

/// Persistence failures classified by whether replacement has occurred.
/// Only `DurabilityUncertain` means the new cache is already visible.
#[derive(Debug, Error)]
pub enum WriteError {
    #[error(transparent)]
    ExistingCache(#[from] CacheError),
    #[error("Invalid new sync data; the previous cache has not been replaced: {0}")]
    InvalidSyncData(#[from] ValidationError),
    #[error("This cache belongs to a different WaniKani account; use another --data-dir PATH.")]
    AccountMismatch,
    #[error("Cannot prepare or replace cache; the previous cache has not been replaced: {0}")]
    BeforeReplacement(#[from] io::Error),
    #[error("Cannot encode sync data; the previous cache has not been replaced: {0}")]
    Encode(#[from] serde_json::Error),
    #[error(
        "Cache was replaced, but synchronizing its directory failed; durability is uncertain: {0}"
    )]
    DurabilityUncertain(#[source] io::Error),
    #[error("Another sync holds the lock; wait for it to finish or use another --data-dir PATH.")]
    Locked,
}

/// Owns the advisory lock for the entire fetch-and-replace operation.
/// Dropping the guard releases the lock without removing the shared lock file.
#[derive(Debug)]
pub struct SyncGuard {
    data_dir: PathBuf,
    lock: fs::File,
}

impl Drop for SyncGuard {
    fn drop(&mut self) {
        // Closing alone can leave the lock held by a descriptor inherited during
        // concurrent process creation. Release it at the guard's lifetime boundary.
        let _ = self.lock.unlock();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteStep {
    Create,
    Encode,
    Flush,
    SyncFile,
    Replace,
    SyncDirectory,
}

impl SyncGuard {
    /// Acquire the writer lock without waiting and validate any existing cache.
    /// Creates private directories and a lock file if needed. Keep this guard
    /// alive throughout retrieval and replacement; offline readers need no lock.
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
            lock,
        })
    }

    /// Validate and atomically replace the cache, refusing a different account.
    /// Errors before replacement preserve the old cache. On
    /// [`WriteError::DurabilityUncertain`], the complete new cache is visible but
    /// its directory sync failed; callers must not assume rollback.
    pub fn replace(&self, sync_data: &WaniKaniSyncData) -> Result<(), WriteError> {
        self.replace_with(sync_data, |_| Ok(()))
    }

    // A private checkpoint lets tests fail or interrupt each storage boundary
    // while exercising the same file operations and error mapping as callers.
    fn replace_with(
        &self,
        sync_data: &WaniKaniSyncData,
        mut before: impl FnMut(WriteStep) -> io::Result<()>,
    ) -> Result<(), WriteError> {
        use std::io::Write;
        sync_data.validate()?;
        if let Some(previous) = existing(&self.data_dir)?
            && previous.learner.id != sync_data.learner.id
        {
            return Err(WriteError::AccountMismatch);
        }
        #[derive(serde::Serialize)]
        struct WritableEnvelope<'a> {
            schema_version: u32,
            snapshot: &'a WaniKaniSyncData,
        }
        before(WriteStep::Create)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.data_dir)?;
        before(WriteStep::Encode)?;
        serde_json::to_writer(
            &mut temporary,
            &WritableEnvelope {
                schema_version: 1,
                snapshot: sync_data,
            },
        )?;
        before(WriteStep::Flush)?;
        temporary.flush()?;
        before(WriteStep::SyncFile)?;
        temporary.as_file().sync_all()?;
        before(WriteStep::Replace)?;
        temporary
            .persist(self.data_dir.join("wanikani.json"))
            .map_err(|error| WriteError::BeforeReplacement(error.error))?;
        before(WriteStep::SyncDirectory)
            .and_then(|()| fs::File::open(&self.data_dir))
            .and_then(|directory| directory.sync_all())
            .map_err(WriteError::DurabilityUncertain)
    }
}

fn existing(data_dir: &Path) -> Result<Option<WaniKaniSyncData>, CacheError> {
    match load(data_dir) {
        Ok(sync_data) => Ok(Some(sync_data)),
        Err(CacheError::Missing { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
