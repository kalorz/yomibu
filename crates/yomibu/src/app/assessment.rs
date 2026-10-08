use super::{
    config::Configuration,
    local::ApplicationError,
    modules::{ModuleId, ModuleState},
    reporting::{ProgressEvent, RunProgress, Step},
};
use crate::{
    adapters::{dictionary::ManagedInstallation, sudachi::SudachiAnalyzer},
    candidate::GeneratedCandidates,
    story::{StoryAssessmentInputs, StoryPassageAssessment, assess_passages},
};

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
) -> Vec<StoryPassageAssessment> {
    if !config.enabled(ModuleId::Assessment) {
        progress.skip(Step::Assessment, "Assessment disabled");
        return assess_passages(generated.passages(), inputs, None);
    }
    if !config.dictionary_dir_explicit
        && std::fs::symlink_metadata(&config.dictionary_dir)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        progress.skip(Step::Assessment, "No dictionary configured");
        return assess_passages(generated.passages(), inputs, None);
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
    let assessments = assess_passages(generated.passages(), inputs, analyzer.as_ref());
    for sentence in assessments.iter().flat_map(|passage| &passage.sentences) {
        if let crate::candidate::CandidateAssessment::ExecutionError { error, .. } =
            &sentence.assessment.assessment
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
