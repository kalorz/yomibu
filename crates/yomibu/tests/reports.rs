#[test]
fn typed_downstream_error_reports_keep_available_analysis_without_completed_judgments() {
    use yomibu::{
        adapters::sudachi::AnalysisError,
        analysis::{AnalysisProvenance, Sentence, SentenceAnalysis},
        evaluation::EvaluationError,
        generation::{CandidateAssessment, CandidateError},
    };
    let analysis = SentenceAnalysis {
        sentence: Sentence::new("猫です。").unwrap(),
        units: vec![],
        provenance: AnalysisProvenance {
            analyzer_revision: "synthetic-boundary-test",
            dictionary_version: "synthetic-boundary-test",
            dictionary_sha256: "synthetic-boundary-test",
            configuration_sha256: "synthetic-boundary-test".into(),
            dictionary_loading: None,
        },
    };
    // These are report-boundary cases, not claims about causing real Sudachi failures.
    for (assessment, stage, code, has_analysis) in [
        (
            CandidateAssessment::ExecutionError {
                analysis: None,
                error: CandidateError::Analysis(AnalysisError::InvalidSpan),
            },
            "analysis",
            "invalid_analysis_span",
            false,
        ),
        (
            CandidateAssessment::ExecutionError {
                analysis: Some(analysis),
                error: CandidateError::Evaluation(EvaluationError::InvalidAnalysis),
            },
            "evaluation",
            "invalid_analysis",
            true,
        ),
    ] {
        let report = yomibu::reports::candidate::CandidateReport::new(1, "猫です。", &assessment);
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(!json["analysis"].is_null(), has_analysis);
        assert_eq!(json["assessment"]["status"], "execution_error");
        assert_eq!(json["assessment"]["stage"], stage);
        assert_eq!(json["assessment"]["code"], code);
        assert!(json["assessment"].get("outcome").is_none());
        assert!(json["assessment"].get("evaluation").is_none());
        let kinds: Vec<_> = json["assessment"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|check| {
                assert_eq!(check["state"], "NotRun");
                check["kind"].as_str().unwrap()
            })
            .collect();
        assert_eq!(
            kinds,
            ["Vocabulary", "Inflection", "Particles", "Nominal", "Scope"]
        );
    }
}

#[test]
fn typed_target_states_preserve_version_one_report_fields_and_spellings() {
    use serde_json::json;
    use yomibu::{
        evaluation::DirectObjectEvidence,
        story::{
            TargetCoverage::{Complete, Partial},
            TargetKind, TargetObservation,
            TargetState::{Absent, NotRun, Observed, Unassessable},
            TargetUncertainty, TargetUncertaintyReason, TargetUncertaintyScope,
        },
    };
    for (state, status, completeness) in [
        (NotRun, "not_run", "not_run"),
        (Absent, "absent", "complete"),
        (Unassessable, "unassessable", "partial"),
        (Observed(Complete), "observed", "complete"),
        (Observed(Partial), "observed", "partial"),
    ] {
        assert_eq!(
            serde_json::to_value(state).unwrap(),
            json!({
                "status":status,"completeness":completeness
            })
        );
    }
    let observation = TargetObservation {
        kind: TargetKind::Grammar,
        id: "topic-or-object".into(),
        state: Observed(Partial),
        spans: std::iter::once(3..6).collect(),
        uncertainties: vec![TargetUncertainty {
            span: 12..18,
            scope: TargetUncertaintyScope::TargetOccurrence,
            reason: TargetUncertaintyReason::DirectObject(DirectObjectEvidence::Unknown),
            inventory_entries: vec!["eat".into()],
        }],
    };
    assert_eq!(
        serde_json::to_value(observation).unwrap(),
        json!({
            "kind":"grammar","id":"topic-or-object","status":"observed",
            "completeness":"partial","spans":[{"start":3,"end":6}]
        })
    );
}
