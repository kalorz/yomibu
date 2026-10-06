//! Explicit bounded cache reads and complete atomic publication; no environment lookup.
use crate::retrieval::{EmbeddingCache, EmbeddingError};
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

pub const MAX_CACHE_BYTES: u64 = 134217728;

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingCacheFileError {
    #[error("{operation}")]
    Read {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("Embedding cache input exceeds 134217728 bytes.")]
    OversizedInput,
    #[error("Invalid Embedding cache JSON")]
    InvalidJson(#[source] serde_json::Error),
    #[error(transparent)]
    InvalidCache(#[from] EmbeddingError),
    #[error("{operation}")]
    BeforeReplacement {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("Embedding cache exceeds the readable byte limit; previous cache preserved.")]
    OversizedOutput,
    #[error("Embedding cache published; directory durability uncertain")]
    DurabilityUncertain(#[source] io::Error),
}

/// Selects one explicit file; construction performs no I/O or directory creation.
pub struct EmbeddingCacheFile {
    path: PathBuf,
}
impl EmbeddingCacheFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    pub fn load(&self) -> Result<Option<EmbeddingCache>, EmbeddingCacheFileError> {
        match std::fs::metadata(&self.path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(EmbeddingCacheFileError::Read {
                    operation: "Reading embedding cache",
                    source,
                });
            }
            Ok(_) => {}
        }
        let file =
            std::fs::File::open(&self.path).map_err(|source| EmbeddingCacheFileError::Read {
                operation: "Cannot open the explicitly supplied embedding cache input",
                source,
            })?;
        let mut bytes = Vec::new();
        file.take(MAX_CACHE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|source| EmbeddingCacheFileError::Read {
                operation: "Cannot read the explicitly supplied embedding cache input",
                source,
            })?;
        if bytes.len() as u64 > MAX_CACHE_BYTES {
            return Err(EmbeddingCacheFileError::OversizedInput);
        }
        let cache: EmbeddingCache =
            serde_json::from_slice(&bytes).map_err(EmbeddingCacheFileError::InvalidJson)?;
        cache.validate()?;
        Ok(Some(cache))
    }
    /// Validate before creating temporary storage or replacing a usable file.
    pub fn save(&self, cache: &EmbeddingCache) -> Result<(), EmbeddingCacheFileError> {
        cache.validate()?;
        self.save_bounded(cache, MAX_CACHE_BYTES)
    }
    fn save_bounded(
        &self,
        cache: &EmbeddingCache,
        limit: u64,
    ) -> Result<(), EmbeddingCacheFileError> {
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let before =
            |operation, source| EmbeddingCacheFileError::BeforeReplacement { operation, source };
        let mut file = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| before("Embedding cache temporary file", e))?;
        // Preserve I/O causes and the old readable-size boundary before publication.
        serde_json::to_writer(&mut file, cache)
            .map_err(|e| before("Writing embedding cache", io::Error::other(e)))?;
        if file
            .as_file()
            .metadata()
            .map_err(|e| before("Inspecting embedding cache", e))?
            .len()
            > limit
        {
            return Err(EmbeddingCacheFileError::OversizedOutput);
        }
        file.as_file()
            .sync_all()
            .map_err(|e| before("Synchronizing embedding cache", e))?;
        file.persist(&self.path)
            .map_err(|e| before("Publishing embedding cache", e.error))?;
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(EmbeddingCacheFileError::DurabilityUncertain)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{adapters::embeddings::LexicalEmbedder, ports::Embedder};
    #[test]
    fn oversized_embedding_cache_preserves_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        std::fs::write(&path, "previous cache").unwrap();
        let cache = EmbeddingCache {
            version: 1,
            model: LexicalEmbedder::new().model_identity().clone(),
            entries: Vec::new(),
        };
        assert!(matches!(
            EmbeddingCacheFile::new(&path).save_bounded(&cache, 16),
            Err(EmbeddingCacheFileError::OversizedOutput)
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "previous cache");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
