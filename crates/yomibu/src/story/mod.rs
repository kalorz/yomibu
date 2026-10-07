//! Shared story planning and execution, independent of the inventory source.
//!
//! The complete algorithm, in execution order:
//!
//! **Plan — [`plan_generation`] (offline)**
//!
//! 1. Validate inventory/request ([`StoryRequest::validate`]), selection limit
//!    ([`StoryRequest::validate_selection_limit`]) and options
//!    ([`StoryGenerationOptions::validate`]).
//! 2. [`select_vocabulary`]: rank cached embeddings; select targets and supports once.
//! 3. [`build_ai_model_request`]: trim optional supports to fit the request bounds;
//!    freeze the exact outbound bytes and hash. Never remove explicit targets.
//! 4. [`StoryAssessmentInputs::new`]: prepare structural evidence from the full
//!    original inventory, retaining that inventory for lexical assessment.
//!
//! **Caller boundary**: after planning succeeds, the CLI initializes the dictionary,
//! client and runtime. Preview ends at the plan and initializes none of them.
//!
//! **Generate — [`generate_story`]**
//!
//! 5. [`Client::generate_story_candidates`]: send the frozen bytes once, without retry.
//! 6. [`assess_candidates`], for every original candidate:
//!    - [`Sentence::new`](crate::analysis::Sentence::new): validate sentence bounds.
//!    - [`SudachiAnalyzer::analyze`]: analyze morphology with original UTF-8 spans.
//!    - [Inventory evaluation]: check vocabulary against the full inventory and
//!      grammar against the bounded structural rules, preserving uncertainty.
//!    - [Target observations]: record practice-target evidence from available analysis.
//!    - [`assess_candidates`]: record departures from the selected vocabulary;
//!      keep execution errors and partial evidence.
//! 7. Return [`StoryGenerationResult`]: owned original texts, provider metadata
//!    and every candidate's available assessment.
//!
//! A failed candidate stage stops its dependent checks; other candidates still run.
//! Fail/Inconclusive judgments, execution errors and NotRun remain distinct.
//! A provider failure returns an error without a result or a retry.
//!
//! [Inventory evaluation]: https://github.com/kalorz/yomibu/blob/main/crates/yomibu/src/evaluation/inventory.rs#L26
//! [Target observations]: https://github.com/kalorz/yomibu/blob/main/crates/yomibu/src/story/assessment.rs#L232
//!
//! The two entry functions below implement this sequence. Stages live in
//! `request.rs`, `selection.rs`, `model_request.rs` and `assessment.rs`;
//! public imports remain under `yomibu::story`.

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
/// assert_eq!(result.candidates().texts().len(), result.assessments().len());
/// # Ok(())
/// # }
/// ```
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
