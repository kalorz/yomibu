//! Owned learner observations and lexical data, independent of CLI and transport.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Normalized data collected for one WaniKani account during a synchronization.
/// Includes progress and its associated accessible material, not the full catalog
/// or an instantaneous view of the remote system. It does not define "known".
/// Construction and deserialization alone do not validate the public fields;
/// use [`Self::validate`] before consuming them directly.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct WaniKaniSyncData {
    pub sync_started_at: DateTime<Utc>,
    pub sync_completed_at: DateTime<Utc>,
    pub learner: Learner,
    pub subjects: Vec<Subject>,
    pub unavailable_subjects: Vec<UnavailableSubject>,
    pub assignments: Vec<Assignment>,
    pub review_statistics: Vec<ReviewStatistic>,
}

/// WaniKani account information; its ID belongs to WaniKani, not to Yomibu.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Learner {
    pub id: String,
    pub username: String,
    pub level: u32,
    pub updated_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub current_vacation_started_at: Option<DateTime<Utc>>,
    pub subscription: Subscription,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Subscription {
    pub active: bool,
    pub kind: String,
    pub max_level_granted: u32,
    pub period_ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Kanji,
    Vocabulary,
    KanaVocabulary,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Subject {
    pub id: u64,
    pub level: u32,
    pub characters: String,
    pub meanings: Vec<Meaning>,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub hidden_at: Option<DateTime<Utc>>,
    pub srs_system_id: u64,
    pub lexical: LexicalContent,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LexicalContent {
    Kanji {
        readings: Vec<KanjiReading>,
    },
    Vocabulary {
        readings: Vec<Reading>,
        parts_of_speech: Vec<String>,
        context_sentences: Vec<ContextSentence>,
    },
    KanaVocabulary {
        parts_of_speech: Vec<String>,
        context_sentences: Vec<ContextSentence>,
    },
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Meaning {
    pub meaning: String,
    pub primary: bool,
    pub accepted_answer: bool,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Reading {
    pub reading: String,
    pub primary: bool,
    pub accepted_answer: bool,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct KanjiReading {
    pub reading: String,
    pub primary: bool,
    pub accepted_answer: bool,
    pub kind: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ContextSentence {
    pub japanese: String,
    pub english: String,
}

/// An explicit content-access exclusion, never an unexplained missing subject.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UnavailableSubject {
    pub id: u64,
    pub kind: SubjectKind,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Assignment {
    pub id: u64,
    pub subject_id: u64,
    pub subject_kind: SubjectKind,
    pub srs_stage: u32,
    pub hidden: bool,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub unlocked_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub passed_at: Option<DateTime<Utc>>,
    pub burned_at: Option<DateTime<Utc>>,
    pub available_at: Option<DateTime<Utc>>,
    pub resurrected_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReviewStatistic {
    pub id: u64,
    pub subject_id: u64,
    pub subject_kind: SubjectKind,
    pub hidden: bool,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub reading_correct: u64,
    pub reading_incorrect: u64,
    pub reading_current_streak: u64,
    pub reading_max_streak: u64,
    pub meaning_correct: u64,
    pub meaning_incorrect: u64,
    pub meaning_current_streak: u64,
    pub meaning_max_streak: u64,
    pub percentage_correct: u32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("invalid {field}")]
    InvalidField { field: &'static str },
    #[error("duplicate {collection} identity {id}")]
    Duplicate { collection: &'static str, id: u64 },
    #[error("subject {id} is both available and excluded")]
    ContentConflict { id: u64 },
    #[error("progress references missing subject {id}")]
    MissingSubject { id: u64 },
    #[error("progress kind does not match subject {id}")]
    KindMismatch { id: u64 },
    #[error("subject {id} has no progress reference")]
    UnreferencedSubject { id: u64 },
}

impl Subject {
    pub fn validate(&self, max_level: u32) -> Result<(), ValidationError> {
        require(self.id > 0, "subject.id")?;
        require(
            self.level > 0 && self.level <= max_level,
            "subject.level/content access",
        )?;
        require(self.srs_system_id > 0, "subject.srs_system_id")?;
        require(!self.characters.trim().is_empty(), "subject.characters")?;
        require(
            !self.meanings.is_empty() && self.meanings.iter().all(|m| !m.meaning.trim().is_empty()),
            "subject.meanings",
        )?;
        match &self.lexical {
            LexicalContent::Kanji { readings } => {
                require(
                    !readings.is_empty()
                        && readings
                            .iter()
                            .all(|r| !r.reading.trim().is_empty() && !r.kind.trim().is_empty()),
                    "kanji.readings",
                )?;
            }
            LexicalContent::Vocabulary { readings, .. } => {
                require(
                    !readings.is_empty() && readings.iter().all(|r| !r.reading.trim().is_empty()),
                    "vocabulary.readings",
                )?;
            }
            LexicalContent::KanaVocabulary { .. } => {}
        }
        Ok(())
    }

    pub fn kind(&self) -> SubjectKind {
        match self.lexical {
            LexicalContent::Kanji { .. } => SubjectKind::Kanji,
            LexicalContent::Vocabulary { .. } => SubjectKind::Vocabulary,
            LexicalContent::KanaVocabulary { .. } => SubjectKind::KanaVocabulary,
        }
    }
}

impl WaniKaniSyncData {
    /// Validate normalized cache data before using it for summaries.
    pub fn validate(&self) -> Result<(), ValidationError> {
        use std::collections::{BTreeMap, BTreeSet};
        require(!self.learner.id.trim().is_empty(), "learner.id")?;
        require(
            !self.learner.username.trim().is_empty()
                && !self.learner.username.chars().any(char::is_control),
            "learner.username",
        )?;
        require((1..=60).contains(&self.learner.level), "learner.level")?;
        require(
            (1..=60).contains(&self.learner.subscription.max_level_granted),
            "subscription.max_level_granted",
        )?;
        require(
            !self.learner.subscription.kind.trim().is_empty(),
            "subscription.kind",
        )?;
        require(
            self.sync_started_at <= self.sync_completed_at,
            "synchronization interval",
        )?;

        let mut subjects = BTreeMap::new();
        for subject in &self.subjects {
            subject.validate(self.learner.subscription.max_level_granted)?;
            if subjects.insert(subject.id, subject.kind()).is_some() {
                return Err(ValidationError::Duplicate {
                    collection: "subjects",
                    id: subject.id,
                });
            }
        }
        let mut excluded = BTreeSet::new();
        for subject in &self.unavailable_subjects {
            require(subject.id > 0, "unavailable_subject.id")?;
            if !excluded.insert(subject.id) {
                return Err(ValidationError::Duplicate {
                    collection: "unavailable subjects",
                    id: subject.id,
                });
            }
            if subjects.insert(subject.id, subject.kind).is_some() {
                return Err(ValidationError::ContentConflict { id: subject.id });
            }
        }
        let mut referenced = BTreeSet::new();
        let mut check_reference = |id, kind| {
            match subjects.get(&id) {
                None => return Err(ValidationError::MissingSubject { id }),
                Some(expected) if *expected != kind => {
                    return Err(ValidationError::KindMismatch { id });
                }
                Some(_) => {}
            }
            referenced.insert(id);
            Ok(())
        };
        let mut assignment_ids = BTreeSet::new();
        let mut assigned_subjects = BTreeSet::new();
        for assignment in &self.assignments {
            unique_id(&mut assignment_ids, assignment.id, "assignments")?;
            unique_id(
                &mut assigned_subjects,
                assignment.subject_id,
                "assignment subjects",
            )?;
            check_reference(assignment.subject_id, assignment.subject_kind)?;
        }
        let mut statistic_ids = BTreeSet::new();
        let mut reviewed_subjects = BTreeSet::new();
        for statistic in &self.review_statistics {
            unique_id(&mut statistic_ids, statistic.id, "review statistics")?;
            unique_id(
                &mut reviewed_subjects,
                statistic.subject_id,
                "review statistic subjects",
            )?;
            require(
                statistic.percentage_correct <= 100,
                "review_statistic.percentage_correct",
            )?;
            check_reference(statistic.subject_id, statistic.subject_kind)?;
        }
        for id in subjects.keys() {
            if !referenced.contains(id) {
                return Err(ValidationError::UnreferencedSubject { id: *id });
            }
        }
        Ok(())
    }
}

fn require(condition: bool, field: &'static str) -> Result<(), ValidationError> {
    if condition {
        Ok(())
    } else {
        Err(ValidationError::InvalidField { field })
    }
}

fn unique_id(
    seen: &mut std::collections::BTreeSet<u64>,
    id: u64,
    collection: &'static str,
) -> Result<(), ValidationError> {
    require(id > 0, collection)?;
    if seen.insert(id) {
        Ok(())
    } else {
        Err(ValidationError::Duplicate { collection, id })
    }
}
