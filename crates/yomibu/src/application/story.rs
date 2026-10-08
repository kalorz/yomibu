use super::selection::select_vocabulary;
use yomibu_components::{
    japanese_constraint_checks::JapaneseConstraintChecks,
    openai_story_generation::{Client, PreparedRequest, ProviderError},
    sudachi_dictionary::{AnalysisError, SudachiAnalyzer},
};
use yomibu_components::{
    openai_story_generation::validate_options, story_prompt_preparation::StoryPromptPreparation,
};
use yomibu_core::domain::story::{
    StoryCandidateAssessment, StoryError, StoryGenerationOptions, StoryRequest,
};
use yomibu_core::domain::{
    candidate::{CandidateAssessment, CandidateError, GeneratedCandidates},
    embedding::{EmbeddingCache, EmbeddingModelIdentity},
    evaluation::EvaluationError,
    inventory::LearnerInventory,
};
use yomibu_core::{
    capabilities::{CandidateGenerator, SentenceAnalyzer, StoryAssessor},
    pipeline::{
        assessment::assess_candidates,
        story::{GenerationError, PreparedStory, prepare_story},
    },
};

/// Plan offline before initializing the caller's dictionary, credential or client.
pub fn plan_generation<'a>(
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    cache: &EmbeddingCache,
    model: &EmbeddingModelIdentity,
    selection_limit: usize,
    options: StoryGenerationOptions,
) -> Result<StoryGenerationPlan<'a>, StoryError> {
    validate_options(&options)?;
    request.validate_selection_limit(selection_limit)?;
    request.validate(inventory)?;
    let selection = select_vocabulary(inventory, request, cache, model, selection_limit)?;
    prepare_story(
        &StoryPromptPreparation,
        inventory,
        request,
        selection,
        options,
    )
}

/// Execute the default components with caller-supplied resources and executor.
/// Candidate errors remain in the result.
///
/// ```no_run
/// use yomibu::application::story::{plan_generation, generate_story};
/// use yomibu_core::domain::{inventory::LearnerInventory, story::StoryRequest, embedding::EmbeddingCache};
/// use yomibu_components::{sudachi_dictionary::{installation::ManagedInstallation, SudachiAnalyzer}, openai_story_generation::Client};
/// # async unsafe fn example(inventory: &LearnerInventory, request: &StoryRequest,
/// #     cache: &EmbeddingCache, dictionary_dir: &std::path::Path, api_key: &str)
/// #     -> Result<(), Box<dyn std::error::Error>> {
/// let plan = plan_generation(inventory, request, cache, &cache.model, 12, Default::default())?;
/// // Safety: this generation was fully verified by Yomibu's importer; the caller
/// // guarantees its mapped bytes stay unchanged for the analyzer's entire lifetime.
/// let installation = ManagedInstallation::open(dictionary_dir)?;
/// let analyzer = unsafe { SudachiAnalyzer::load(installation) }?;
/// let client = Client::new(api_key)?;
/// let result = generate_story(&plan, &client, &analyzer).await?;
/// assert_eq!(result.candidates().passages().iter().map(|p| p.sentence_spans.len()).sum::<usize>(), result.assessments().len());
/// # Ok(())
/// # }
/// ```
pub async fn generate_story(
    plan: &StoryGenerationPlan<'_>,
    client: &Client,
    analyzer: &SudachiAnalyzer,
) -> Result<StoryGenerationResult, GenerationError<ProviderError>> {
    let result = generate_story_with(plan, client, analyzer, &JapaneseConstraintChecks).await?;
    Ok(StoryGenerationResult {
        generated: result.generated,
        assessments: result
            .assessments
            .into_iter()
            .map(|assessment| assessment.map_error(DefaultCandidateError))
            .collect(),
    })
}

/// Execute supplied capabilities once; sentence failures retain other candidates.
pub async fn generate_story_with<R, G, A, S>(
    plan: &PreparedStory<'_, R>,
    generator: &G,
    analyzer: &A,
    assessor: &S,
) -> Result<StoryGenerationResult<CandidateError<A::Error, S::Error>>, GenerationError<G::Error>>
where
    G: CandidateGenerator<PreparedRequest = R>,
    A: SentenceAnalyzer,
    S: StoryAssessor,
{
    let generated = plan.generate(generator).await?;
    let assessments = assess_candidates(&generated, plan.assessment_inputs(), analyzer, assessor)
        .into_iter()
        .map(StoryCandidateAssessment::into_owned)
        .collect();
    Ok(StoryGenerationResult {
        generated,
        assessments,
    })
}

pub type StoryGenerationPlan<'a> = PreparedStory<'a, PreparedRequest>;
/// Default workflow diagnostics retain typed causes without exposing Sudachi internals.
#[derive(Debug)]
pub struct DefaultCandidateError(pub CandidateError<AnalysisError, EvaluationError>);

impl std::fmt::Display for DefaultCandidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            CandidateError::Analysis(_) => f.write_str("Pinned Sudachi analysis failed."),
            error => error.fmt(f),
        }
    }
}

impl std::error::Error for DefaultCandidateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

/// Owns every original candidate, available assessment and typed execution error.
#[derive(Debug)]
pub struct StoryGenerationResult<E = DefaultCandidateError> {
    generated: GeneratedCandidates,
    assessments: Vec<StoryCandidateAssessment<'static, E>>,
}
impl<E> StoryGenerationResult<E> {
    pub fn candidates(&self) -> &GeneratedCandidates {
        &self.generated
    }
    pub fn assessments(&self) -> &[StoryCandidateAssessment<'static, E>] {
        &self.assessments
    }
    pub fn has_execution_errors(&self) -> bool {
        self.assessments
            .iter()
            .any(|a| matches!(a.assessment, CandidateAssessment::ExecutionError { .. }))
    }
}
