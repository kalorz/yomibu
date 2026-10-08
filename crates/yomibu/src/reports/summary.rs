//! Deterministic summaries of validated cached observations.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use yomibu_core::domain::source::{SubjectKind, ValidationError, WaniKaniSyncData};

#[derive(Debug, PartialEq, Serialize)]
pub struct Summary {
    pub username: String,
    pub level: u32,
    pub sync_started_at: DateTime<Utc>,
    pub sync_completed_at: DateTime<Utc>,
    pub kanji: usize,
    /// Includes kana-only vocabulary.
    pub vocabulary: usize,
    pub kana_vocabulary: usize,
    /// Distinct subject IDs hidden in any retained source record.
    pub hidden_subjects: usize,
    pub unavailable_content: usize,
    /// Assignment counts by source SRS system and raw stage; None means content is excluded.
    #[serde(serialize_with = "serialize_srs_stages")]
    pub srs_stages: BTreeMap<Option<u64>, BTreeMap<u32, usize>>,
    pub reading_accuracy: Accuracy,
    pub meaning_accuracy: Accuracy,
}

fn serialize_srs_stages<S: serde::Serializer>(
    groups: &BTreeMap<Option<u64>, BTreeMap<u32, usize>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Group<'a> {
        srs_system_id: Option<u64>,
        stages: &'a BTreeMap<u32, usize>,
    }
    groups
        .iter()
        .map(|(&srs_system_id, stages)| Group {
            srs_system_id,
            stages,
        })
        .collect::<Vec<_>>()
        .serialize(serializer)
}

/// Validate and summarize source state into a result independent of the
/// input's lifetime. Accuracy uses aggregate counters, not averaged percentages.
pub fn summarize(data: &WaniKaniSyncData) -> Result<Summary, ValidationError> {
    data.validate()?;
    let mut summary = Summary {
        username: data.learner.username.clone(),
        level: data.learner.level,
        sync_started_at: data.sync_started_at,
        sync_completed_at: data.sync_completed_at,
        kanji: 0,
        vocabulary: 0,
        kana_vocabulary: 0,
        hidden_subjects: 0,
        unavailable_content: data.unavailable_subjects.len(),
        srs_stages: BTreeMap::new(),
        reading_accuracy: Accuracy::default(),
        meaning_accuracy: Accuracy::default(),
    };
    let mut hidden = BTreeSet::new();
    for subject in &data.subjects {
        match subject.kind() {
            SubjectKind::Kanji => summary.kanji += 1,
            SubjectKind::Vocabulary => summary.vocabulary += 1,
            SubjectKind::KanaVocabulary => {
                summary.vocabulary += 1;
                summary.kana_vocabulary += 1;
            }
        }
        if subject.hidden_at.is_some() {
            hidden.insert(subject.id);
        }
    }
    let systems: BTreeMap<_, _> = data
        .subjects
        .iter()
        .map(|s| (s.id, s.srs_system_id))
        .collect();
    for assignment in &data.assignments {
        let system = systems.get(&assignment.subject_id).copied();
        *summary
            .srs_stages
            .entry(system)
            .or_default()
            .entry(assignment.srs_stage)
            .or_default() += 1;
        if assignment.hidden {
            hidden.insert(assignment.subject_id);
        }
    }
    for statistic in &data.review_statistics {
        summary
            .reading_accuracy
            .add(statistic.reading_correct, statistic.reading_incorrect);
        summary
            .meaning_accuracy
            .add(statistic.meaning_correct, statistic.meaning_incorrect);
        if statistic.hidden {
            hidden.insert(statistic.subject_id);
        }
    }
    summary.hidden_subjects = hidden.len();
    Ok(summary)
}

/// Exact aggregate counts; a missing percentage means no answers were recorded.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct Accuracy {
    correct: u128,
    total: u128,
}

impl Accuracy {
    pub fn correct(&self) -> u128 {
        self.correct
    }

    pub fn total(&self) -> u128 {
        self.total
    }

    pub fn percentage(&self) -> Option<f64> {
        if self.total == 0 {
            None
        } else {
            Some(100.0 * self.correct as f64 / self.total as f64)
        }
    }

    fn add(&mut self, correct: u64, incorrect: u64) {
        // Widen before addition so even maximum source counters can be aggregated.
        self.correct += u128::from(correct);
        self.total += u128::from(correct) + u128::from(incorrect);
    }
}
