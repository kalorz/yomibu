use std::{
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering},
};

use yomibu_core::{
    capabilities::{CandidateGenerator, SentenceAnalyzer, StoryAssessor, StoryPreparer},
    domain::{
        analysis::{
            AnalysisProvenance, DictionaryProvenance, LexicalUnit, Sentence, SentenceAnalysis,
            Token,
        },
        candidate::{
            CandidateAssessment, CandidateConstructionError, CandidateError, GeneratedCandidates,
            GeneratedPassage, GenerationProvenance,
        },
        evaluation::{Check, CheckOutcome, CheckState, Evaluation, EvaluationBasis},
        inventory::LearnerInventory,
        story::{
            PlanDeparture, StoryAssessmentInputs, StoryError, StoryFindings, StoryFormat,
            StoryGenerationOptions, StoryRequest, StoryVocabularySelection, TargetKind,
            TargetObservation,
        },
    },
    pipeline::{
        assessment::assess_passages,
        story::{GenerationError, prepare_story},
    },
};

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("test provider unavailable: {0}")]
struct ProviderFailure(u16);

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
struct PreparationFailure(#[from] StoryError);

struct TestRequest {
    options: StoryGenerationOptions,
    selected: Vec<String>,
}

struct TestPreparer;
impl StoryPreparer for TestPreparer {
    type PreparedRequest = TestRequest;
    type Error = PreparationFailure;

    fn prepare<'a>(
        &self,
        _: &'a LearnerInventory,
        _: &'a StoryRequest,
        mut selection: StoryVocabularySelection<'a>,
        options: StoryGenerationOptions,
    ) -> Result<(StoryVocabularySelection<'a>, TestRequest), Self::Error> {
        // This test's prompt uses just the explicit target.
        selection.selected.truncate(1);
        let prepared = TestRequest {
            options,
            selected: selection
                .selected
                .iter()
                .map(|entry| entry.word.id.clone())
                .collect(),
        };
        Ok((selection, prepared))
    }
}

struct TestGenerator {
    calls: AtomicUsize,
    fail: bool,
}
impl CandidateGenerator for TestGenerator {
    type PreparedRequest = TestRequest;
    type Error = ProviderFailure;

    async fn generate_candidates(
        &self,
        request: &TestRequest,
    ) -> Result<GeneratedCandidates, Self::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.selected, ["cat"]);
        assert_eq!(request.options.model, "test-only-model");
        if self.fail {
            return Err(ProviderFailure(429));
        }
        Ok(GeneratedCandidates::new(
            vec![GeneratedPassage {
                text: "猫。".into(),
                sentence_spans: std::iter::once(0..6).collect(),
            }],
            GenerationProvenance {
                provider: "test",
                requested_model: request.options.model.clone(),
                returned_model: "test".into(),
                requested_tier: "test",
                returned_tier: None,
                prompt_revision: "test-v1",
                request_sha256: "test".into(),
                request_bytes: 0,
                response_id: "test".into(),
                request_id: None,
                request_count: 1,
                usage: None,
            },
            request.options.format,
            request.options.candidate_count,
        )
        .unwrap())
    }
}

fn inputs() -> (LearnerInventory, StoryRequest) {
    let inventory = LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap();
    let request = serde_json::from_value(
        serde_json::json!({"version":1,"targets":{"vocabulary":["cat"],"grammar":[]}}),
    )
    .unwrap();
    (inventory, request)
}

#[tokio::test]
async fn supplied_preparer_binds_trimmed_selection_and_executes_matching_generator_once() {
    let result = {
        let (inventory, request) = inputs();
        let selection =
            yomibu::application::selection::select_builtin_vocabulary(&inventory, &request, 3, 7)
                .unwrap();
        let plan = prepare_story(
            &TestPreparer,
            &inventory,
            &request,
            selection,
            StoryGenerationOptions {
                model: "test-only-model".into(),
                candidate_count: 1,
                format: StoryFormat::Sentence,
            },
        )
        .unwrap();
        assert_eq!(plan.selection().selected.len(), 1);
        assert_eq!(plan.assessment_inputs().selected_vocabulary_ids(), ["cat"]);
        assert_eq!(plan.assessment_inputs().inventory().vocabulary.len(), 4);
        assert!(std::ptr::eq(
            plan.assessment_inputs().inventory(),
            &inventory
        ));
        assert!(std::ptr::eq(plan.assessment_inputs().request(), &request));
        let generator = TestGenerator {
            calls: AtomicUsize::new(0),
            fail: false,
        };
        let analyzer = TestAnalyzer {
            calls: RefCell::new(vec![]),
            replace_text: false,
        };
        let assessor = TestAssessor(RefCell::new(vec![]));
        let result = yomibu::application::story::generate_story_with(
            &plan, &generator, &analyzer, &assessor,
        )
        .await
        .unwrap();
        assert_eq!(generator.calls.load(Ordering::SeqCst), 1);
        assert_eq!(*analyzer.calls.borrow(), ["猫。"]);
        assert_eq!(*assessor.0.borrow(), ["猫。"]);
        result
    };
    assert_eq!(result.candidates().passages()[0].text, "猫。");
    assert!(!result.has_execution_errors());
    let CandidateAssessment::Completed {
        analysis,
        evaluation,
    } = &result.assessments()[0].assessment
    else {
        panic!()
    };
    assert_eq!(analysis.sentence.text(), "猫。");
    assert_eq!(
        evaluation.outcome(),
        CheckState::Completed(CheckOutcome::Pass)
    );
}

#[tokio::test]
async fn supplied_generator_failure_stays_typed_and_is_not_retried() {
    let (inventory, request) = inputs();
    let selection =
        yomibu::application::selection::select_builtin_vocabulary(&inventory, &request, 3, 7)
            .unwrap();
    let plan = prepare_story(
        &TestPreparer,
        &inventory,
        &request,
        selection,
        StoryGenerationOptions {
            model: "test-only-model".into(),
            candidate_count: 1,
            format: StoryFormat::Sentence,
        },
    )
    .unwrap();
    let generator = TestGenerator {
        calls: AtomicUsize::new(0),
        fail: true,
    };
    let analyzer = TestAnalyzer {
        calls: RefCell::new(vec![]),
        replace_text: false,
    };
    let assessor = TestAssessor(RefCell::new(vec![]));
    assert!(matches!(
        yomibu::application::story::generate_story_with(&plan, &generator, &analyzer, &assessor)
            .await,
        Err(GenerationError::Generator(ProviderFailure(429)))
    ));
    assert_eq!(generator.calls.load(Ordering::SeqCst), 1);
    assert!(analyzer.calls.borrow().is_empty());
    assert!(assessor.0.borrow().is_empty());
}

#[test]
fn preparation_cannot_bind_a_selection_that_drops_a_request_target() {
    let (inventory, mut request) = inputs();
    request.targets.vocabulary.push("dog".into());
    let selection =
        yomibu::application::selection::select_builtin_vocabulary(&inventory, &request, 3, 7)
            .unwrap();
    assert!(matches!(
        prepare_story(
            &TestPreparer,
            &inventory,
            &request,
            selection,
            Default::default()
        ),
        Err(PreparationFailure(StoryError::Invalid(
            "plan does not match inventory and targets"
        )))
    ));
}

#[test]
fn default_assessor_can_use_supplied_morphology_without_a_dictionary() {
    use yomibu_components::japanese_constraint_checks::JapaneseConstraintChecks;
    use yomibu_core::{domain::evaluation::CheckKind, pipeline::assessment::assess_candidate};
    let (inventory, request) = inputs();
    let selection =
        yomibu::application::selection::select_builtin_vocabulary(&inventory, &request, 1, 7)
            .unwrap();
    let inputs = StoryAssessmentInputs::new(&inventory, &request, &selection).unwrap();
    let analyzer = TestAnalyzer {
        calls: RefCell::new(vec![]),
        replace_text: false,
    };
    let result = assess_candidate("猫。", &inputs, &analyzer, &JapaneseConstraintChecks);
    let CandidateAssessment::Completed {
        analysis,
        evaluation,
    } = result.assessment
    else {
        panic!()
    };
    assert_eq!(
        evaluation.check(CheckKind::Vocabulary).state,
        CheckState::Completed(CheckOutcome::Pass)
    );
    assert_eq!(
        evaluation.check(CheckKind::Scope).state,
        CheckState::Completed(CheckOutcome::Inconclusive)
    );
    assert_eq!(result.targets[0].spans.len(), 1);
    assert_eq!(result.targets[0].spans[0], 0..3);
    assert_eq!(analysis.provenance.analyzer_revision, "test");
    assert_eq!(*analyzer.calls.borrow(), ["猫。"]);
}

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("test analysis failed")]
struct AnalysisFailure;

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("test assessment failed")]
struct AssessmentFailure;

struct TestAnalyzer {
    calls: RefCell<Vec<String>>,
    replace_text: bool,
}
impl SentenceAnalyzer for TestAnalyzer {
    type Error = AnalysisFailure;

    fn analyze<'a>(&self, sentence: Sentence<'a>) -> Result<SentenceAnalysis<'a>, Self::Error> {
        self.calls.borrow_mut().push(sentence.text().into());
        if sentence.text() == "犬。" {
            return Err(AnalysisFailure);
        }
        let sentence = if self.replace_text {
            Sentence::new("別。").unwrap()
        } else {
            sentence
        };
        let token = Token {
            span: 0..3,
            dictionary_form: sentence.text()[..3].into(),
            reading: "ネコ".into(),
            part_of_speech: vec!["名詞".into(); 6],
            out_of_vocabulary: false,
        };
        let punctuation = Token {
            span: 3..6,
            dictionary_form: "。".into(),
            reading: "。".into(),
            part_of_speech: vec!["補助記号".into(); 6],
            out_of_vocabulary: false,
        };
        Ok(SentenceAnalysis {
            sentence,
            units: [token, punctuation]
                .into_iter()
                .map(|token| LexicalUnit {
                    components: vec![token.clone()],
                    token,
                })
                .collect(),
            provenance: AnalysisProvenance {
                analyzer_revision: "test",
                dictionary_version: "test",
                dictionary_sha256: "test",
                configuration_sha256: "test".into(),
                dictionary_loading: DictionaryProvenance {
                    generation: "test".into(),
                    verification: "test",
                    startup_checks: "test",
                    file_stability: "test",
                },
            },
        })
    }
}

struct TestAssessor(RefCell<Vec<String>>);
impl StoryAssessor for TestAssessor {
    type Error = AssessmentFailure;

    fn assess<'a>(
        &self,
        analysis: &'a SentenceAnalysis<'_>,
        inputs: &StoryAssessmentInputs<'_>,
    ) -> Result<StoryFindings<'a>, Self::Error> {
        self.0.borrow_mut().push(analysis.sentence.text().into());
        assert_eq!(analysis.provenance.analyzer_revision, "test");
        assert_eq!(inputs.inventory().vocabulary.len(), 4);
        assert_eq!(inputs.selected_vocabulary_ids(), ["cat"]);
        assert_eq!(inputs.request().targets.vocabulary, ["cat"]);
        if analysis.sentence.text() == "鳥。" {
            return Err(AssessmentFailure);
        }
        let pass = || Check {
            state: CheckState::Completed(CheckOutcome::Pass),
            findings: vec![],
            coverage: "test",
        };
        let evaluation = Evaluation::new(
            analysis.sentence.text(),
            EvaluationBasis::FullLearnerInventory,
            pass(),
            pass(),
            pass(),
            pass(),
            pass(),
        )
        .unwrap();
        Ok(StoryFindings::new(
            analysis.sentence.text(),
            evaluation,
            vec![TargetObservation::from_evidence(
                TargetKind::Vocabulary,
                "cat",
                std::iter::once(0..3).collect(),
                vec![],
            )],
            vec![PlanDeparture {
                span: 3..6,
                inventory_entries: vec!["test-departure".into()],
            }],
        )
        .unwrap())
    }
}

#[test]
fn supplied_evidence_and_assessor_run_once_per_sentence_and_retain_independent_typed_failures() {
    let results = {
        let (inventory, request) = inputs();
        let selection =
            yomibu::application::selection::select_builtin_vocabulary(&inventory, &request, 1, 7)
                .unwrap();
        let inputs = StoryAssessmentInputs::new(&inventory, &request, &selection).unwrap();
        let analyzer = TestAnalyzer {
            calls: RefCell::new(vec![]),
            replace_text: false,
        };
        let assessor = TestAssessor(RefCell::new(vec![]));
        let passages = vec![
            GeneratedPassage {
                text: "猫。犬。鳥。".into(),
                sentence_spans: vec![0..6, 6..12, 12..18],
            },
            GeneratedPassage {
                text: "猫。".into(),
                sentence_spans: std::iter::once(0..6).collect(),
            },
        ];
        let results = assess_passages(&passages, &inputs, Some((&analyzer, &assessor)));
        assert_eq!(*analyzer.calls.borrow(), ["猫。", "犬。", "鳥。", "猫。"]);
        assert_eq!(*assessor.0.borrow(), ["猫。", "鳥。", "猫。"]);
        assert_eq!(passages[0].text, "猫。犬。鳥。");
        results
    };
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].sentences.len(), 3);
    assert_eq!(results[0].targets[0].spans.len(), 1);
    assert_eq!(results[0].targets[0].spans[0], 0..3);
    assert_eq!(
        results[0].targets[0]
            .uncertainties
            .iter()
            .map(|u| u.span.clone())
            .collect::<Vec<_>>(),
        [6..12, 12..18]
    );
    assert_eq!(results[0].plan_departures[0].span, 3..6);
    assert!(matches!(
        results[0].sentences[1].assessment.assessment,
        CandidateAssessment::ExecutionError {
            analysis: None,
            error: CandidateError::Analysis(AnalysisFailure)
        }
    ));
    assert!(matches!(
        results[0].sentences[2].assessment.assessment,
        CandidateAssessment::ExecutionError {
            analysis: Some(_),
            error: CandidateError::Assessment(AssessmentFailure)
        }
    ));
    let CandidateAssessment::Completed {
        analysis,
        evaluation,
    } = &results[1].sentences[0].assessment.assessment
    else {
        panic!()
    };
    assert_eq!(analysis.sentence.text(), "猫。");
    assert_eq!(
        evaluation.outcome(),
        CheckState::Completed(CheckOutcome::Pass)
    );
}

#[test]
fn mismatched_analysis_is_rejected_before_judgment_and_absent_capabilities_are_not_run() {
    let (inventory, request) = inputs();
    let selection =
        yomibu::application::selection::select_builtin_vocabulary(&inventory, &request, 1, 7)
            .unwrap();
    let inputs = StoryAssessmentInputs::new(&inventory, &request, &selection).unwrap();
    let analyzer = TestAnalyzer {
        calls: RefCell::new(vec![]),
        replace_text: true,
    };
    let assessor = TestAssessor(RefCell::new(vec![]));
    let passages = [GeneratedPassage {
        text: "猫。".into(),
        sentence_spans: std::iter::once(0..6).collect(),
    }];
    let result = assess_passages(&passages, &inputs, Some((&analyzer, &assessor)));
    assert!(matches!(
        result[0].sentences[0].assessment.assessment,
        CandidateAssessment::ExecutionError {
            analysis: None,
            error: CandidateError::MismatchedAnalysis
        }
    ));
    assert!(assessor.0.borrow().is_empty());
    let result = assess_passages::<TestAnalyzer, TestAssessor>(&passages, &inputs, None);
    assert!(matches!(
        result[0].sentences[0].assessment.assessment,
        CandidateAssessment::NotRun
    ));
    assert_eq!(result[0].targets[0].state.status(), "not_run");
    assert_eq!(analyzer.calls.borrow().len(), 1);
}

struct OtherSentenceAssessor;
impl StoryAssessor for OtherSentenceAssessor {
    type Error = AssessmentFailure;

    fn assess<'a>(
        &self,
        _: &'a SentenceAnalysis<'_>,
        _: &StoryAssessmentInputs<'_>,
    ) -> Result<StoryFindings<'a>, Self::Error> {
        let pass = || Check {
            state: CheckState::Completed(CheckOutcome::Pass),
            findings: vec![],
            coverage: "test",
        };
        let evaluation = Evaluation::new(
            "犬。",
            EvaluationBasis::FullLearnerInventory,
            pass(),
            pass(),
            pass(),
            pass(),
            pass(),
        )
        .unwrap();
        Ok(StoryFindings::new("犬。", evaluation, vec![], vec![]).unwrap())
    }
}

#[test]
fn findings_for_a_different_sentence_cannot_be_attached_to_original_analysis() {
    let (inventory, request) = inputs();
    let selection =
        yomibu::application::selection::select_builtin_vocabulary(&inventory, &request, 1, 7)
            .unwrap();
    let inputs = StoryAssessmentInputs::new(&inventory, &request, &selection).unwrap();
    let analyzer = TestAnalyzer {
        calls: RefCell::new(vec![]),
        replace_text: false,
    };
    let result = yomibu_core::pipeline::assessment::assess_candidate(
        "猫。",
        &inputs,
        &analyzer,
        &OtherSentenceAssessor,
    );
    let CandidateAssessment::ExecutionError {
        analysis: Some(analysis),
        error: CandidateError::MismatchedAssessment,
    } = result.assessment
    else {
        panic!("mismatched findings must be an execution error")
    };
    assert_eq!(analysis.sentence.text(), "猫。");
    assert_eq!(*analyzer.calls.borrow(), ["猫。"]);
}

#[derive(Debug, Clone, Copy)]
enum InvalidFindings {
    MissingTarget,
    DuplicateTarget,
    ExtraTarget,
    WrongKind,
    ContradictoryState,
    WrongBasis,
}

impl StoryAssessor for InvalidFindings {
    type Error = AssessmentFailure;

    fn assess<'a>(
        &self,
        analysis: &'a SentenceAnalysis<'_>,
        _: &StoryAssessmentInputs<'_>,
    ) -> Result<StoryFindings<'a>, Self::Error> {
        let observed = || {
            TargetObservation::from_evidence(
                TargetKind::Vocabulary,
                "cat",
                std::iter::once(0..3).collect(),
                vec![],
            )
        };
        let mut targets = vec![observed()];
        let mut basis = EvaluationBasis::FullLearnerInventory;
        // Keep a later sentence valid to check passage coverage and retention.
        if analysis.sentence.text() == "猫。" {
            match self {
                Self::MissingTarget => targets.clear(),
                Self::DuplicateTarget => targets.push(observed()),
                Self::ExtraTarget => targets.push(TargetObservation::from_evidence(
                    TargetKind::Vocabulary,
                    "dog",
                    vec![],
                    vec![],
                )),
                Self::WrongKind => targets[0].kind = TargetKind::Grammar,
                Self::ContradictoryState => targets[0].spans.clear(),
                Self::WrongBasis => basis = EvaluationBasis::ExplicitWordUses,
            }
        }
        let pass = || Check {
            state: CheckState::Completed(CheckOutcome::Pass),
            findings: vec![],
            coverage: "test",
        };
        let evaluation = Evaluation::new(
            analysis.sentence.text(),
            basis,
            pass(),
            pass(),
            pass(),
            pass(),
            pass(),
        )
        .unwrap();
        Ok(StoryFindings::new(analysis.sentence.text(), evaluation, targets, vec![]).unwrap())
    }
}

#[test]
fn inconsistent_assessor_outputs_fail_the_sentence_without_losing_other_evidence() {
    use yomibu_core::domain::story::{TargetCoverage, TargetState, TargetUncertaintyReason};
    let (inventory, request) = inputs();
    let selection =
        yomibu::application::selection::select_builtin_vocabulary(&inventory, &request, 1, 7)
            .unwrap();
    let inputs = StoryAssessmentInputs::new(&inventory, &request, &selection).unwrap();
    let passages = [GeneratedPassage {
        text: "猫。鳥。".into(),
        sentence_spans: vec![0..6, 6..12],
    }];
    for assessor in [
        InvalidFindings::MissingTarget,
        InvalidFindings::DuplicateTarget,
        InvalidFindings::ExtraTarget,
        InvalidFindings::WrongKind,
        InvalidFindings::ContradictoryState,
        InvalidFindings::WrongBasis,
    ] {
        let analyzer = TestAnalyzer {
            calls: RefCell::new(vec![]),
            replace_text: false,
        };
        let results = assess_passages(&passages, &inputs, Some((&analyzer, &assessor)));
        assert!(
            matches!(
                results[0].sentences[0].assessment.assessment,
                CandidateAssessment::ExecutionError {
                    analysis: Some(_),
                    error: CandidateError::MismatchedAssessment,
                }
            ),
            "{assessor:?}"
        );
        assert!(
            matches!(
                results[0].sentences[1].assessment.assessment,
                CandidateAssessment::Completed { .. }
            ),
            "{assessor:?}"
        );
        assert_eq!(results[0].targets.len(), 1);
        let target = &results[0].targets[0];
        assert_eq!(target.state, TargetState::Observed(TargetCoverage::Partial));
        assert_eq!(target.spans, vec![6..9]);
        assert_eq!(target.uncertainties.len(), 1);
        assert_eq!(target.uncertainties[0].span, 0..6);
        assert_eq!(
            target.uncertainties[0].reason,
            TargetUncertaintyReason::AssessmentUnavailable
        );
        assert_eq!(*analyzer.calls.borrow(), ["猫。", "鳥。"]);
    }
}

struct ForeignPreparer;
impl StoryPreparer for ForeignPreparer {
    type PreparedRequest = TestRequest;
    type Error = PreparationFailure;

    fn prepare<'a>(
        &self,
        _: &'a LearnerInventory,
        _: &'a StoryRequest,
        _: StoryVocabularySelection<'a>,
        options: StoryGenerationOptions,
    ) -> Result<(StoryVocabularySelection<'a>, TestRequest), Self::Error> {
        static FOREIGN: std::sync::OnceLock<(LearnerInventory, StoryRequest)> =
            std::sync::OnceLock::new();
        let (inventory, request) = FOREIGN.get_or_init(inputs);
        let selection =
            yomibu::application::selection::select_builtin_vocabulary(inventory, request, 1, 7)?;
        TestPreparer.prepare(inventory, request, selection, options)
    }
}

#[test]
fn supplied_preparer_cannot_replace_the_callers_full_inventory() {
    let (inventory, request) = inputs();
    let selection =
        yomibu::application::selection::select_builtin_vocabulary(&inventory, &request, 1, 7)
            .unwrap();
    assert!(matches!(
        prepare_story(
            &ForeignPreparer,
            &inventory,
            &request,
            selection,
            Default::default()
        ),
        Err(PreparationFailure(StoryError::Invalid(
            "plan does not match inventory and targets"
        )))
    ));
}

struct WrongShapeGenerator {
    format: StoryFormat,
    count: usize,
    calls: AtomicUsize,
}
impl CandidateGenerator for WrongShapeGenerator {
    type PreparedRequest = TestRequest;
    type Error = ProviderFailure;

    async fn generate_candidates(
        &self,
        _: &TestRequest,
    ) -> Result<GeneratedCandidates, Self::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let sentences = self.format.sentence_bounds().0;
        Ok(GeneratedCandidates::new(
            (0..self.count)
                .map(|_| GeneratedPassage {
                    text: "猫。".repeat(sentences),
                    sentence_spans: (0..sentences).map(|i| i * 6..(i + 1) * 6).collect(),
                })
                .collect(),
            GenerationProvenance {
                provider: "test",
                requested_model: "test".into(),
                returned_model: "test".into(),
                requested_tier: "test",
                returned_tier: None,
                prompt_revision: "test",
                request_sha256: "test".into(),
                request_bytes: 0,
                response_id: "test".into(),
                request_id: None,
                request_count: 1,
                usage: None,
            },
            self.format,
            self.count,
        )
        .unwrap())
    }
}

#[tokio::test]
async fn generated_count_and_format_must_match_the_bound_options_before_assessment() {
    let (inventory, request) = inputs();
    for (expected, returned, count) in [
        (StoryFormat::Sentence, StoryFormat::Sentence, 3),
        (StoryFormat::Sentence, StoryFormat::Passage, 1),
        (StoryFormat::Passage, StoryFormat::Sentence, 1),
    ] {
        let selection =
            yomibu::application::selection::select_builtin_vocabulary(&inventory, &request, 1, 7)
                .unwrap();
        let plan = prepare_story(
            &TestPreparer,
            &inventory,
            &request,
            selection,
            StoryGenerationOptions {
                format: expected,
                candidate_count: 1,
                model: "test-only-model".into(),
            },
        )
        .unwrap();
        let generator = WrongShapeGenerator {
            format: returned,
            count,
            calls: AtomicUsize::new(0),
        };
        let analyzer = TestAnalyzer {
            calls: RefCell::new(vec![]),
            replace_text: false,
        };
        let assessor = TestAssessor(RefCell::new(vec![]));
        let result = yomibu::application::story::generate_story_with(
            &plan, &generator, &analyzer, &assessor,
        )
        .await;
        match result.unwrap_err() {
            GenerationError::InvalidCandidates(CandidateConstructionError::CandidateCount {
                expected: requested_count,
                actual,
            }) => {
                assert_eq!(requested_count, 1);
                assert_eq!(actual, 3);
                assert_eq!(count, 3);
            }
            GenerationError::InvalidCandidates(CandidateConstructionError::SentenceCount {
                format,
                actual,
            }) => {
                assert_eq!(format, expected);
                assert_eq!(
                    actual,
                    if returned == StoryFormat::Sentence {
                        1
                    } else {
                        3
                    }
                );
                assert_eq!(count, 1);
            }
            error => panic!("unexpected error: {error}"),
        }
        assert_eq!(generator.calls.load(Ordering::SeqCst), 1);
        assert!(analyzer.calls.borrow().is_empty());
        assert!(assessor.0.borrow().is_empty());
    }
}
