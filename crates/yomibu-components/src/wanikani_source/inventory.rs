use super::Error;
use std::collections::BTreeMap;
use yomibu_core::domain::{
    inventory::{ExcludedMaterial, InventoryWord, LearnerInventory, SourceOrigin},
    knowledge::{ExclusionReason, KnowledgeDecision, LearnerKnowledgePolicy, LearnerProgress},
    source::{LexicalContent, WaniKaniSyncData},
};

pub fn prepare_inventory(
    source: &WaniKaniSyncData,
    policy: &LearnerKnowledgePolicy,
) -> Result<LearnerInventory, Error> {
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
        let assignment = assignments.get(&subject_id);
        let statistic = statistics.get(&subject_id);
        let progress = assignment.map(|entry| LearnerProgress {
            is_marked_known: None,
            learning_started_at: entry.started_at,
            first_passed_at: entry.passed_at,
            is_hidden: entry.hidden,
        });
        let decision = if subject.is_none() {
            KnowledgeDecision::Excluded(ExclusionReason::ContentUnavailable)
        } else if subject.is_some_and(|entry| entry.hidden_at.is_some())
            || statistic.is_some_and(|entry| entry.hidden)
        {
            KnowledgeDecision::Excluded(ExclusionReason::Hidden)
        } else {
            policy.decide(progress.as_ref())
        };
        if let KnowledgeDecision::Excluded(reason) = decision {
            inventory
                .excluded
                .push(ExcludedMaterial { subject_id, reason });
            continue;
        }
        let Some(subject) = subject else {
            continue;
        };
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
            origin: SourceOrigin::Wanikani { subject_id },
        });
    }
    inventory.validate()?;
    Ok(inventory)
}
