//! Executable-only explicit input, credentials, runtime and experimental reports.

use super::candidate_report::{CandidateReport, NOTICE, write_candidate, write_provenance};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};
use yomibu::{
    adapters::openai::{Client, ProviderError},
    evaluation::EvaluationBindings,
    generation::{CandidateAssessment, GenerationProvenance},
    grammar::GrammarDeclarations,
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Input {
    version: u32,
    grammar: Vec<String>,
    bindings: EvaluationBindings,
}

pub(super) fn run(
    dictionary: &super::dictionary::DictionaryArgs,
    input_path: &Path,
    json: bool,
    make_client: impl FnOnce(&str) -> Result<Client, ProviderError>,
    out: &mut impl Write,
) -> Result<()> {
    let input = load_input(input_path)?;
    let grammar = GrammarDeclarations::from_descriptions(input.grammar.iter().map(String::as_str))
        .context("Grammar input")?;
    input
        .bindings
        .validate(&grammar)
        .context("Evaluation bindings")?;
    let analyzer = dictionary.load().context("Dictionary initialization")?;
    let key = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Set OPENAI_API_KEY in the environment before explicitly requesting candidates."
            )
        })?;
    let client = make_client(&key)?;
    drop(key);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Generation runtime")?;
    let generated = runtime.block_on(client.generate_candidates(&grammar, &input.bindings))?;
    let assessments = generated.assess(&analyzer);
    let candidates = std::array::from_fn(|index| {
        CandidateReport::new(index + 1, &generated.texts()[index], &assessments[index])
    });
    let report = Report {
        version: 1,
        kind: "experimental_sentence_candidates",
        notice: NOTICE,
        input: &input,
        generation: generated.provenance(),
        candidates,
    };
    if json {
        super::cli_support::write_json(out, &report)?;
    } else {
        write_text(out, &report)?;
    }
    if assessments
        .iter()
        .any(|assessment| matches!(assessment, CandidateAssessment::ExecutionError { .. }))
    {
        bail!("One or both candidate executions failed; see the experimental report.");
    }
    Ok(())
}

fn load_input(path: &Path) -> Result<Input> {
    let bytes = super::cli_support::read_input(path, "Generation")?;
    let input: Input = serde_json::from_slice(&bytes).context("Invalid generation input JSON")?;
    if input.version != 1 {
        bail!(
            "Unsupported generation input version {}; expected 1.",
            input.version
        );
    }
    Ok(input)
}

#[derive(Serialize)]
struct Report<'a> {
    version: u32,
    kind: &'static str,
    notice: &'static str,
    input: &'a Input,
    generation: &'a GenerationProvenance,
    candidates: [CandidateReport<'a>; 2],
}

fn write_text(out: &mut impl Write, report: &Report<'_>) -> std::io::Result<()> {
    writeln!(out, "{}", report.notice)?;
    write_provenance(out, report.generation)?;
    for (index, description) in report.input.grammar.iter().enumerate() {
        writeln!(out, "Grammar {}: {}", index + 1, description.escape_debug())?;
    }
    writeln!(
        out,
        "Spans are half-open UTF-8 byte ranges in the original candidate."
    )?;
    for candidate in &report.candidates {
        write_candidate(out, candidate)?;
    }
    Ok(())
}
