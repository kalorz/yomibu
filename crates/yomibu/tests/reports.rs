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
