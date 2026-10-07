//! Story workflow: offline planning, OpenAI execution and Sudachi assessment.

mod assessment;
mod observation;
mod preparation;
mod request;
mod selection;

pub use assessment::{
    PlanDeparture, StoryAssessmentInputs, StoryCandidateAssessment, StoryPassageAssessment,
    assess_candidates, assess_passages,
};
pub use observation::{
    TargetCoverage, TargetKind, TargetObservation, TargetState, TargetUncertainty,
    TargetUncertaintyReason, TargetUncertaintyScope,
};
pub use preparation::{STORY_PROMPT_REVISION, fit_selection_and_build_request};
pub use request::{
    PracticeTargets, StoryError, StoryFormat, StoryGenerationOptions, StoryRequest, StoryTopic,
};
pub use selection::{
    SelectedVocabulary, StoryVocabularySelection, select_builtin_vocabulary, select_vocabulary,
};

use crate::{
    adapters::{
        openai::{self, Client, PreparedRequest, ProviderError},
        sudachi::SudachiAnalyzer,
    },
    candidate::{CandidateAssessment, GeneratedCandidates},
    inventory::LearnerInventory,
    retrieval::{EmbeddingCache, EmbeddingModelIdentity},
};

impl StoryGenerationOptions {
    pub fn validate(&self) -> Result<(), StoryError> {
        openai::output_token_budget(self).map(|_| ()).map_err(|_| {
            StoryError::Invalid("candidate count must be positive and fit the output token budget")
        })
    }
}

/// Plan offline before initializing the caller's dictionary, credential or client.
pub fn plan_generation<'a>(
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    cache: &EmbeddingCache,
    model: &EmbeddingModelIdentity,
    selection_limit: usize,
    options: StoryGenerationOptions,
) -> Result<StoryGenerationPlan<'a>, StoryError> {
    options.validate()?;
    request.validate_selection_limit(selection_limit)?;
    request.validate(inventory)?;
    let selection =
        selection::select_vocabulary(inventory, request, cache, model, selection_limit)?;
    let (selection, prepared_request) =
        preparation::fit_selection_and_build_request(inventory, request, selection, options)?;
    let assessment_inputs = assessment::StoryAssessmentInputs::new(inventory, request, &selection)?;
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
    let assessments = assessment::assess_candidates(&generated, &plan.assessment_inputs, analyzer)
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
/// use yomibu::{
///     adapters::{openai::Client, sudachi::SudachiAnalyzer},
///     inventory::LearnerInventory,
///     story::{StoryRequest, plan_generation, generate_story},
///     retrieval::EmbeddingCache,
/// };
/// # async fn example(inventory: &LearnerInventory, request: &StoryRequest,
/// #     cache: &EmbeddingCache, dictionary: &std::path::Path, api_key: &str)
/// #     -> Result<(), Box<dyn std::error::Error>> {
/// let plan = plan_generation(inventory, request, cache, &cache.model, 12, Default::default())?;
/// let analyzer = SudachiAnalyzer::load(dictionary)?;
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
    assessments: Vec<StoryCandidateAssessment<'static>>,
}
impl StoryGenerationResult {
    pub fn candidates(&self) -> &GeneratedCandidates {
        &self.generated
    }
    pub fn assessments(&self) -> &[StoryCandidateAssessment<'static>] {
        &self.assessments
    }
    pub fn has_execution_errors(&self) -> bool {
        self.assessments
            .iter()
            .any(|a| matches!(a.assessment, CandidateAssessment::ExecutionError { .. }))
    }
}
