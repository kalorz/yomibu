use yomibu_components::{
    embedding_vocabulary_selection::select_vocabulary,
    japanese_constraint_checks::assess_candidates, openai_story_generation::validate_options,
    story_prompt_preparation::fit_selection_and_build_request,
};
use yomibu_components::{
    japanese_constraint_checks::CandidateError,
    openai_story_generation::{Client, PreparedRequest, ProviderError},
    sudachi_dictionary::SudachiAnalyzer,
};
use yomibu_core::domain::story::{
    StoryAssessmentInputs, StoryCandidateAssessment, StoryError, StoryGenerationOptions,
    StoryRequest, StoryVocabularySelection,
};
use yomibu_core::domain::{
    candidate::{CandidateAssessment, GeneratedCandidates},
    embedding::{EmbeddingCache, EmbeddingModelIdentity},
    inventory::LearnerInventory,
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
    let (selection, prepared_request) =
        fit_selection_and_build_request(inventory, request, selection, options)?;
    let assessment_inputs = StoryAssessmentInputs::new(inventory, request, &selection)?;
    Ok(StoryGenerationPlan {
        selection,
        prepared_request,
        assessment_inputs,
    })
}

/// The shared execution entry point for CLI and other library callers. The caller
/// supplies resources and drives the future; candidate errors remain in the result.
pub async fn generate_story(
    plan: &StoryGenerationPlan<'_>,
    client: &Client,
    analyzer: &SudachiAnalyzer,
) -> Result<StoryGenerationResult, ProviderError> {
    let generated = client.generate_candidates(&plan.prepared_request).await?;
    let assessments = assess_candidates(&generated, &plan.assessment_inputs, analyzer)
        .into_iter()
        .map(StoryCandidateAssessment::into_owned)
        .collect();
    Ok(StoryGenerationResult {
        generated,
        assessments,
    })
}

/// Complete offline preflight: finalized selection, exact payload and full-inventory
/// assessment inputs. Fields are private so execution cannot replace those inputs.
///
/// ```no_run
/// use yomibu::application::story::{plan_generation, generate_story};
/// use yomibu_core::domain::{inventory::LearnerInventory, story::StoryRequest, embedding::EmbeddingCache};
/// use yomibu_components::{sudachi_dictionary::{installation::ManagedInstallation, SudachiAnalyzer}, openai_story_generation::Client};
/// # async unsafe fn example(inventory: &LearnerInventory, request: &StoryRequest,
/// #     cache: &EmbeddingCache, dictionary_dir: &std::path::Path, api_key: &str)
/// #     -> Result<(), Box<dyn std::error::Error>> {
/// let plan = plan_generation(inventory, request, cache, &cache.model, 12, Default::default())?;
/// // The caller keeps this imported generation unchanged for the analyzer's lifetime.
/// let installation = ManagedInstallation::open(dictionary_dir)?;
/// let analyzer = unsafe { SudachiAnalyzer::load(installation) }?;
/// let client = Client::new(api_key)?;
/// let result = generate_story(&plan, &client, &analyzer).await?;
/// assert_eq!(result.candidates().passages().iter().map(|p| p.sentence_spans.len()).sum::<usize>(), result.assessments().len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct StoryGenerationPlan<'a> {
    selection: StoryVocabularySelection<'a>,
    prepared_request: PreparedRequest,
    assessment_inputs: StoryAssessmentInputs<'a>,
}
impl<'a> StoryGenerationPlan<'a> {
    pub fn selection(&self) -> &StoryVocabularySelection<'a> {
        &self.selection
    }
    pub fn prepared_request(&self) -> &PreparedRequest {
        &self.prepared_request
    }
    pub fn generation_options(&self) -> StoryGenerationOptions {
        self.prepared_request.options().clone()
    }
}

/// Owns every original candidate, available assessment and typed execution error.
#[derive(Debug)]
pub struct StoryGenerationResult {
    generated: GeneratedCandidates,
    assessments: Vec<StoryCandidateAssessment<'static, CandidateError>>,
}
impl StoryGenerationResult {
    pub fn candidates(&self) -> &GeneratedCandidates {
        &self.generated
    }
    pub fn assessments(&self) -> &[StoryCandidateAssessment<'static, CandidateError>] {
        &self.assessments
    }
    pub fn has_execution_errors(&self) -> bool {
        self.assessments
            .iter()
            .any(|a| matches!(a.assessment, CandidateAssessment::ExecutionError { .. }))
    }
}
