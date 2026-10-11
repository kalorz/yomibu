use std::collections::BTreeMap;
use yomibu_core::domain::{
    inventory::{ExcludedMaterial, InventoryError, InventoryWord, LearnerInventory, SourceOrigin},
    knowledge::{ExclusionReason, KnowledgeDecision, LearnerKnowledgePolicy, LearnerProgress},
    source::{
        Assignment, LexicalContent, ReviewStatistic, Subject, ValidationError, WaniKaniSyncData,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error(transparent)]
    Source(#[from] ValidationError),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
}

pub fn project_inventory(
    source: &WaniKaniSyncData,
    policy: &LearnerKnowledgePolicy,
) -> Result<LearnerInventory, ProjectionError> {
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
    let mut subjects: BTreeMap<_, _> = source
        .subjects
        .iter()
        .map(|subject| (subject.id, Some(subject)))
        .collect();
    for unavailable in &source.unavailable_subjects {
        subjects.insert(unavailable.id, None);
    }
    let mut inventory = LearnerInventory {
        vocabulary: Vec::new(),
        grammar_declarations: Vec::new(),
        grammar_bindings: Vec::new(),
        excluded: Vec::new(),
    };
    for (subject_id, subject) in subjects {
        let origin = SourceOrigin::Wanikani { subject_id };
        let Some(subject) = subject else {
            inventory.excluded.push(ExcludedMaterial {
                origin,
                reason: ExclusionReason::ContentUnavailable,
            });
            continue;
        };
        let assignment = assignments.get(&subject_id).copied();
        let statistic = statistics.get(&subject_id).copied();
        let progress = learner_progress(subject, assignment, statistic);
        if let KnowledgeDecision::Excluded(reason) = policy.decide(progress.as_ref()) {
            inventory.excluded.push(ExcludedMaterial { origin, reason });
            continue;
        }
        let readings = match &subject.lexical {
            LexicalContent::Kanji { .. } => continue,
            LexicalContent::Vocabulary { readings, .. } => readings
                .iter()
                .filter(|reading| reading.accepted_answer)
                .map(|reading| reading.reading.clone())
                .collect(),
            LexicalContent::KanaVocabulary { .. } => Vec::new(),
        };
        inventory.vocabulary.push(InventoryWord {
            id: format!("wanikani:{subject_id}"),
            written_form: subject.characters.clone(),
            readings,
            meanings: subject
                .meanings
                .iter()
                .filter(|meaning| meaning.accepted_answer)
                .map(|meaning| meaning.meaning.clone())
                .collect(),
            direct_object: None,
            origin,
        });
    }
    inventory.validate()?;
    Ok(inventory)
}

fn learner_progress(
    subject: &Subject,
    assignment: Option<&Assignment>,
    statistic: Option<&ReviewStatistic>,
) -> Option<LearnerProgress> {
    let is_hidden = subject.hidden_at.is_some()
        || assignment.is_some_and(|entry| entry.hidden)
        || statistic.is_some_and(|entry| entry.hidden);
    (assignment.is_some() || is_hidden).then(|| LearnerProgress {
        is_marked_known: None,
        learning_started_at: assignment.and_then(|entry| entry.started_at),
        first_passed_at: assignment.and_then(|entry| entry.passed_at),
        is_hidden,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_progress_preserves_hidden_evidence_from_each_wanikani_record() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../tests/fixtures/mixed.json")).unwrap();
        for hidden_in in 0..3 {
            let mut data: WaniKaniSyncData =
                serde_json::from_value(fixture["snapshot"].clone()).unwrap();
            let subject = &mut data.subjects[1];
            let assignment = &mut data.assignments[1];
            let statistic = &mut data.review_statistics[1];
            subject.hidden_at = (hidden_in == 0).then_some(data.sync_completed_at);
            assignment.hidden = hidden_in == 1;
            statistic.hidden = hidden_in == 2;
            let progress = learner_progress(subject, Some(assignment), Some(statistic)).unwrap();
            assert!(progress.is_hidden, "hidden source {hidden_in}");
            assert_eq!(progress.learning_started_at, assignment.started_at);
            assert_eq!(progress.first_passed_at, assignment.passed_at);
            assert_eq!(
                LearnerKnowledgePolicy::default().decide(Some(&progress)),
                KnowledgeDecision::Excluded(ExclusionReason::Hidden)
            );
            let without_assignment = learner_progress(subject, None, Some(statistic));
            if hidden_in == 1 {
                assert_eq!(without_assignment, None);
            } else {
                let progress = without_assignment.unwrap();
                assert!(progress.is_hidden);
                assert_eq!(progress.learning_started_at, None);
                assert_eq!(progress.first_passed_at, None);
            }
        }
    }
}
