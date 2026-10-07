//! Shared story planning and execution, independent of the inventory source.
//!
//! Read [`plan_generation`] then [`generate_story`] for the complete sequence.
//! Stages live in `request.rs`, `selection.rs`, `model_request.rs` and
//! `assessment.rs`; their public imports remain under `yomibu::story`.
//! Planning is offline; callers initialize execution resources after it succeeds.
//!
//! ```no_run
//! use yomibu::{
//!     adapters::{openai::Client, sudachi::SudachiAnalyzer},
//!     inventory::LearnerInventory,
//!     story::{StoryRequest, plan_generation, generate_story},
//!     retrieval::EmbeddingCache,
//! };
//! # async fn example(inventory: &LearnerInventory, request: &StoryRequest,
//! #     cache: &EmbeddingCache, dictionary: &std::path::Path, api_key: &str)
//! #     -> Result<(), Box<dyn std::error::Error>> {
//! let plan = plan_generation(inventory, request, cache, &cache.model, 12, Default::default())?;
//! let analyzer = SudachiAnalyzer::load(dictionary)?;
//! let client = Client::new(api_key)?;
//! let result = generate_story(&plan, &client, &analyzer).await?;
//! assert_eq!(result.candidates().texts().len(), result.assessments().len());
//! # Ok(())
//! # }
//! ```

mod assessment;
mod model_request;
mod request;
mod selection;

pub use assessment::{
    PlanDeparture, StoryAssessmentInputs, StoryCandidateAssessment, StoryCandidates,
    TargetObservation, assess_candidates,
};
pub use model_request::{AiModelRequest, STORY_PROMPT_REVISION, build_ai_model_request};
pub use request::{PracticeTargets, StoryError, StoryGenerationOptions, StoryRequest};
pub use selection::{SelectedVocabulary, StoryVocabularySelection, select_vocabulary};

use crate::{
    adapters::{
        openai::{Client, ProviderError},
        sudachi::SudachiAnalyzer,
    },
    generation::CandidateAssessment,
    inventory::LearnerInventory,
    retrieval::{EmbeddingCache, EmbeddingModelIdentity},
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
    options.validate()?;
    request.validate_selection_limit(selection_limit)?;
    request.validate(inventory)?;
    let selection =
        selection::select_vocabulary(inventory, request, cache, model, selection_limit)?;
    let (selection, ai_request) =
        model_request::build_ai_model_request(inventory, request, selection, options)?;
    let assessment_inputs = assessment::StoryAssessmentInputs::new(inventory, request, &selection)?;
    Ok(StoryGenerationPlan {
        selection,
        ai_request,
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
    let generated = client.generate_story_candidates(&plan.ai_request).await?;
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
#[derive(Debug)]
pub struct StoryGenerationPlan<'a> {
    selection: StoryVocabularySelection<'a>,
    ai_request: AiModelRequest,
    assessment_inputs: StoryAssessmentInputs<'a>,
}
impl<'a> StoryGenerationPlan<'a> {
    pub fn selection(&self) -> &StoryVocabularySelection<'a> {
        &self.selection
    }
    pub fn ai_model_request(&self) -> &AiModelRequest {
        &self.ai_request
    }
}

/// Owns every original candidate, available assessment and typed execution error.
#[derive(Debug)]
pub struct StoryGenerationResult {
    generated: StoryCandidates,
    assessments: Vec<StoryCandidateAssessment<'static>>,
}
impl StoryGenerationResult {
    pub fn candidates(&self) -> &StoryCandidates {
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
