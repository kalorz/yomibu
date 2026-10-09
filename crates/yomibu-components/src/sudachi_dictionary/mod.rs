//! Pinned Sudachi/Core adapter with explicit loading and no ambient configuration.

pub const COMPONENT: yomibu_core::component::Component = yomibu_core::component::Component {
    id: "sudachi-dictionary",
    settings: &[],
};

use sha2::{Digest, Sha256};
use sudachi::{
    analysis::{Mode, Tokenize, morpheme::Morpheme, stateless_tokenizer::StatelessTokenizer},
    config::ConfigBuilder,
    dic::{
        dictionary::JapaneseDictionary,
        storage::{Storage, SudachiDicData},
    },
};

pub mod installation;
use installation::ManagedInstallation;
use yomibu_core::domain::analysis::{
    AnalysisProvenance, DictionaryProvenance, LexicalUnit, Sentence, SentenceAnalysis, Token,
};

pub const ANALYZER_REVISION: &str = "90fd6068c80c2fc3b63e0dbab0e341475bad4d8f";
pub const DICTIONARY_VERSION: &str = "SudachiDict Core 20260723 V0";
pub const DICTIONARY_SHA256: &str =
    "53fa281d11eef3769712fe1c3c892117338f9892bee6daf4dad51daa5281bb6f";
pub(crate) const DICTIONARY_BYTES: u64 = 217_466_039;
const CONFIGURATION: &[u8] = include_bytes!("sudachi.json");

#[derive(Debug, thiserror::Error)]
pub enum DictionaryError {
    #[error("Cannot map the selected dictionary generation: {0}")]
    Mapping(#[from] std::io::Error),
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

/// Keeps one dictionary's storage alive; shared references support repeated calls.
pub struct SudachiAnalyzer {
    dictionary: JapaneseDictionary,
    generation: String,
}

impl yomibu_core::capabilities::SentenceAnalyzer for SudachiAnalyzer {
    type Error = AnalysisError;

    fn analyze<'a>(&self, sentence: Sentence<'a>) -> Result<SentenceAnalysis<'a>, AnalysisError> {
        self.analyze(sentence)
    }
}

impl SudachiAnalyzer {
    /// Map the handle checked by `ManagedInstallation::open`, retaining embedded
    /// configuration and character definitions. No full hash or ambient resource
    /// access is done here.
    ///
    /// # Safety
    /// The caller must ensure this generation was fully verified by Yomibu's
    /// importer and that its dictionary bytes remain unchanged for this analyzer's
    /// entire lifetime. No process may write or truncate the mapped file. Receipts,
    /// metadata, read-only permissions and advisory locks cannot prove this.
    pub unsafe fn load(installation: ManagedInstallation) -> Result<Self, DictionaryError> {
        // The caller supplies the file-stability guarantee. Mapping this same
        // checked handle avoids reopening a path between startup checks and use.
        let mapping = unsafe { memmap2::Mmap::map(&installation.dictionary) }?;
        let config = ConfigBuilder::from_bytes(CONFIGURATION)?.build();
        let dictionary = JapaneseDictionary::from_cfg_storage_with_embedded_chardef(
            &config,
            SudachiDicData::new(Storage::File(mapping)),
        )
        .map_err(Box::new)?;
        Ok(Self {
            dictionary,
            generation: installation.generation,
        })
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
                dictionary_loading: DictionaryProvenance {
                    generation: self.generation.clone(),
                    verification: "full_sha256_at_installation",
                    startup_checks: "installation_records_metadata_and_header",
                    file_stability: "requires_unchanged_managed_files_for_analyzer_lifetime",
                },
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
