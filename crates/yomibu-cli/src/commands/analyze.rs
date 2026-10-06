//! Read explicit input and configure the dictionary for library analysis/checks.
use crate::{args::DictionaryArgs, output};
use anyhow::{Context, Result, bail};
use std::{io::Write, path::Path};
use yomibu::{
    analysis::Sentence,
    evaluation::evaluate,
    grammar::GrammarDeclarations,
    reports::analysis::{AnalysisInput, AnalysisReport},
};

pub(crate) fn run(
    dictionary: &DictionaryArgs,
    input_path: &Path,
    json: bool,
    out: &mut impl Write,
) -> Result<()> {
    let input = load_input(input_path)?;
    let sentence = Sentence::new(&input.sentence).context("Sentence input")?;
    let grammar = GrammarDeclarations::from_descriptions(input.grammar.iter().map(String::as_str))
        .context("Grammar input")?;
    let analyzer = dictionary.load().context("Dictionary initialization")?;
    let analysis = analyzer.analyze(sentence).context("Analyzer execution")?;
    let evaluation = evaluate(&analysis, &grammar, &input.bindings)
        .context("Evaluation bindings or analysis")?;
    let report = AnalysisReport {
        version: 1,
        input: &input,
        analysis,
        outcome: evaluation.outcome(),
        evaluation,
    };
    if json {
        output::write_json(out, &report)?;
    } else {
        output::analysis::write_text(out, &report)?;
    }
    Ok(())
}

fn load_input(path: &Path) -> Result<AnalysisInput> {
    let bytes = yomibu::adapters::input_file::read_bounded(
        path,
        "Analysis",
        65536,
        "64 KiB (65536 bytes)",
    )?;
    let input: AnalysisInput =
        serde_json::from_slice(&bytes).context("Invalid analysis input JSON")?;
    if input.version != 1 {
        bail!(
            "Unsupported analysis input version {}; expected 1.",
            input.version
        );
    }
    Ok(input)
}
