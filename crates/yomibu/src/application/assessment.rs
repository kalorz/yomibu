use super::{
    local::ApplicationError,
    progress::{ProgressEvent, RunProgress, Step},
};
use crate::application::story::DefaultCandidateError;
use crate::configuration::Configuration;
use crate::configuration::modules::{ModuleId, ModuleState};
use yomibu_components::sudachi_dictionary::{SudachiAnalyzer, installation::ManagedInstallation};
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
    let installation = ManagedInstallation::open(&config.application.dictionary_dir)?;
    // The caller guarantees verified, unchanged files for the analyzer's lifetime.
    match config.pipeline.components.analysis {
        crate::configuration::components::Analysis::Sudachi => {
            Ok(unsafe { SudachiAnalyzer::load(installation) }?)
        }
    }
}

/// # Safety
/// The selected dictionary must satisfy `SudachiAnalyzer::load` until this call returns.
pub(super) unsafe fn assess_local_optional<F: FnMut(ProgressEvent)>(
    config: &Configuration,
    generated: &GeneratedCandidates,
    inputs: &StoryAssessmentInputs<'_>,
    progress: &mut RunProgress<F>,
) -> Vec<StoryPassageAssessment<DefaultCandidateError>> {
    if !config.enabled(ModuleId::Assessment) {
        progress.skip(Step::Assessment, "Assessment disabled");
        return assess_and_report(config, generated, inputs, None, progress, None);
    }
    if !config.application.dictionary_dir_explicit
        && std::fs::symlink_metadata(&config.application.dictionary_dir)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        progress.skip(Step::Assessment, "No dictionary configured");
        return assess_and_report(config, generated, inputs, None, progress, None);
    }
    let started = progress.start(Step::Assessment);
    let analyzer = match unsafe { load_analyzer(config) } {
        Ok(analyzer) => Some(analyzer),
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
    assess_and_report(
        config,
        generated,
        inputs,
        analyzer.as_ref(),
        progress,
        Some(started),
    )
}

pub(super) fn assess_optional<F: FnMut(ProgressEvent)>(
    config: &Configuration,
    generated: &GeneratedCandidates,
    inputs: &StoryAssessmentInputs<'_>,
    analyzer: Option<&SudachiAnalyzer>,
    progress: &mut RunProgress<F>,
) -> Vec<StoryPassageAssessment<DefaultCandidateError>> {
    let (analyzer, started) = if !config.enabled(ModuleId::Assessment) {
        progress.skip(Step::Assessment, "Assessment disabled");
        (None, None)
    } else if analyzer.is_none() {
        progress.skip(Step::Assessment, "No analyzer supplied");
        (None, None)
    } else {
        (analyzer, Some(progress.start(Step::Assessment)))
    };
    assess_and_report(config, generated, inputs, analyzer, progress, started)
}

fn assess_and_report<F: FnMut(ProgressEvent)>(
    config: &Configuration,
    generated: &GeneratedCandidates,
    inputs: &StoryAssessmentInputs<'_>,
    analyzer: Option<&SudachiAnalyzer>,
    progress: &mut RunProgress<F>,
    started: Option<std::time::Instant>,
) -> Vec<StoryPassageAssessment<DefaultCandidateError>> {
    if analyzer.is_some() {
        progress.state(ModuleId::Assessment, ModuleState::Available);
    }
    let assessor = config.pipeline.components.assessment.construct();
    let assessments: Vec<_> = assess_passages(
        generated.passages(),
        inputs,
        analyzer.map(|analyzer| (analyzer, &assessor)),
    )
    .into_iter()
    .map(|passage| passage.map_error(DefaultCandidateError))
    .collect();
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
    if let Some(started) = started {
        progress.finish(Step::Assessment, started);
    }
    assessments
}
