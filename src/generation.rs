//! Experimental text from one explicit provider request; never accepted exercises.
//!
//! The caller explicitly authorizes each request, supplies credentials and owns
//! the Tokio runtime. Assessment then stays synchronous and offline:
//!
//! ```no_run
//! use std::path::Path;
//! use yomibu::{adapters::{openai::Client, sudachi::SudachiAnalyzer},
//!     evaluation::{EvaluationBindings, VocabularyEntry, GrammarBinding, GrammarRule},
//!     grammar::GrammarDeclarations};
//!
//! # async fn example(api_key: &str, dictionary: &Path) -> Result<(), Box<dyn std::error::Error>> {
//! let grammar = GrammarDeclarations::from_descriptions(["です — nominal copula"])?;
//! let bindings = EvaluationBindings {
//!     vocabulary: vec![VocabularyEntry {
//!         written_form: "犬".into(), reading: "イヌ".into(), sense: "dog".into(),
//!         direct_object: false,
//!     }],
//!     grammar: vec![GrammarBinding { declaration_id: 1, rule: GrammarRule::NominalDesu }],
//! };
//! bindings.validate(&grammar)?;
//! let analyzer = SudachiAnalyzer::load(dictionary)?;
//! let client = Client::new(api_key)?;
//! let generated = client.generate_candidates(&grammar, &bindings).await?;
//! let assessments = generated.assess(&analyzer);
//! assert_eq!(generated.texts().len(), assessments.len());
//! // Inspect both results. A completed Pass is still not an accepted exercise.
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};

use crate::{
    adapters::{
        openai::ProviderError,
        sudachi::{AnalysisError, SudachiAnalyzer},
    },
    analysis::{Sentence, SentenceAnalysis, SentenceError},
    evaluation::{Evaluation, EvaluationBindings, EvaluationError, evaluate},
    grammar::GrammarDeclarations,
};

#[derive(Debug, thiserror::Error)]
pub enum GenerationError {
    #[error(transparent)]
    Bindings(#[from] EvaluationError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// Counts reported by the provider, not an independently verified bill.
#[derive(Debug, Deserialize, Serialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Serialize)]
pub struct GenerationProvenance {
    pub provider: &'static str,
    pub requested_model: &'static str,
    pub returned_model: String,
    pub requested_tier: &'static str,
    pub returned_tier: Option<String>,
    pub prompt_revision: &'static str,
    pub request_sha256: String,
    pub request_bytes: usize,
    pub response_id: String,
    pub request_id: Option<String>,
    pub request_count: u8,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug)]
pub struct GeneratedCandidates<'input> {
    pub(crate) texts: [String; 2],
    pub(crate) provenance: GenerationProvenance,
    pub(crate) grammar: &'input GrammarDeclarations,
    pub(crate) bindings: &'input EvaluationBindings,
}

impl GeneratedCandidates<'_> {
    pub fn texts(&self) -> &[String; 2] {
        &self.texts
    }
    pub fn provenance(&self) -> &GenerationProvenance {
        &self.provenance
    }
    /// Each candidate is assessed independently against the unchanged permissions.
    pub fn assess(&self, analyzer: &SudachiAnalyzer) -> [CandidateAssessment<'_>; 2] {
        self.texts.each_ref().map(|text| {
            let sentence = match Sentence::new(text) {
                Ok(sentence) => sentence,
                Err(error) => {
                    return CandidateAssessment::ExecutionError {
                        analysis: None,
                        error: CandidateError::Sentence(error),
                    };
                }
            };
            let analysis = match analyzer.analyze(sentence) {
                Ok(analysis) => analysis,
                Err(error) => {
                    return CandidateAssessment::ExecutionError {
                        analysis: None,
                        error: CandidateError::Analysis(error),
                    };
                }
            };
            match evaluate(&analysis, self.grammar, self.bindings) {
                Ok(evaluation) => CandidateAssessment::Completed {
                    analysis,
                    evaluation: Box::new(evaluation),
                },
                Err(error) => CandidateAssessment::ExecutionError {
                    analysis: Some(analysis),
                    error: CandidateError::Evaluation(error),
                },
            }
        })
    }
}

#[derive(Debug)]
pub enum CandidateAssessment<'a> {
    Completed {
        analysis: SentenceAnalysis<'a>,
        evaluation: Box<Evaluation>,
    },
    ExecutionError {
        analysis: Option<SentenceAnalysis<'a>>,
        error: CandidateError,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum CandidateError {
    #[error(transparent)]
    Sentence(SentenceError),
    // Retain the typed cause for callers without reflecting dependency text in reports.
    #[error("Pinned Sudachi analysis failed.")]
    Analysis(#[source] AnalysisError),
    #[error(transparent)]
    Evaluation(EvaluationError),
}
