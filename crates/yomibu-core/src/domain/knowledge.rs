//! Revisable, deterministic eligibility derived from preserved learner evidence.

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LearnerKnowledgePolicy {
    #[default]
    LessonStarted,
    RecordedPass,
}

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
    NoRecordedPass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeDecision {
    Eligible,
    Excluded(ExclusionReason),
}

impl LearnerKnowledgePolicy {
    /// Hidden observations are excluded. Explicit declarations take precedence
    /// over the selected lifecycle milestone; none of these establish mastery.
    pub fn decide(&self, progress: Option<&LearnerProgress>) -> KnowledgeDecision {
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
        let (recorded, missing) = match self {
            Self::LessonStarted => (
                progress.learning_started_at,
                ExclusionReason::NoRecordedLessonStart,
            ),
            Self::RecordedPass => (progress.first_passed_at, ExclusionReason::NoRecordedPass),
        };
        if recorded.is_some() {
            Eligible
        } else {
            Excluded(missing)
        }
    }
}
