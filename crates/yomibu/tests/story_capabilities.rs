#[path = "../../../tests/support/story_inventory.rs"]
mod story_inventory;

use std::sync::atomic::{AtomicUsize, Ordering};
use yomibu_core::{
    capabilities::{CandidateGenerator, StoryPreparer},
    domain::{
        candidate::{
            CandidateConstructionError, GeneratedCandidates, GeneratedPassage, GenerationProvenance,
        },
        inventory::LearnerInventory,
        story::{
            StoryError, StoryFormat, StoryGenerationOptions, StoryRequest, StoryVocabularySelection,
        },
    },
    pipeline::story::{GenerationError, prepare_story},
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

#[derive(Default)]
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
            generation_provenance(&request.options.model, "test-v1"),
            request.options.format,
            request.options.candidate_count,
        )
        .unwrap())
    }
}

fn inputs() -> (LearnerInventory, StoryRequest) {
    let inventory = story_inventory::inventory();
    let request = serde_json::from_value(
        serde_json::json!({"version":1,"targets":{"vocabulary":["cat"],"grammar":[]}}),
    )
    .unwrap();
    (inventory, request)
}

fn sentence_options() -> StoryGenerationOptions {
    StoryGenerationOptions {
        model: "test-only-model".into(),
        candidate_count: 1,
        format: StoryFormat::Sentence,
    }
}

fn generation_provenance(model: &str, revision: &'static str) -> GenerationProvenance {
    GenerationProvenance {
        provider: "test",
        requested_model: model.into(),
        returned_model: "test".into(),
        requested_tier: "test",
        returned_tier: None,
        prompt_revision: revision,
        request_sha256: "test".into(),
        request_bytes: 0,
        response_id: "test".into(),
        request_id: None,
        request_count: 1,
        usage: None,
    }
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
            sentence_options(),
        )
        .unwrap();
        assert_eq!(plan.selection().selected.len(), 1);
        let generator = TestGenerator::default();

        let result = plan.generate(&generator).await.unwrap();
        assert_eq!(generator.calls.load(Ordering::SeqCst), 1);
        result
    };
    assert_eq!(result.passages()[0].text, "猫。");
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
        sentence_options(),
    )
    .unwrap();
    let generator = TestGenerator {
        fail: true,
        ..Default::default()
    };

    assert!(matches!(
        plan.generate(&generator).await,
        Err(GenerationError::Generator(ProviderFailure(429)))
    ));
    assert_eq!(generator.calls.load(Ordering::SeqCst), 1);
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
            generation_provenance("test", "test"),
            self.format,
            self.count,
        )
        .unwrap())
    }
}

#[tokio::test]
async fn generated_count_and_format_must_match_the_bound_options() {
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
                ..sentence_options()
            },
        )
        .unwrap();
        let generator = WrongShapeGenerator {
            format: returned,
            count,
            calls: AtomicUsize::new(0),
        };

        let result = plan.generate(&generator).await;
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
    }
}
