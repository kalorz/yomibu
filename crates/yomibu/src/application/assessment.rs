use super::{
    local::ApplicationError,
    progress::{ProgressEvent, RunProgress, Step},
};
use crate::application::story::DefaultCandidateError;
use crate::configuration::Configuration;
use crate::configuration::modules::{ModuleId, ModuleState};
use yomibu_components::{
    japanese_constraint_checks::JapaneseConstraintChecks,
    sudachi_dictionary::{SudachiAnalyzer, installation::ManagedInstallation},
};
use yomibu_core::domain::{
    candidate::GeneratedCandidates,
    story::{StoryAssessmentInputs, StoryPassageAssessment},
};
use yomibu_core::pipeline::assessment::assess_passages;

/// # Safety
/// The selected dictionary must satisfy `SudachiAnalyzer::load` for the returned
/// analyzer's lifetime.
pub(super) unsafe fn load_analyzer(
    config: &Configuration,
) -> Result<SudachiAnalyzer, ApplicationError> {
    let installation = ManagedInstallation::open(&config.dictionary_dir)?;
    // The caller guarantees verified, unchanged files for the analyzer's lifetime.
    Ok(unsafe { SudachiAnalyzer::load(installation) }?)
}

/// # Safety
/// The selected dictionary must satisfy `SudachiAnalyzer::load` until this call returns.
pub(super) unsafe fn assess_optional<F: FnMut(ProgressEvent)>(
    config: &Configuration,
    generated: &GeneratedCandidates,
    inputs: &StoryAssessmentInputs<'_>,
    progress: &mut RunProgress<F>,
) -> Vec<StoryPassageAssessment<DefaultCandidateError>> {
    if !config.enabled(ModuleId::Assessment) {
        progress.skip(Step::Assessment, "Assessment disabled");
        return assess_with_defaults(generated, inputs, None);
    }
    if !config.dictionary_dir_explicit
        && std::fs::symlink_metadata(&config.dictionary_dir)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        progress.skip(Step::Assessment, "No dictionary configured");
        return assess_with_defaults(generated, inputs, None);
    }
    let started = progress.start(Step::Assessment);
    let analyzer = match unsafe { load_analyzer(config) } {
        Ok(analyzer) => {
            progress.state(ModuleId::Assessment, ModuleState::Available);
            Some(analyzer)
        }
        Err(error) => {
            progress.warn(ModuleId::Assessment, error.to_string());
            progress.state(
                ModuleId::Assessment,
                ModuleState::Unavailable {
                    error: error.to_string(),
                },
            );
            None
        }
    };
    let assessments = assess_with_defaults(generated, inputs, analyzer.as_ref());
    for sentence in assessments.iter().flat_map(|passage| &passage.sentences) {
        if let yomibu_core::domain::candidate::CandidateAssessment::ExecutionError {
            error, ..
        } = &sentence.assessment.assessment
        {
            progress.warn(ModuleId::Assessment, error.to_string());
            progress.state(
                ModuleId::Assessment,
                ModuleState::Unavailable {
                    error: error.to_string(),
                },
            );
        }
    }
    progress.finish(Step::Assessment, started);
    assessments
}

fn assess_with_defaults(
    generated: &GeneratedCandidates,
    inputs: &StoryAssessmentInputs<'_>,
    analyzer: Option<&SudachiAnalyzer>,
) -> Vec<StoryPassageAssessment<DefaultCandidateError>> {
    assess_passages(
        generated.passages(),
        inputs,
        analyzer.map(|analyzer| (analyzer, &JapaneseConstraintChecks)),
    )
    .into_iter()
    .map(|passage| passage.map_error(DefaultCandidateError))
    .collect()
}
