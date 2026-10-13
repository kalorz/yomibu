#[path = "../../../tests/support/story_inventory.rs"]
mod story_inventory;

use std::sync::atomic::{AtomicUsize, Ordering};
use yomibu_app::{
    application::generate_story::{
        StoryInputs, StoryPipeline, StoryRunError,
        progress::{ProgressEvent, RunProgress, Step},
        run_story,
    },
    configuration::StorySettings,
};
use yomibu_core::{
    domain::generation::preparation::{GenerationError, prepare_story},
    domain::{
        generation::candidate::{
            CandidateConstructionError, GeneratedCandidates, GeneratedPassage, GenerationProvenance,
        },
        generation::{
            StoryError, StoryFormat, StoryGenerationOptions, StoryRequest, StoryVocabularySelection,
        },
        knowledge::inventory::LearnerInventory,
    },
    ports::capabilities::{CandidateGenerator, StoryPreparer},
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

fn settings() -> StorySettings {
    StorySettings {
        topic: None,
        select: 3,
        seed: Some(7),
        format: StoryFormat::Sentence,
        candidates: 1,
        vocabulary_targets: vec![],
    }
}

#[tokio::test]
async fn supplied_preparer_binds_trimmed_selection_and_executes_matching_generator_once() {
    let (inventory, request) = inputs();
    let generator = TestGenerator::default();
    let mut events = Vec::new();
    let report = run_story(
        StoryInputs { inventory, request },
        StoryPipeline {
            selection: &[],
            selection_revision: "test",
            preparation: &TestPreparer,
            generation: &generator,
        },
        sentence_options(),
        3,
        7,
        RunProgress::new(|event| events.push(event)),
    )
    .await
    .unwrap();
    assert_eq!(generator.calls.load(Ordering::SeqCst), 1);
    assert_eq!(report.selection.vocabulary_ids, ["cat"]);
    assert_eq!(report.selection.seed, 7);
    assert_eq!(report.generated.passages()[0].text, "猫。");
    let starts: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            ProgressEvent::Started { step } => Some(*step),
            _ => None,
        })
        .collect();
    assert_eq!(starts, [Step::Inputs, Step::Selection, Step::Generation]);
    assert_eq!(events.len(), 6);
}

#[tokio::test]
async fn supplied_generator_failure_stays_typed_and_is_not_retried() {
    let (inventory, request) = inputs();
    let generator = TestGenerator {
        fail: true,
        ..Default::default()
    };
    let result = run_story(
        StoryInputs { inventory, request },
        StoryPipeline {
            selection: &[],
            selection_revision: "test",
            preparation: &TestPreparer,
            generation: &generator,
        },
        sentence_options(),
        3,
        7,
        RunProgress::new(|_| {}),
    )
    .await;
    assert!(matches!(
        result,
        Err(StoryRunError::Generation(GenerationError::Generator(
            ProviderFailure(429)
        )))
    ));
    assert_eq!(generator.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn preparation_cannot_bind_a_selection_that_drops_a_request_target() {
    let (inventory, mut request) = inputs();
    request.targets.vocabulary.push("dog".into());
    let selection = yomibu_ext::learner_vocabulary_selection::select_builtin_vocabulary(
        &inventory, &request, 3, 7,
    )
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
        let selection = yomibu_ext::learner_vocabulary_selection::select_builtin_vocabulary(
            inventory, request, 1, 7,
        )?;
        TestPreparer.prepare(inventory, request, selection, options)
    }
}

#[test]
fn supplied_preparer_cannot_replace_the_callers_full_inventory() {
    let (inventory, request) = inputs();
    let selection = yomibu_ext::learner_vocabulary_selection::select_builtin_vocabulary(
        &inventory, &request, 1, 7,
    )
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
        let selection = yomibu_ext::learner_vocabulary_selection::select_builtin_vocabulary(
            &inventory, &request, 1, 7,
        )
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

#[tokio::test]
async fn request_overrides_leave_defaults_unchanged_and_reuse_the_supplied_generator() {
    use yomibu_app::configuration::{Patch, StoryOverrides};
    let base = settings();
    let changed = base.for_invocation(StoryOverrides {
        topic: Patch::Set("猫".into()),
        ..Default::default()
    });
    let generator = TestGenerator::default();
    for (settings, topic) in [(&changed, Some("猫")), (&base, None)] {
        let (inventory, mut request) = inputs();
        request.topic = settings.request().unwrap().topic;
        let report = run_story(
            StoryInputs { inventory, request },
            StoryPipeline {
                selection: &[],
                selection_revision: "test",
                preparation: &TestPreparer,
                generation: &generator,
            },
            sentence_options(),
            settings.select,
            settings.seed.unwrap_or(0),
            RunProgress::new(|_| {}),
        )
        .await
        .unwrap();
        assert_eq!(
            report.request.topic.as_ref().map(|value| value.text()),
            topic
        );
    }
    assert_eq!(generator.calls.load(Ordering::SeqCst), 2);
    assert!(base.topic.is_none());
}
