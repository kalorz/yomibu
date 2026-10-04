//! Executable-only input, composition, and presentation for offline analysis.

use std::{io::Write, path::Path};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use yomibu::{
    adapters::sudachi::SudachiAnalyzer,
    analysis::{Sentence, SentenceAnalysis},
    evaluation::{CheckState, Evaluation, EvaluationBindings, evaluate},
    grammar::GrammarDeclarations,
};

use super::cli_support::{read_input, state_label, write_checks, write_json};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Input {
    version: u32,
    sentence: String,
    grammar: Vec<String>,
    bindings: EvaluationBindings,
}

#[derive(Serialize)]
struct Report<'a> {
    version: u32,
    input: &'a Input,
    analysis: SentenceAnalysis<'a>,
    outcome: CheckState,
    evaluation: Evaluation,
}

pub(super) fn run(
    dictionary: &Path,
    input_path: &Path,
    json: bool,
    out: &mut impl Write,
) -> Result<()> {
    let input = load_input(input_path)?;
    let sentence = Sentence::new(&input.sentence).context("Sentence input")?;
    let grammar = GrammarDeclarations::from_descriptions(input.grammar.iter().map(String::as_str))
        .context("Grammar input")?;
    let analyzer = SudachiAnalyzer::load(dictionary).context("Dictionary initialization")?;
    let analysis = analyzer.analyze(sentence).context("Analyzer execution")?;
    let evaluation = evaluate(&analysis, &grammar, &input.bindings)
        .context("Evaluation bindings or analysis")?;
    let report = Report {
        version: 1,
        input: &input,
        analysis,
        outcome: evaluation.outcome(),
        evaluation,
    };
    if json {
        write_json(out, &report)?;
    } else {
        write_text(out, &report)?;
    }
    Ok(())
}

fn load_input(path: &Path) -> Result<Input> {
    let bytes = read_input(path, "Analysis")?;
    let input: Input = serde_json::from_slice(&bytes).context("Invalid analysis input JSON")?;
    if input.version != 1 {
        bail!(
            "Unsupported analysis input version {}; expected 1.",
            input.version
        );
    }
    Ok(input)
}

fn write_text(out: &mut impl Write, report: &Report<'_>) -> std::io::Result<()> {
    writeln!(out, "{}", report.evaluation.notice)?;
    writeln!(out, "Overall: {} (completed)", state_label(report.outcome))?;
    let original = report.analysis.sentence.text();
    writeln!(out, "Sentence: \"{}\"", original.escape_debug())?;
    for (index, description) in report.input.grammar.iter().enumerate() {
        writeln!(out, "Grammar {}: {}", index + 1, description.escape_debug())?;
    }
    writeln!(
        out,
        "Spans are half-open UTF-8 byte ranges in the original sentence."
    )?;
    write_checks(out, original, &report.evaluation)
}
