use yomibu::domain::WaniKaniSyncData;
use yomibu::grammar::GrammarDeclarations;
use yomibu::knowledge::{
    ExclusionReason, KnowledgeDecision, LearnerKnowledgePolicy, WaniKaniKnowledgeRule,
};
use yomibu::preparation::{PracticeTarget, PrepareError, UnassessedAspect, prepare_context};

fn source() -> WaniKaniSyncData {
    #[derive(serde::Deserialize)]
    struct Envelope {
        snapshot: WaniKaniSyncData,
    }
    serde_json::from_str::<Envelope>(include_str!("fixtures/mixed.json"))
        .unwrap()
        .snapshot
}

#[test]
fn derives_explainable_knowledge_without_changing_source_observations() {
    use ExclusionReason::{ContentUnavailable, Hidden, NoAssignment, NoRecordedPass};
    use KnowledgeDecision::{Eligible, Excluded};

    let data = source();
    let before = serde_json::to_value(&data).unwrap();
    let grammar = GrammarDeclarations::from_descriptions(["です", "です"]).unwrap();
    let policy = LearnerKnowledgePolicy::default();
    let known = policy.derive(&data, &grammar).unwrap();
    assert_eq!(known.policy.wanikani, WaniKaniKnowledgeRule::LessonStarted);
    assert_eq!(known.learner_id, data.learner.id);
    assert_eq!(known.sync_started_at, data.sync_started_at);
    assert_eq!(known.sync_completed_at, data.sync_completed_at);
    assert_eq!(known.grammar, grammar.entries());
    let decisions: Vec<_> = known
        .materials
        .iter()
        .map(|entry| (entry.subject_id, entry.decision))
        .collect();
    assert_eq!(
        decisions,
        vec![
            (1, Excluded(Hidden)),
            (2, Excluded(Hidden)),
            (3, Eligible),
            (4, Excluded(NoAssignment)),
            (5, Excluded(ContentUnavailable)),
        ]
    );
    let eligible = &known.materials[2];
    assert_eq!(eligible.material.unwrap().characters, "これ");
    assert_eq!(eligible.assignment.unwrap().id, 103);
    assert!(eligible.review_statistic.is_none());
    assert!(known.materials[4].material.is_none());
    let passed = LearnerKnowledgePolicy {
        wanikani: WaniKaniKnowledgeRule::RecordedPass,
    }
    .derive(&data, &grammar)
    .unwrap();
    assert_eq!(passed.materials[2].decision, Excluded(NoRecordedPass));
    assert_eq!(serde_json::to_value(&data).unwrap(), before);
}

#[test]
fn lifecycle_evidence_not_stage_accuracy_or_burned_state_controls_eligibility() {
    use KnowledgeDecision::{Eligible, Excluded};

    let mut data = source();
    data.subjects[0].hidden_at = None;
    data.assignments[1].hidden = false;
    data.assignments[2].srs_stage = 999;
    let grammar = GrammarDeclarations::from_descriptions(Vec::<String>::new()).unwrap();
    let policy = LearnerKnowledgePolicy::default();
    let known = policy.derive(&data, &grammar).unwrap();
    assert_eq!(
        known.materials[0].decision,
        Excluded(ExclusionReason::NoRecordedLessonStart)
    );
    assert_eq!(known.materials[1].decision, Eligible);
    assert!(known.materials[1].assignment.unwrap().burned_at.is_some());
    assert_eq!(known.materials[2].decision, Eligible);
    let passed = LearnerKnowledgePolicy {
        wanikani: WaniKaniKnowledgeRule::RecordedPass,
    }
    .derive(&data, &grammar)
    .unwrap();
    assert_eq!(passed.materials[1].decision, Eligible);
    data.review_statistics[1].hidden = true;
    assert_eq!(
        policy.derive(&data, &grammar).unwrap().materials[1].decision,
        Excluded(ExclusionReason::Hidden)
    );
    data.assignments[2].started_at = None;
    assert_eq!(
        policy.derive(&data, &grammar).unwrap().materials[2].decision,
        Excluded(ExclusionReason::NoRecordedLessonStart)
    );
}

#[test]
fn policy_order_is_stable_and_invalid_direct_input_is_rejected() {
    let mut data = source();
    let grammar = GrammarDeclarations::from_descriptions(Vec::<String>::new()).unwrap();
    let policy = LearnerKnowledgePolicy::default();
    data.subjects.reverse();
    data.assignments.reverse();
    let known = policy.derive(&data, &grammar).unwrap();
    let ids: Vec<_> = known
        .materials
        .iter()
        .map(|entry| entry.subject_id)
        .collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);
    data.subjects.clear();
    assert!(policy.derive(&data, &grammar).is_err());
}

fn practice_source() -> WaniKaniSyncData {
    let mut data = source();
    data.assignments[1].hidden = false;
    data
}

fn target(word: &str, reading: &str, sense: &str) -> PracticeTarget {
    PracticeTarget {
        word: word.into(),
        intended_reading: reading.into(),
        intended_sense: sense.into(),
    }
}

#[test]
fn retrieves_requested_uses_and_attached_examples_with_explicit_assessment_limits() {
    let data = practice_source();
    let grammar = GrammarDeclarations::from_descriptions(["です"]).unwrap();
    let policy = LearnerKnowledgePolicy::default();
    let targets = [
        target("一つ", "ひとつ", "one thing"),
        target("一つ", "ひとつ", "one thing"),
    ];
    let result = prepare_context(&data, &grammar, &policy, &targets).unwrap();
    assert_eq!(result.targets.len(), 2);
    for (selected, requested) in result.targets.iter().zip(&targets) {
        assert_eq!(selected.target, requested);
        assert_eq!(selected.subject.id, 2);
        assert_eq!(selected.assignment.id, 102);
        assert_eq!(selected.examples[0].japanese, "一つあります。");
        assert_eq!(selected.examples[0].english, "There is one.");
    }
    assert_eq!(result.knowledge.grammar, grammar.entries());
    assert_eq!(
        result.unassessed,
        &[
            UnassessedAspect::Grammar,
            UnassessedAspect::ReadingSenseAssociation,
            UnassessedAspect::ExampleSuitability,
            UnassessedAspect::LinguisticCorrectness,
        ]
    );
}

#[test]
fn rejects_missing_unsupported_and_policy_ineligible_targets_without_declaring_them_known() {
    let data = practice_source();
    let grammar = GrammarDeclarations::from_descriptions(Vec::<String>::new()).unwrap();
    let policy = LearnerKnowledgePolicy::default();
    for word in ["not in this cache", "一", "一二"] {
        let targets = [target(word, "いち", "one")];
        assert!(matches!(
            prepare_context(&data, &grammar, &policy, &targets),
            Err(PrepareError::TargetNotFound { entry: 1 })
        ));
    }
    for entry in [
        target("一つ", "いち", "one thing"),
        target("一つ", "ひとつ", "a single thing"),
        target("一つ", " ひとつ", "one thing"),
        target("これ", "これ", "this"),
    ] {
        assert!(matches!(
            prepare_context(&data, &grammar, &policy, &[entry]),
            Err(PrepareError::UnsupportedUse { entry: 1 })
        ));
    }
    let hidden_data = source();
    let targets = [target("一つ", "ひとつ", "one thing")];
    assert!(matches!(
        prepare_context(&hidden_data, &grammar, &policy, &targets),
        Err(PrepareError::IneligibleTarget {
            entry: 1,
            subject_id: 2,
            reason: ExclusionReason::Hidden,
        })
    ));
}

#[test]
fn validates_all_structured_targets_and_source_data_before_returning_a_result() {
    let mut data = practice_source();
    let grammar = GrammarDeclarations::from_descriptions(Vec::<String>::new()).unwrap();
    let policy = LearnerKnowledgePolicy::default();
    assert!(matches!(
        prepare_context(&data, &grammar, &policy, &[]),
        Err(PrepareError::EmptyTargets)
    ));
    for field in ["word", "intended_reading", "intended_sense"] {
        let mut invalid = target("一つ", "ひとつ", "one thing");
        match field {
            "word" => invalid.word = " \t".into(),
            "intended_reading" => invalid.intended_reading = "　".into(),
            _ => invalid.intended_sense = "\n".into(),
        }
        let targets = [target("一つ", "ひとつ", "one thing"), invalid];
        assert!(matches!(
            prepare_context(&data, &grammar, &policy, &targets),
            Err(PrepareError::BlankTargetField { entry: 2, field: actual }) if actual == field
        ));
    }
    data.subjects.clear();
    let targets = [target("一つ", "ひとつ", "one thing")];
    assert!(matches!(
        prepare_context(&data, &grammar, &policy, &targets),
        Err(PrepareError::InvalidSourceData(_))
    ));
}

#[test]
fn uses_accepted_fields_only_and_does_not_assert_reading_sense_associations() {
    use yomibu::domain::{LexicalContent, Meaning, Reading};

    let grammar = GrammarDeclarations::from_descriptions(Vec::<String>::new()).unwrap();
    let policy = LearnerKnowledgePolicy::default();
    for rejected_field in ["reading", "meaning"] {
        let mut data = practice_source();
        if rejected_field == "meaning" {
            data.subjects[1].meanings[0].accepted_answer = false;
        } else if let LexicalContent::Vocabulary { readings, .. } = &mut data.subjects[1].lexical {
            readings[0].accepted_answer = false;
        }
        let targets = [target("一つ", "ひとつ", "one thing")];
        assert!(matches!(
            prepare_context(&data, &grammar, &policy, &targets),
            Err(PrepareError::UnsupportedUse { entry: 1 })
        ));
    }
    let mut data = practice_source();
    data.subjects[1].meanings.push(Meaning {
        meaning: "another source gloss".into(),
        primary: false,
        accepted_answer: true,
    });
    if let LexicalContent::Vocabulary {
        readings,
        context_sentences,
        ..
    } = &mut data.subjects[1].lexical
    {
        readings.push(Reading {
            reading: "another source reading".into(),
            primary: false,
            accepted_answer: true,
        });
        context_sentences.clear();
    }
    let targets = [target("一つ", "ひとつ", "another source gloss")];
    let result = prepare_context(&data, &grammar, &policy, &targets).unwrap();
    assert_eq!(result.targets.len(), 1);
    assert_eq!(result.targets[0].target, &targets[0]);
    assert!(result.targets[0].examples.is_empty());
    assert!(
        result
            .unassessed
            .contains(&UnassessedAspect::ReadingSenseAssociation)
    );
}

#[test]
fn ambiguous_source_records_are_not_disambiguated_by_learner_eligibility() {
    let mut data = practice_source();
    let mut other = source();
    let mut subject = other.subjects.remove(1);
    let mut assignment = other.assignments.remove(1);
    subject.id = 6;
    assignment.id = 106;
    assignment.subject_id = 6;
    data.subjects.push(subject);
    data.assignments.push(assignment);
    let grammar = GrammarDeclarations::from_descriptions(Vec::<String>::new()).unwrap();
    let targets = [target("一つ", "ひとつ", "one thing")];
    let policy = LearnerKnowledgePolicy::default();
    let result = prepare_context(&data, &grammar, &policy, &targets);
    assert!(matches!(
        result,
        Err(PrepareError::AmbiguousTarget { entry: 1, subject_ids }) if subject_ids == [2, 6]
    ));
}
