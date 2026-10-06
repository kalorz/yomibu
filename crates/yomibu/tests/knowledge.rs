use yomibu::domain::WaniKaniSyncData;
use yomibu::knowledge::{
    ExclusionReason, KnowledgeDecision, LearnerKnowledgePolicy, WaniKaniKnowledgeRule,
};

fn source() -> WaniKaniSyncData {
    #[derive(serde::Deserialize)]
    struct Envelope {
        snapshot: WaniKaniSyncData,
    }
    serde_json::from_str::<Envelope>(include_str!("../../../tests/fixtures/mixed.json"))
        .unwrap()
        .snapshot
}

#[test]
fn derives_explainable_knowledge_without_changing_source_observations() {
    use ExclusionReason::{ContentUnavailable, Hidden, NoAssignment, NoRecordedPass};
    use KnowledgeDecision::{Eligible, Excluded};

    let data = source();
    let before = serde_json::to_value(&data).unwrap();
    let policy = LearnerKnowledgePolicy::default();
    let known = policy.derive(&data).unwrap();
    assert_eq!(known.policy.wanikani, WaniKaniKnowledgeRule::LessonStarted);
    assert_eq!(known.learner_id, data.learner.id);
    assert_eq!(known.sync_started_at, data.sync_started_at);
    assert_eq!(known.sync_completed_at, data.sync_completed_at);
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
    .derive(&data)
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
    let policy = LearnerKnowledgePolicy::default();
    let known = policy.derive(&data).unwrap();
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
    .derive(&data)
    .unwrap();
    assert_eq!(passed.materials[1].decision, Eligible);
    data.review_statistics[1].hidden = true;
    assert_eq!(
        policy.derive(&data).unwrap().materials[1].decision,
        Excluded(ExclusionReason::Hidden)
    );
    data.assignments[2].started_at = None;
    assert_eq!(
        policy.derive(&data).unwrap().materials[2].decision,
        Excluded(ExclusionReason::NoRecordedLessonStart)
    );
}

#[test]
fn policy_order_is_stable_and_invalid_direct_input_is_rejected() {
    let mut data = source();
    let policy = LearnerKnowledgePolicy::default();
    data.subjects.reverse();
    data.assignments.reverse();
    let known = policy.derive(&data).unwrap();
    let ids: Vec<_> = known
        .materials
        .iter()
        .map(|entry| entry.subject_id)
        .collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);
    data.subjects.clear();
    assert!(policy.derive(&data).is_err());
}
