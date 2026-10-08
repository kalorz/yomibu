use crate::{
    capabilities::CandidateGenerator,
    domain::{
        candidate::GeneratedCandidates,
        inventory::LearnerInventory,
        story::{StoryAssessmentInputs, StoryError, StoryRequest, StoryVocabularySelection},
    },
    pipeline::selection::validate_selection,
};

/// Immutable finalized selection, provider request (including its options), and
/// bound full-inventory assessment inputs. Execution cannot replace these inputs.
#[derive(Debug)]
pub struct PreparedStory<'a, R> {
    selection: StoryVocabularySelection<'a>,
    prepared_request: R,
    assessment_inputs: StoryAssessmentInputs<'a>,
}

impl<'a, R> PreparedStory<'a, R> {
    /// The preparer must supply the exact request encoded from this selection.
    pub fn new(
        inventory: &'a LearnerInventory,
        request: &'a StoryRequest,
        selection: StoryVocabularySelection<'a>,
        prepared_request: R,
    ) -> Result<Self, StoryError> {
        validate_selection(inventory, request, &selection)?;
        let assessment_inputs = StoryAssessmentInputs::new(inventory, request, &selection)?;
        Ok(Self {
            selection,
            prepared_request,
            assessment_inputs,
        })
    }

    pub fn selection(&self) -> &StoryVocabularySelection<'a> {
        &self.selection
    }

    pub fn prepared_request(&self) -> &R {
        &self.prepared_request
    }

    pub fn assessment_inputs(&self) -> &StoryAssessmentInputs<'a> {
        &self.assessment_inputs
    }

    pub fn into_parts(self) -> (StoryVocabularySelection<'a>, R) {
        (self.selection, self.prepared_request)
    }

    /// Calls the supplied generator once. Request compatibility is static.
    ///
    /// ```compile_fail
    /// use yomibu_core::{capabilities::CandidateGenerator, pipeline::story::PreparedStory};
    /// struct FirstRequest;
    /// struct OtherRequest;
    /// async fn mismatched<G: CandidateGenerator<PreparedRequest = OtherRequest>>(
    ///     plan: PreparedStory<'_, FirstRequest>, generator: &G,
    /// ) {
    ///     plan.generate(generator).await;
    /// }
    /// ```
    pub async fn generate<G: CandidateGenerator<PreparedRequest = R>>(
        &self,
        generator: &G,
    ) -> Result<GeneratedCandidates, G::Error> {
        generator.generate_candidates(&self.prepared_request).await
    }
}
