use chrono::{TimeZone, Utc};
use yomibu_core::domain::knowledge::{
    ExclusionReason, KnowledgeDecision, LearnerKnowledgePolicy, LearnerProgress,
};

#[test]
fn policy_distinguishes_missing_progress_learning_start_and_first_pass() {
    use KnowledgeDecision::{Eligible, Excluded};
    let lesson = LearnerKnowledgePolicy::LessonStarted;
    let passed = LearnerKnowledgePolicy::RecordedPass;
    let mut progress = LearnerProgress::default();
    assert_eq!(lesson.decide(None), Excluded(ExclusionReason::NoProgress));
    assert_eq!(
        lesson.decide(Some(&progress)),
        Excluded(ExclusionReason::NoRecordedLessonStart)
    );
    progress.learning_started_at = Some(Utc.timestamp_opt(1, 0).unwrap());
    assert_eq!(lesson.decide(Some(&progress)), Eligible);
    assert_eq!(
        passed.decide(Some(&progress)),
        Excluded(ExclusionReason::NoRecordedPass)
    );
    progress.first_passed_at = Some(Utc.timestamp_opt(2, 0).unwrap());
    assert_eq!(passed.decide(Some(&progress)), Eligible);
    progress.learning_started_at = None;
    assert_eq!(passed.decide(Some(&progress)), Eligible);
    assert_eq!(
        lesson.decide(Some(&progress)),
        Excluded(ExclusionReason::NoRecordedLessonStart)
    );
}

#[test]
fn explicit_declarations_override_milestones_but_not_hidden_progress() {
    use KnowledgeDecision::{Eligible, Excluded};
    for policy in [
        LearnerKnowledgePolicy::LessonStarted,
        LearnerKnowledgePolicy::RecordedPass,
    ] {
        let mut progress = LearnerProgress {
            is_marked_known: Some(true),
            ..Default::default()
        };
        assert_eq!(policy.decide(Some(&progress)), Eligible);
        progress.is_marked_known = Some(false);
        progress.learning_started_at = Some(Utc.timestamp_opt(1, 0).unwrap());
        progress.first_passed_at = progress.learning_started_at;
        assert_eq!(
            policy.decide(Some(&progress)),
            Excluded(ExclusionReason::MarkedUnknown)
        );
        progress.is_marked_known = Some(true);
        progress.is_hidden = true;
        assert_eq!(
            policy.decide(Some(&progress)),
            Excluded(ExclusionReason::Hidden)
        );
    }
}
