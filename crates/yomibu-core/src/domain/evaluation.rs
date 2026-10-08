use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::domain::grammar::{GrammarDeclarations, GrammarRule};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VocabularyEntry {
    pub written_form: String,
    /// Dictionary-style katakana reading; no implicit normalization.
    pub reading: String,
    pub sense: String,
    /// Explicit synthetic evidence for this lexical use; never inferred from を.
    /// Does not assess an object/predicate combination or resolve multiword uses.
    pub direct_object: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrammarBinding {
    /// One-based ID in the supplied declarations; descriptions are never parsed.
    pub declaration_id: usize,
    pub rule: GrammarRule,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationBindings {
    pub vocabulary: Vec<VocabularyEntry>,
    pub grammar: Vec<GrammarBinding>,
}

impl EvaluationBindings {
    /// Validate explicit permissions without analysis or interpreting descriptions.
    pub fn validate(&self, grammar: &GrammarDeclarations) -> Result<(), EvaluationError> {
        if self.vocabulary.iter().any(|word| {
            [&word.written_form, &word.reading, &word.sense]
                .iter()
                .any(|s| s.trim().is_empty())
        }) {
            return Err(EvaluationError::BlankVocabulary);
        }
        if self.grammar.iter().any(|binding| {
            !grammar
                .entries()
                .iter()
                .any(|entry| entry.id == binding.declaration_id)
        }) {
            return Err(EvaluationError::MissingDeclaration);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CheckOutcome {
    Pass,
    Fail,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CheckState {
    Completed(CheckOutcome),
    NotRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CheckKind {
    Vocabulary,
    Inflection,
    Particles,
    Nominal,
    Scope,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub span: Range<usize>,
    pub reason: &'static str,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Check {
    pub state: CheckState,
    pub findings: Vec<Finding>,
    pub coverage: &'static str,
}

/// These require evidence outside the morphological and bounded structural checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum UnassessedAspect {
    Naturalness,
    MultiwordExpressions,
    ContextualReadingAndSense,
}

pub const REPORT_NOTICE: &str = "Bounded analysis only; these sentences are not accepted exercises. Pass does not establish linguistic validity or mastery.";

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Evaluation {
    pub notice: &'static str,
    pub unassessed: [UnassessedAspect; 3],
    pub basis: EvaluationBasis,
    vocabulary: Check,
    inflection: Check,
    particles: Check,
    nominal: Check,
    scope: Check,
}

impl Evaluation {
    /// Validate findings against the original text and check states.
    /// Callers must keep this evaluation paired with that text.
    pub fn new(
        text: &str,
        basis: EvaluationBasis,
        vocabulary: Check,
        inflection: Check,
        particles: Check,
        nominal: Check,
        scope: Check,
    ) -> Result<Self, EvaluationError> {
        for check in [&vocabulary, &inflection, &particles, &nominal, &scope] {
            if check
                .findings
                .iter()
                .any(|finding| text.get(finding.span.clone()).is_none())
            {
                return Err(EvaluationError::InvalidFindingSpan);
            }
            let consistent = match check.state {
                CheckState::Completed(CheckOutcome::Fail | CheckOutcome::Inconclusive) => {
                    !check.findings.is_empty()
                }
                CheckState::Completed(CheckOutcome::Pass) | CheckState::NotRun => {
                    check.findings.is_empty()
                }
            };
            if !consistent {
                return Err(EvaluationError::InvalidCheckState);
            }
        }
        Ok(Self {
            notice: REPORT_NOTICE,
            unassessed: [
                UnassessedAspect::Naturalness,
                UnassessedAspect::MultiwordExpressions,
                UnassessedAspect::ContextualReadingAndSense,
            ],
            basis,
            vocabulary,
            inflection,
            particles,
            nominal,
            scope,
        })
    }

    pub fn check(&self, kind: CheckKind) -> &Check {
        match kind {
            CheckKind::Vocabulary => &self.vocabulary,
            CheckKind::Scope => &self.scope,
            CheckKind::Inflection => &self.inflection,
            CheckKind::Particles => &self.particles,
            CheckKind::Nominal => &self.nominal,
        }
    }

    pub fn outcome(&self) -> CheckState {
        use CheckOutcome::*;
        let states = [
            self.vocabulary.state,
            self.inflection.state,
            self.particles.state,
            self.nominal.state,
            self.scope.state,
        ];
        [
            CheckState::Completed(Fail),
            CheckState::NotRun,
            CheckState::Completed(Inconclusive),
        ]
        .into_iter()
        .find(|state| states.contains(state))
        .unwrap_or(CheckState::Completed(Pass))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EvaluationError {
    #[error("Vocabulary permissions must contain a nonblank form, reading, and sense.")]
    BlankVocabulary,
    #[error("A grammar binding refers to a declaration ID that does not exist.")]
    MissingDeclaration,
    #[error("Analysis must completely cover the original text with valid whole/component spans.")]
    InvalidAnalysis,
    #[error("Finding spans must lie within the original text on UTF-8 boundaries.")]
    InvalidFindingSpan,
    #[error(
        "Fail and Inconclusive checks require findings; Pass and NotRun checks must have none."
    )]
    InvalidCheckState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationBasis {
    ExplicitWordUses,
    FullLearnerInventory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LexicalUncertainty {
    Whitespace,
    OutOfDictionary,
    CompetingUses,
    CompetingIdentities,
    MissingReading,
    ReadingAlternatives,
    MissingMeaning,
    MeaningAlternatives,
    ReadingMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum DirectObjectEvidence {
    Confirmed,
    NotAsserted,
    Unknown,
    Unavailable,
    Unresolved(LexicalUncertainty),
}
