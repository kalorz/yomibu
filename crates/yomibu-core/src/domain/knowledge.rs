//! Revisable, deterministic eligibility derived from preserved learner evidence.

use chrono::{DateTime, Utc};
use serde::Serialize;

/// Common learner observations; provider stages and raw responses stay with the source.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LearnerProgress {
    pub is_marked_known: Option<bool>,
    pub learning_started_at: Option<DateTime<Utc>>,
    pub first_passed_at: Option<DateTime<Utc>>,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ExclusionReason {
    ContentUnavailable,
    Hidden,
    NoProgress,
    MarkedUnknown,
    NoRecordedLessonStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeDecision {
    Eligible,
    Excluded(ExclusionReason),
}

/// Explicit declarations take precedence over lesson completion, but not hidden state.
pub fn vocabulary_eligibility(progress: Option<&LearnerProgress>) -> KnowledgeDecision {
    use KnowledgeDecision::{Eligible, Excluded};
    let Some(progress) = progress else {
        return Excluded(ExclusionReason::NoProgress);
    };
    if progress.is_hidden {
        return Excluded(ExclusionReason::Hidden);
    }
    if let Some(known) = progress.is_marked_known {
        return if known {
            Eligible
        } else {
            Excluded(ExclusionReason::MarkedUnknown)
        };
    }
    if progress.learning_started_at.is_some() {
        Eligible
    } else {
        Excluded(ExclusionReason::NoRecordedLessonStart)
    }
}
