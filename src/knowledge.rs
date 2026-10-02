//! Revisable, deterministic eligibility derived from preserved learner evidence.

use crate::{
    domain::{
        Assignment, ReviewStatistic, Subject, SubjectKind, ValidationError, WaniKaniSyncData,
    },
    grammar::{GrammarDeclaration, GrammarDeclarations},
};
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WaniKaniKnowledgeRule {
    #[default]
    LessonStarted,
    RecordedPass,
}

/// Source timestamps are practice-eligibility rules, not mastery guarantees.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LearnerKnowledgePolicy {
    pub wanikani: WaniKaniKnowledgeRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionReason {
    ContentUnavailable,
    Hidden,
    NoAssignment,
    NoRecordedLessonStart,
    NoRecordedPass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeDecision {
    Eligible,
    Excluded(ExclusionReason),
}

/// Retains the evidence for either eligibility or an explicit exclusion.
#[derive(Debug, PartialEq, Eq)]
pub struct MaterialKnowledge<'a> {
    pub subject_id: u64,
    pub kind: SubjectKind,
    pub material: Option<&'a Subject>,
    pub assignment: Option<&'a Assignment>,
    pub review_statistic: Option<&'a ReviewStatistic>,
    pub decision: KnowledgeDecision,
}

/// An on-demand interpretation. It never replaces the original observations.
#[derive(Debug, PartialEq, Eq)]
pub struct LearnerKnowledge<'a> {
    pub policy: LearnerKnowledgePolicy,
    pub learner_id: &'a str,
    pub sync_started_at: DateTime<Utc>,
    pub sync_completed_at: DateTime<Utc>,
    /// Ordered by source subject ID, independently of collection order.
    pub materials: Vec<MaterialKnowledge<'a>>,
    /// Self-declared familiarity, without a grammar recognizer or equivalence map.
    pub grammar: &'a [GrammarDeclaration],
}

impl LearnerKnowledgePolicy {
    /// Validate and derive without I/O, environment access, time, or persistence.
    ///
    /// Exclusion precedence is unavailable content, any hidden evidence, missing
    /// assignment, then the selected missing lifecycle timestamp. Other evidence
    /// remains inspectable in the result. SRS stages and accuracy are not cutoffs.
    pub fn derive<'a>(
        &self,
        source: &'a WaniKaniSyncData,
        grammar: &'a GrammarDeclarations,
    ) -> Result<LearnerKnowledge<'a>, ValidationError> {
        use ExclusionReason::{
            ContentUnavailable, Hidden, NoAssignment, NoRecordedLessonStart, NoRecordedPass,
        };
        use KnowledgeDecision::{Eligible, Excluded};

        source.validate()?;
        let assignments: BTreeMap<_, _> = source
            .assignments
            .iter()
            .map(|entry| (entry.subject_id, entry))
            .collect();
        let statistics: BTreeMap<_, _> = source
            .review_statistics
            .iter()
            .map(|entry| (entry.subject_id, entry))
            .collect();
        let mut material_by_id: BTreeMap<_, _> = source
            .subjects
            .iter()
            .map(|subject| (subject.id, (subject.kind(), Some(subject))))
            .collect();
        for unavailable in &source.unavailable_subjects {
            material_by_id.insert(unavailable.id, (unavailable.kind, None));
        }
        let materials = material_by_id
            .into_iter()
            .map(|(subject_id, (kind, material))| {
                let assignment = assignments.get(&subject_id).copied();
                let review_statistic = statistics.get(&subject_id).copied();
                let decision = if material.is_none() {
                    Excluded(ContentUnavailable)
                } else if material.is_some_and(|subject| subject.hidden_at.is_some())
                    || assignment.is_some_and(|entry| entry.hidden)
                    || review_statistic.is_some_and(|entry| entry.hidden)
                {
                    Excluded(Hidden)
                } else {
                    match assignment {
                        None => Excluded(NoAssignment),
                        Some(assignment) => match self.wanikani {
                            WaniKaniKnowledgeRule::LessonStarted => {
                                if assignment.started_at.is_some() {
                                    Eligible
                                } else {
                                    Excluded(NoRecordedLessonStart)
                                }
                            }
                            WaniKaniKnowledgeRule::RecordedPass => {
                                if assignment.passed_at.is_some() {
                                    Eligible
                                } else {
                                    Excluded(NoRecordedPass)
                                }
                            }
                        },
                    }
                };
                MaterialKnowledge {
                    subject_id,
                    kind,
                    material,
                    assignment,
                    review_statistic,
                    decision,
                }
            })
            .collect();
        Ok(LearnerKnowledge {
            policy: *self,
            learner_id: &source.learner.id,
            sync_started_at: source.sync_started_at,
            sync_completed_at: source.sync_completed_at,
            materials,
            grammar: grammar.entries(),
        })
    }
}
