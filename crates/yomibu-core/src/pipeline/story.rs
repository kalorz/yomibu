use crate::{
    capabilities::{CandidateGenerator, StoryPreparer},
    domain::{
        candidate::{CandidateConstructionError, GeneratedCandidates},
        inventory::LearnerInventory,
        story::{
            StoryAssessmentInputs, StoryGenerationOptions, StoryRequest, StoryVocabularySelection,
        },
    },
    pipeline::selection::validate_selection,
};

/// Prepare once, then bind the returned selection to the caller's inventory and request.
pub fn prepare_story<'a, P: StoryPreparer>(
    preparer: &P,
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    selection: StoryVocabularySelection<'a>,
    options: StoryGenerationOptions,
) -> Result<PreparedStory<'a, P::PreparedRequest>, P::Error> {
    let (selection, prepared_request) =
        preparer.prepare(inventory, request, selection, options.clone())?;
    validate_selection(inventory, request, &selection)?;
    let assessment_inputs = StoryAssessmentInputs::new(inventory, request, &selection)?;
    Ok(PreparedStory {
        selection,
        prepared_request,
        options,
        assessment_inputs,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum GenerationError<E> {
    #[error(transparent)]
    Generator(E),
    #[error(transparent)]
    InvalidCandidates(#[from] CandidateConstructionError),
}

/// Immutable finalized selection, provider request (including its options), and
/// bound full-inventory assessment inputs. Execution cannot replace these inputs.
///
/// Plans cannot be assembled from independently supplied request bytes and inputs.
/// ```compile_fail
/// use yomibu_core::{domain::{inventory::LearnerInventory, story::{StoryRequest, StoryVocabularySelection}}, pipeline::story::PreparedStory};
/// fn rebind<'a>(inventory: &'a LearnerInventory, request: &'a StoryRequest,
///     selection: StoryVocabularySelection<'a>) {
///     let _ = PreparedStory::new(inventory, request, selection, (), Default::default());
/// }
/// ```
#[derive(Debug)]
pub struct PreparedStory<'a, R> {
    selection: StoryVocabularySelection<'a>,
    prepared_request: R,
    options: StoryGenerationOptions,
    assessment_inputs: StoryAssessmentInputs<'a>,
}

impl<'a, R> PreparedStory<'a, R> {
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
    ) -> Result<GeneratedCandidates, GenerationError<G::Error>> {
        let generated = generator
            .generate_candidates(&self.prepared_request)
            .await
            .map_err(GenerationError::Generator)?;
        let actual = generated.passages().len();
        if actual != self.options.candidate_count {
            return Err(CandidateConstructionError::CandidateCount {
                expected: self.options.candidate_count,
                actual,
            }
            .into());
        }
        let (min, max) = self.options.format.sentence_bounds();
        for passage in generated.passages() {
            let actual = passage.sentence_spans.len();
            if !(min..=max).contains(&actual) {
                return Err(CandidateConstructionError::SentenceCount {
                    format: self.options.format,
                    actual,
                }
                .into());
            }
        }
        Ok(generated)
    }
}
