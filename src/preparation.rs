//! Offline retrieval evidence for explicit practice intentions, not an exercise.

use crate::{
    domain::{
        Assignment, ContextSentence, LexicalContent, Subject, SubjectKind, ValidationError,
        WaniKaniSyncData,
    },
    grammar::GrammarDeclarations,
    knowledge::{ExclusionReason, KnowledgeDecision, LearnerKnowledge, LearnerKnowledgePolicy},
};
use std::collections::BTreeMap;

/// One intended use; the sense is an exact accepted source gloss in this slice.
#[derive(Debug, PartialEq, Eq)]
pub struct PracticeTarget {
    pub word: String,
    pub intended_reading: String,
    pub intended_sense: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnassessedAspect {
    Grammar,
    ReadingSenseAssociation,
    ExampleSuitability,
    LinguisticCorrectness,
}

/// Source attachment explains example selection; it does not validate an example.
#[derive(Debug, PartialEq, Eq)]
pub struct RetrievedTarget<'a> {
    pub target: &'a PracticeTarget,
    pub subject: &'a Subject,
    pub assignment: &'a Assignment,
    /// All attached examples in source order; empty means none were recorded.
    pub examples: &'a [ContextSentence],
}

#[derive(Debug, PartialEq, Eq)]
pub struct PreparedContext<'a> {
    pub knowledge: LearnerKnowledge<'a>,
    pub targets: Vec<RetrievedTarget<'a>>,
    pub unassessed: &'static [UnassessedAspect],
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PrepareError {
    #[error("Supply at least one practice target.")]
    EmptyTargets,
    #[error("Target entry {entry} has blank {field}.")]
    BlankTargetField { entry: usize, field: &'static str },
    #[error("Invalid source data: {0}")]
    InvalidSourceData(#[from] ValidationError),
    #[error("Target entry {entry} has no vocabulary material in this cache.")]
    TargetNotFound { entry: usize },
    #[error(
        "Target entry {entry} lacks an exact accepted cached reading and gloss; kana-only records supply no reading."
    )]
    UnsupportedUse { entry: usize },
    #[error("Target entry {entry} matches multiple source subjects: {subject_ids:?}.")]
    AmbiguousTarget { entry: usize, subject_ids: Vec<u64> },
    #[error("Target entry {entry}, subject {subject_id}, is excluded by policy: {reason:?}.")]
    IneligibleTarget {
        entry: usize,
        subject_id: u64,
        reason: ExclusionReason,
    },
}

/// Validate explicit inputs, derive knowledge, and retrieve source-attached data.
///
/// No I/O, normalization, inferred readings, semantic ranking, or writes occur.
/// Requests preserve order and duplicates. Matching reading and gloss fields
/// separately does not establish their linguistic association. Ambiguous records
/// are rejected before applying eligibility, which is not a disambiguator.
pub fn prepare_context<'a>(
    source: &'a WaniKaniSyncData,
    grammar: &'a GrammarDeclarations,
    policy: &LearnerKnowledgePolicy,
    targets: &'a [PracticeTarget],
) -> Result<PreparedContext<'a>, PrepareError> {
    validate_targets(targets)?;
    let knowledge = policy.derive(source, grammar)?;
    let mut by_word = BTreeMap::<&str, Vec<_>>::new();
    for entry in &knowledge.materials {
        if let Some(subject) = entry.material
            && matches!(
                subject.kind(),
                SubjectKind::Vocabulary | SubjectKind::KanaVocabulary
            )
        {
            by_word
                .entry(subject.characters.as_str())
                .or_default()
                .push((subject, entry.assignment, entry.decision));
        }
    }
    let selected = targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            let entry = index + 1;
            let available = by_word
                .get(target.word.as_str())
                .ok_or(PrepareError::TargetNotFound { entry })?;
            let matches: Vec<_> = available
                .iter()
                .filter(|(subject, _, _)| supports_use(subject, target))
                .collect();
            let [matched] = matches.as_slice() else {
                return Err(if matches.is_empty() {
                    PrepareError::UnsupportedUse { entry }
                } else {
                    PrepareError::AmbiguousTarget {
                        entry,
                        subject_ids: matches.iter().map(|(subject, _, _)| subject.id).collect(),
                    }
                });
            };
            let (subject, assignment, decision) = **matched;
            if let KnowledgeDecision::Excluded(reason) = decision {
                return Err(PrepareError::IneligibleTarget {
                    entry,
                    subject_id: subject.id,
                    reason,
                });
            }
            let assignment = assignment.ok_or(PrepareError::IneligibleTarget {
                entry,
                subject_id: subject.id,
                reason: ExclusionReason::NoAssignment,
            })?;
            let examples = match &subject.lexical {
                LexicalContent::Vocabulary {
                    context_sentences, ..
                } => context_sentences.as_slice(),
                _ => &[],
            };
            Ok(RetrievedTarget {
                target,
                subject,
                assignment,
                examples,
            })
        })
        .collect::<Result<_, PrepareError>>()?;
    Ok(PreparedContext {
        knowledge,
        targets: selected,
        unassessed: &[
            UnassessedAspect::Grammar,
            UnassessedAspect::ReadingSenseAssociation,
            UnassessedAspect::ExampleSuitability,
            UnassessedAspect::LinguisticCorrectness,
        ],
    })
}

fn supports_use(subject: &Subject, target: &PracticeTarget) -> bool {
    let LexicalContent::Vocabulary { readings, .. } = &subject.lexical else {
        return false;
    };
    readings
        .iter()
        .any(|reading| reading.accepted_answer && reading.reading == target.intended_reading)
        && subject
            .meanings
            .iter()
            .any(|meaning| meaning.accepted_answer && meaning.meaning == target.intended_sense)
}

fn validate_targets(targets: &[PracticeTarget]) -> Result<(), PrepareError> {
    if targets.is_empty() {
        return Err(PrepareError::EmptyTargets);
    }
    for (index, target) in targets.iter().enumerate() {
        for (field, value) in [
            ("word", &target.word),
            ("intended_reading", &target.intended_reading),
            ("intended_sense", &target.intended_sense),
        ] {
            if value.trim().is_empty() {
                return Err(PrepareError::BlankTargetField {
                    entry: index + 1,
                    field,
                });
            }
        }
    }
    Ok(())
}
