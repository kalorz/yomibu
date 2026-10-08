use std::error::Error;

use crate::{
    domain::{
        analysis::{Sentence, SentenceAnalysis},
        candidate::GeneratedCandidates,
        inventory::LearnerInventory,
        story::{
            StoryAssessmentInputs, StoryFindings, StoryGenerationOptions, StoryRequest,
            StoryVocabularySelection,
        },
    },
    pipeline::story::PreparedStory,
};

/// Synchronous preparation. The request must encode the finalized selection and
/// supplied options; its concrete type belongs to the matching provider.
pub trait StoryPreparer {
    type PreparedRequest;
    type Error: Error + Send + Sync + 'static;

    fn prepare<'a>(
        &self,
        inventory: &'a LearnerInventory,
        request: &'a StoryRequest,
        selection: StoryVocabularySelection<'a>,
        options: StoryGenerationOptions,
    ) -> Result<PreparedStory<'a, Self::PreparedRequest>, Self::Error>;
}

/// Analyze unchanged text with original byte spans. Resource loading is separate;
/// callers must retain any backing resources and their safety guarantees.
pub trait SentenceAnalyzer {
    type Error: Error + Send + Sync + 'static;

    fn analyze<'a>(&self, sentence: Sentence<'a>) -> Result<SentenceAnalysis<'a>, Self::Error>;
}

/// Judge supplied evidence against the full inventory and finalized prompt plan.
/// Implementations do not load resources or rerun sentence analysis.
pub trait StoryAssessor {
    type Error: Error + Send + Sync + 'static;

    fn assess<'a>(
        &self,
        analysis: &'a SentenceAnalysis<'_>,
        inputs: &StoryAssessmentInputs<'_>,
    ) -> Result<StoryFindings<'a>, Self::Error>;
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
