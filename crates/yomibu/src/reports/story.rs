//! Shared preview and generation report projections; no execution or output.
use super::candidate;
use crate::application::story::{StoryGenerationPlan, StoryGenerationResult};
use serde::Serialize;
use yomibu_core::domain::story::{
    PlanDeparture, StoryRequest, StoryVocabularySelection, TargetObservation,
};
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
    pub generation_options: yomibu_core::domain::story::StoryGenerationOptions,
    pub provider_request: RequestBytes<'a>,
}
impl<'a> StoryPreviewReport<'a> {
    pub fn new(request: &'a StoryRequest, plan: &'a StoryGenerationPlan<'a>) -> Self {
        let prepared_request = plan.prepared_request();
        Self {
            version: 1,
            kind: "story_generation_plan_preview",
            request,
            plan: plan.selection(),
            generation_options: plan.generation_options(),
            provider_request: RequestBytes {
                body_utf8: prepared_request.body_utf8(),
                bytes: prepared_request.body_utf8().len(),
                sha256: prepared_request.sha256(),
                prompt_revision: prepared_request.prompt_revision(),
            },
        }
    }
}
#[derive(Serialize)]
pub struct StoryCandidateReport<'a> {
    pub passage_index: usize,
    pub sentence_span: &'a std::ops::Range<usize>,
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
    pub generation: &'a yomibu_core::domain::candidate::GenerationProvenance,
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
            candidates: generated
                .passages()
                .iter()
                .enumerate()
                .flat_map(|(index, passage)| {
                    passage
                        .sentence_spans
                        .iter()
                        .map(move |span| (index + 1, span, &passage.text[span.clone()]))
                })
                .zip(result.assessments())
                .enumerate()
                .map(|(i, ((passage_index, sentence_span, text), assessment))| {
                    StoryCandidateReport {
                        passage_index,
                        sentence_span,
                        candidate: candidate::CandidateReport::new(
                            i + 1,
                            text,
                            &assessment.assessment,
                        ),
                        targets: &assessment.targets,
                        plan_departures: &assessment.plan_departures,
                    }
                })
                .collect(),
        }
    }
}
