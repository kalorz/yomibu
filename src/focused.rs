//! Focused command composition. Start at `run_generation` for the full sequence;
//! `run_preview` stops after the same offline selection and request preparation.
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{io::Write, path::Path};
use yomibu::{
    adapters::openai::{Client, ProviderError, prepare_focused_request},
    evaluation::EvaluationBindings,
    generation::CandidateAssessment,
    generation_context::{ContextError, PERMISSION_FILE_LIMIT, VocabularyEntryId, select_context},
    grammar::GrammarDeclarations,
};

mod report;

pub(super) fn run_generation(
    path: &Path,
    focus: usize,
    dictionary: &super::dictionary::DictionaryArgs,
    json: bool,
    make_client: impl FnOnce(&str) -> Result<Client, ProviderError>,
    out: &mut impl Write,
) -> Result<()> {
    let input = read_permissions(path)?;
    let focus = VocabularyEntryId::new(focus)?;
    let context = match select_context(&input.grammar, &input.bindings, focus) {
        Ok(context) => context,
        Err(ContextError::Unavailable { situations, .. }) => {
            report::write_unavailable(out, &input, focus, &situations, json)?;
            bail!("Focused context is unavailable; see the declared prerequisite decisions.");
        }
        Err(error) => return Err(error.into()),
    };
    let prepared = prepare_focused_request(context)?;

    let analyzer = dictionary.load().context("Dictionary initialization")?;
    let key = read_openai_key()?;
    let client = make_client(&key)?;
    drop(key);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Generation runtime")?;

    let generated = runtime.block_on(client.generate_focused_candidates(&prepared))?;
    let assessments = generated.assess_focused(&analyzer, prepared.context());

    let report = report::GenerationReport::new(&prepared, &generated, &assessments);
    report::write_generation(out, &report, json)?;
    if assessments.iter().any(|candidate| {
        matches!(
            &candidate.assessment,
            CandidateAssessment::ExecutionError { .. }
        )
    }) {
        bail!("One or both candidate executions failed; see the experimental report.");
    }
    Ok(())
}

/// Inspect the same preparation stages without initializing execution resources.
pub(super) fn run_preview(
    path: &Path,
    focus: usize,
    json: bool,
    out: &mut impl Write,
) -> Result<()> {
    let input = read_permissions(path)?;
    let focus = VocabularyEntryId::new(focus)?;
    let context = match select_context(&input.grammar, &input.bindings, focus) {
        Ok(context) => context,
        Err(ContextError::Unavailable { situations, .. }) => {
            report::write_unavailable(out, &input, focus, &situations, json)?;
            bail!("Focused context is unavailable; see the declared prerequisite decisions.");
        }
        Err(error) => return Err(error.into()),
    };
    let prepared = prepare_focused_request(context)?;
    let report = report::ContextReport::ready(&prepared);
    report::write_preview(out, &report, json)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    version: u32,
    grammar: Vec<String>,
    bindings: EvaluationBindings,
}

struct Permissions {
    grammar: GrammarDeclarations,
    bindings: EvaluationBindings,
}

fn read_permissions(path: &Path) -> Result<Permissions> {
    let bytes = super::cli_support::read_input_bounded(
        path,
        "Focused permission",
        PERMISSION_FILE_LIMIT,
        "4 MiB (4194304 bytes)",
    )?;
    let input: Input = serde_json::from_slice(&bytes).context("Invalid focused permission JSON")?;
    if input.version != 1 {
        bail!(
            "Unsupported focused permission version {}; expected 1.",
            input.version
        );
    }
    let grammar = GrammarDeclarations::from_descriptions(input.grammar).context("Grammar input")?;
    Ok(Permissions {
        grammar,
        bindings: input.bindings,
    })
}

fn read_openai_key() -> Result<String> {
    std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Set OPENAI_API_KEY in the environment before explicitly requesting candidates."
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preflight_and_preview_never_construct_a_client_and_output_errors_propagate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("permissions.json");
        let input = include_str!("../tests/fixtures/focused/pet-rest.json");
        std::fs::write(&path, input).unwrap();
        let mut out = Vec::new();
        run_preview(&path, 1, true, &mut out).unwrap();
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(run_preview(&path, 1, true, &mut Broken).is_err());
        for contents in [
            input.to_owned(),
            "{".into(),
            input.replace("\"TopicWa\"", "\"ObjectWo\""),
        ] {
            std::fs::write(&path, contents).unwrap();
            assert!(
                run_generation(
                    &path,
                    1,
                    &crate::dictionary::DictionaryArgs {
                        dictionary: Some("missing.dic".into()),
                        dictionary_dir: None,
                    },
                    true,
                    |_| panic!("preflight must not construct a client"),
                    &mut Vec::new()
                )
                .is_err()
            );
        }
    }
}
