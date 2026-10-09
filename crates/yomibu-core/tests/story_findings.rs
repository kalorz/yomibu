use yomibu_core::domain::{
    evaluation::{
        Check, CheckOutcome, CheckState, Evaluation, EvaluationBasis, EvaluationError, Finding,
    },
    story::{PlanDeparture, StoryFindings, TargetKind, TargetObservation},
};

fn evaluation(text: &str) -> Evaluation {
    let pass = || Check {
        state: CheckState::Completed(CheckOutcome::Pass),
        findings: vec![],
        coverage: "test",
    };
    Evaluation::new(
        text,
        EvaluationBasis::FullLearnerInventory,
        pass(),
        pass(),
        pass(),
        pass(),
        Check {
            state: CheckState::Completed(CheckOutcome::Inconclusive),
            findings: vec![Finding {
                span: 0..text.len(),
                reason: "test",
            }],
            coverage: "test",
        },
    )
    .unwrap()
}

#[test]
fn story_findings_check_all_output_spans_against_original_text() {
    for (evaluation, targets, departures) in [
        (evaluation("猫です。"), vec![], vec![]),
        (
            evaluation("猫。"),
            vec![TargetObservation::from_evidence(
                TargetKind::Vocabulary,
                "cat",
                std::iter::once(1..3).collect(),
                vec![],
            )],
            vec![],
        ),
        (
            evaluation("猫。"),
            vec![],
            vec![PlanDeparture {
                span: 0..9,
                inventory_entries: vec!["dog".into()],
            }],
        ),
    ] {
        assert!(matches!(
            StoryFindings::new("猫。", evaluation, targets, departures),
            Err(EvaluationError::InvalidFindingSpan)
        ));
    }
}
