use yomibu_core::domain::evaluation::{
    Check, CheckKind, CheckOutcome, CheckState, Evaluation, EvaluationBasis, EvaluationError,
    Finding,
};

fn with_check(text: &str, position: usize, check: Check) -> Result<Evaluation, EvaluationError> {
    let mut checks = std::array::from_fn(|_| Check {
        state: CheckState::Completed(CheckOutcome::Pass),
        findings: vec![],
        coverage: "checked",
    });
    checks[position] = check;
    let [vocabulary, inflection, particles, nominal, scope] = checks;
    Evaluation::new(
        text,
        EvaluationBasis::ExplicitWordUses,
        vocabulary,
        inflection,
        particles,
        nominal,
        scope,
    )
}

#[test]
fn evaluation_construction_rejects_out_of_bounds_or_non_utf8_finding_spans() {
    for position in 0..5 {
        for span in [0..100, 0..1, 1..3, std::ops::Range { start: 6, end: 3 }] {
            let check = Check {
                state: CheckState::Completed(CheckOutcome::Fail),
                findings: vec![Finding {
                    span,
                    reason: "unavailable word",
                }],
                coverage: "checked",
            };
            assert!(matches!(
                with_check("猫です。", position, check),
                Err(EvaluationError::InvalidFindingSpan)
            ));
        }
    }
}

#[test]
fn evaluation_construction_rejects_states_that_contradict_their_findings() {
    for position in 0..5 {
        for state in [
            CheckState::Completed(CheckOutcome::Pass),
            CheckState::NotRun,
        ] {
            let check = Check {
                state,
                findings: vec![Finding {
                    span: 0..3,
                    reason: "unavailable word",
                }],
                coverage: "checked",
            };
            assert!(matches!(
                with_check("猫です。", position, check),
                Err(EvaluationError::InvalidCheckState)
            ));
        }
        for outcome in [CheckOutcome::Fail, CheckOutcome::Inconclusive] {
            let check = Check {
                state: CheckState::Completed(outcome),
                findings: vec![],
                coverage: "checked",
            };
            assert!(matches!(
                with_check("猫です。", position, check),
                Err(EvaluationError::InvalidCheckState)
            ));
        }
    }
}

#[test]
fn evaluation_construction_preserves_consistent_states_and_original_byte_spans() {
    for (position, kind) in [
        CheckKind::Vocabulary,
        CheckKind::Inflection,
        CheckKind::Particles,
        CheckKind::Nominal,
        CheckKind::Scope,
    ]
    .into_iter()
    .enumerate()
    {
        for state in [
            CheckState::NotRun,
            CheckState::Completed(CheckOutcome::Pass),
            CheckState::Completed(CheckOutcome::Fail),
            CheckState::Completed(CheckOutcome::Inconclusive),
        ] {
            let findings = match state {
                CheckState::Completed(CheckOutcome::Fail | CheckOutcome::Inconclusive) => {
                    vec![Finding {
                        span: 3..9,
                        reason: "unavailable form",
                    }]
                }
                _ => vec![],
            };
            let report = with_check(
                "猫です。",
                position,
                Check {
                    state,
                    findings,
                    coverage: "checked",
                },
            )
            .unwrap();
            let check = report.check(kind);
            assert_eq!(check.state, state);
            assert_eq!(report.outcome(), state);
            assert_eq!(check.coverage, "checked");
            match state {
                CheckState::Completed(CheckOutcome::Fail | CheckOutcome::Inconclusive) => {
                    assert_eq!(check.findings.len(), 1);
                    assert_eq!(check.findings[0].span, 3..9);
                    assert_eq!(&"猫です。"[check.findings[0].span.clone()], "です");
                    assert_eq!(check.findings[0].reason, "unavailable form");
                }
                _ => assert!(check.findings.is_empty()),
            }
        }
    }
}
