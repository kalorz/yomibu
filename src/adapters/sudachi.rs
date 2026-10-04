//! Pinned Sudachi/Core adapter with explicit loading and no ambient configuration.

use std::{fs::File, io::Read, path::Path};

use sha2::{Digest, Sha256};
use sudachi::{
    analysis::{Mode, Tokenize, morpheme::Morpheme, stateless_tokenizer::StatelessTokenizer},
    config::ConfigBuilder,
    dic::{
        dictionary::JapaneseDictionary,
        storage::{Storage, SudachiDicData},
    },
};

use crate::analysis::{AnalysisProvenance, LexicalUnit, Sentence, SentenceAnalysis, Token};

pub const ANALYZER_REVISION: &str = "90fd6068c80c2fc3b63e0dbab0e341475bad4d8f";
pub const DICTIONARY_VERSION: &str = "SudachiDict Core 20260723 V0";
pub const DICTIONARY_SHA256: &str =
    "53fa281d11eef3769712fe1c3c892117338f9892bee6daf4dad51daa5281bb6f";
const DICTIONARY_BYTES: u64 = 217_466_039;
const CONFIGURATION: &[u8] = include_bytes!("sudachi.json");

#[derive(Debug, thiserror::Error)]
pub enum DictionaryError {
    #[error("Cannot read the explicitly selected dictionary: {0}")]
    Io(#[from] std::io::Error),
    #[error("Dictionary does not match the pinned SudachiDict Core 20260723 V0 bytes/checksum.")]
    Mismatch,
    #[error("Cannot construct the pinned analyzer configuration: {0}")]
    Configuration(#[from] sudachi::config::ConfigError),
    #[error("Cannot initialize the pinned analyzer: {0}")]
    Initialization(#[from] Box<sudachi::error::SudachiError>),
}

#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("Sudachi analysis failed: {0}")]
    Analyzer(#[from] Box<sudachi::error::SudachiError>),
    #[error("Analyzer returned an invalid original UTF-8 span.")]
    InvalidSpan,
}

/// Owns one verified dictionary; shared references support repeated offline calls.
pub struct SudachiAnalyzer {
    dictionary: JapaneseDictionary,
}

impl SudachiAnalyzer {
    /// Read only the supplied dictionary. No configuration files, plugins,
    /// environment variables, user dictionaries, or network resources are loaded.
    /// The roughly 217 MB dictionary stays owned in memory to prevent file-change
    /// races between checksum verification and the dependency's dictionary access.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, DictionaryError> {
        let file = File::open(path)?;
        if file.metadata()?.len() != DICTIONARY_BYTES {
            return Err(DictionaryError::Mismatch);
        }
        let mut bytes = Vec::new();
        file.take(DICTIONARY_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != DICTIONARY_BYTES
            || format!("{:x}", Sha256::digest(&bytes)) != DICTIONARY_SHA256
        {
            return Err(DictionaryError::Mismatch);
        }
        let config = ConfigBuilder::from_bytes(CONFIGURATION)?.build();
        let dictionary = JapaneseDictionary::from_cfg_storage_with_embedded_chardef(
            &config,
            SudachiDicData::new(Storage::Owned(bytes)),
        )
        .map_err(Box::new)?;
        Ok(Self { dictionary })
    }

    pub fn analyze<'a>(
        &self,
        sentence: Sentence<'a>,
    ) -> Result<SentenceAnalysis<'a>, AnalysisError> {
        let tokenizer = StatelessTokenizer::new(&self.dictionary);
        let whole = tokenizer
            .tokenize(sentence.text(), Mode::C, false)
            .map_err(Box::new)?;
        let mut split = whole.empty_clone();
        let mut units = Vec::with_capacity(whole.len());
        for morpheme in whole.iter() {
            split.clear();
            morpheme.split_into(Mode::A, &mut split).map_err(Box::new)?;
            let token = extract_token(&morpheme, sentence.text())?;
            let components = if split.is_empty() {
                vec![token.clone()]
            } else {
                split
                    .iter()
                    .map(|m| extract_token(&m, sentence.text()))
                    .collect::<Result<_, _>>()?
            };
            units.push(LexicalUnit { token, components });
        }
        Ok(SentenceAnalysis {
            sentence,
            units,
            provenance: AnalysisProvenance {
                analyzer_revision: ANALYZER_REVISION,
                dictionary_version: DICTIONARY_VERSION,
                dictionary_sha256: DICTIONARY_SHA256,
                configuration_sha256: format!("{:x}", Sha256::digest(CONFIGURATION)),
            },
        })
    }
}

fn extract_token(
    morpheme: &Morpheme<'_, &JapaneseDictionary>,
    original: &str,
) -> Result<Token, AnalysisError> {
    let span = morpheme.begin()..morpheme.end();
    if span.is_empty() || original.get(span.clone()) != Some(&*morpheme.surface()) {
        return Err(AnalysisError::InvalidSpan);
    }
    Ok(Token {
        span,
        dictionary_form: morpheme.dictionary_form().into(),
        reading: morpheme.reading_form().into(),
        part_of_speech: morpheme.part_of_speech().to_vec(),
        out_of_vocabulary: morpheme.is_oov(),
    })
}
