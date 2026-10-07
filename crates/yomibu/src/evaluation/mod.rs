//! Bounded structural and full-inventory evaluation.
//! A passed check never establishes exercise acceptance or linguistic mastery.
//!
//! Result types and check outcomes live here. `structure.rs` owns the supported
//! constructions and safeguards; `inventory.rs` checks the complete inventory
//! and projects its single-use evidence for the structural checker.

mod inventory;
mod structure;

pub(crate) use inventory::{evaluate_inventory, single_use};
pub use structure::evaluate;
pub(crate) use structure::{observed_grammar, reading_matches, regular_stem};

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::grammar::GrammarDeclarations;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrammarRule {
    NominalDesu,
    TopicWa,
    ObjectWo,
    PoliteNonPast,
    PolitePast,
    PoliteNegativeNonPast,
    PoliteNegativePast,
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

/// These require evidence outside A1's morphological and bounded structural checks.
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
    vocabulary: Check,
    inflection: Check,
    particles: Check,
    nominal: Check,
    scope: Check,
}

impl Evaluation {
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
        if states.contains(&CheckState::NotRun) {
            return CheckState::NotRun;
        }
        for outcome in [Fail, Inconclusive] {
            if states.contains(&CheckState::Completed(outcome)) {
                return CheckState::Completed(outcome);
            }
        }
        CheckState::Completed(Pass)
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
}

fn passed(coverage: &'static str) -> Check {
    Check {
        state: CheckState::Completed(CheckOutcome::Pass),
        findings: Vec::new(),
        coverage,
    }
}

fn problem(outcome: CheckOutcome, span: Range<usize>, reason: &'static str) -> Check {
    Check {
        state: CheckState::Completed(outcome),
        findings: vec![Finding { span, reason }],
        coverage: "see findings",
    }
}

fn combine(mut first: Check, second: Check) -> Check {
    let states = [first.state, second.state];
    first.state = if states.contains(&CheckState::Completed(CheckOutcome::Fail)) {
        CheckState::Completed(CheckOutcome::Fail)
    } else if states.contains(&CheckState::Completed(CheckOutcome::Inconclusive)) {
        CheckState::Completed(CheckOutcome::Inconclusive)
    } else {
        CheckState::Completed(CheckOutcome::Pass)
    };
    first.findings.extend(second.findings);
    first.coverage = "all scoped particle occurrences checked";
    first
}
