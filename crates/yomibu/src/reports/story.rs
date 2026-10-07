//! Shared preview and generation report projections; no execution or output.
use super::candidate;
use crate::story::{
    PlanDeparture, StoryGenerationPlan, StoryGenerationResult, StoryRequest,
    StoryVocabularySelection, TargetObservation,
};
use serde::Serialize;
#[derive(Serialize)]
pub struct RequestBytes<'a> {
    pub body_utf8: &'a str,
    pub bytes: usize,
    pub sha256: &'a str,
    pub prompt_revision: &'static str,
}
#[derive(Serialize)]
pub struct StoryPreviewReport<'a> {
    pub version: u32,
    pub kind: &'static str,
    pub request: &'a StoryRequest,
    pub plan: &'a StoryVocabularySelection<'a>,
    pub generation_options: crate::story::StoryGenerationOptions,
    pub provider_request: RequestBytes<'a>,
}
impl<'a> StoryPreviewReport<'a> {
    pub fn new(request: &'a StoryRequest, plan: &'a StoryGenerationPlan<'a>) -> Self {
        let ai_request = plan.ai_model_request();
        Self {
            version: 1,
            kind: "story_generation_plan_preview",
            request,
            plan: plan.selection(),
            generation_options: ai_request.options(),
            provider_request: RequestBytes {
                body_utf8: ai_request.body_utf8(),
                bytes: ai_request.body_utf8().len(),
                sha256: ai_request.sha256(),
                prompt_revision: crate::story::STORY_PROMPT_REVISION,
            },
        }
    }
}
#[derive(Serialize)]
pub struct StoryCandidateReport<'a> {
    #[serde(flatten)]
    pub candidate: candidate::CandidateReport<'a>,
    pub targets: &'a [TargetObservation],
    pub plan_departures: &'a [PlanDeparture],
}
#[derive(Serialize)]
pub struct StoryReport<'a> {
    pub version: u32,
    pub kind: &'static str,
    pub notice: &'static str,
    pub plan: StoryPreviewReport<'a>,
    pub generation: &'a crate::candidate::GenerationProvenance,
    pub candidates: Vec<StoryCandidateReport<'a>>,
}
impl<'a> StoryReport<'a> {
    pub fn new(
        request: &'a StoryRequest,
        plan: &'a StoryGenerationPlan<'a>,
        result: &'a StoryGenerationResult,
    ) -> Self {
        let generated = result.candidates();
        Self {
            version: 1,
            kind: "experimental_story_candidates",
            notice: candidate::NOTICE,
            plan: StoryPreviewReport::new(request, plan),
            generation: generated.provenance(),
            candidates: result
                .assessments()
                .iter()
                .enumerate()
                .map(|(i, assessment)| StoryCandidateReport {
                    candidate: candidate::CandidateReport::new(
                        i + 1,
                        &generated.texts()[i],
                        &assessment.assessment,
                    ),
                    targets: &assessment.targets,
                    plan_departures: &assessment.plan_departures,
                })
                .collect(),
        }
    }
}
