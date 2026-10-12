use std::error::Error;

use crate::domain::{
    candidate::GeneratedCandidates,
    inventory::LearnerInventory,
    story::{StoryError, StoryGenerationOptions, StoryRequest, StoryVocabularySelection},
};

/// Synchronous preparation. The request must encode the finalized selection and
/// supplied options; its concrete type belongs to the matching provider.
/// Use [`prepare_story`](crate::pipeline::story::prepare_story) to bind this output
/// to the caller's inventory and request.
pub trait StoryPreparer {
    type PreparedRequest;
    type Error: Error + From<StoryError> + Send + Sync + 'static;

    fn prepare<'a>(
        &self,
        inventory: &'a LearnerInventory,
        request: &'a StoryRequest,
        selection: StoryVocabularySelection<'a>,
        options: StoryGenerationOptions,
    ) -> Result<(StoryVocabularySelection<'a>, Self::PreparedRequest), Self::Error>;
}

/// Executes a compatible request on the caller's executor, retaining typed failures.
/// Return original candidates with the prepared request's count and format.
pub trait CandidateGenerator {
    type PreparedRequest;
    type Error: Error + Send + Sync + 'static;

    fn generate_candidates(
        &self,
        request: &Self::PreparedRequest,
    ) -> impl Future<Output = Result<GeneratedCandidates, Self::Error>> + Send;
}
