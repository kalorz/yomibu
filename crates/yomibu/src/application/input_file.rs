//! Read an explicitly chosen input file with a caller-supplied byte limit.

use std::{io::Read, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InputFileError {
    #[error("Cannot open the explicitly supplied {kind} input")]
    Open {
        kind: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Cannot read the explicitly supplied {kind} input")]
    Read {
        kind: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{kind} input exceeds {limit_label}.")]
    TooLarge { kind: String, limit_label: String },
}

pub fn read_bounded(
    path: &Path,
    kind: &str,
    limit: usize,
    limit_label: &str,
) -> Result<Vec<u8>, InputFileError> {
    let file = std::fs::File::open(path).map_err(|source| InputFileError::Open {
        kind: kind.to_ascii_lowercase(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.take((limit as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| InputFileError::Read {
            kind: kind.to_ascii_lowercase(),
            source,
        })?;
    if bytes.len() > limit {
        return Err(InputFileError::TooLarge {
            kind: kind.into(),
            limit_label: limit_label.into(),
        });
    }
    Ok(bytes)
}
