use chrono::{TimeZone, Utc};
use yomibu_core::domain::knowledge::{
    ExclusionReason, KnowledgeDecision, LearnerProgress, vocabulary_eligibility,
};

#[test]
fn eligibility_requires_lesson_start_even_after_a_recorded_pass() {
    use KnowledgeDecision::{Eligible, Excluded};
    let mut progress = LearnerProgress::default();
    assert_eq!(
        vocabulary_eligibility(None),
        Excluded(ExclusionReason::NoProgress)
    );
    assert_eq!(
        vocabulary_eligibility(Some(&progress)),
        Excluded(ExclusionReason::NoRecordedLessonStart)
    );
    progress.learning_started_at = Some(Utc.timestamp_opt(1, 0).unwrap());
    assert_eq!(vocabulary_eligibility(Some(&progress)), Eligible);
    progress.first_passed_at = Some(Utc.timestamp_opt(2, 0).unwrap());
    progress.learning_started_at = None;
    assert_eq!(
        vocabulary_eligibility(Some(&progress)),
        Excluded(ExclusionReason::NoRecordedLessonStart)
    );
}

#[test]
fn explicit_declarations_override_milestones_but_not_hidden_progress() {
    use KnowledgeDecision::{Eligible, Excluded};
    let mut progress = LearnerProgress {
        is_marked_known: Some(true),
        ..Default::default()
    };
    assert_eq!(vocabulary_eligibility(Some(&progress)), Eligible);
    progress.is_marked_known = Some(false);
    progress.learning_started_at = Some(Utc.timestamp_opt(1, 0).unwrap());
    progress.first_passed_at = progress.learning_started_at;
    assert_eq!(
        vocabulary_eligibility(Some(&progress)),
        Excluded(ExclusionReason::MarkedUnknown)
    );
    progress.is_marked_known = Some(true);
    progress.is_hidden = true;
    assert_eq!(
        vocabulary_eligibility(Some(&progress)),
        Excluded(ExclusionReason::Hidden)
    );
}
