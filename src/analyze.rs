//! Executable-only input, composition, and presentation for offline analysis.

use std::{
    io::{Read, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use yomibu::{
    adapters::sudachi::SudachiAnalyzer,
    analysis::{Sentence, SentenceAnalysis},
    evaluation::{
        CheckKind, CheckOutcome, CheckState, Evaluation, EvaluationBindings, UnassessedAspect,
        evaluate,
    },
    grammar::GrammarDeclarations,
};

const MAX_INPUT_BYTES: usize = 64 * 1024;

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

fn write_json(out: &mut impl Write, report: &Report<'_>) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    let mut start = 0;
    // Serde escapes C0 controls. DEL and nonprinting Unicode also need escaping
    // for terminals; JSON requires UTF-16 escapes, not Rust's \u{...} syntax.
    for (index, character) in json.char_indices() {
        if character == '\u{7f}'
            || (!character.is_ascii() && character.escape_debug().next() == Some('\\'))
        {
            out.write_all(&json.as_bytes()[start..index])?;
            for unit in character.encode_utf16(&mut [0; 2]) {
                write!(out, "\\u{unit:04x}")?;
            }
            start = index + character.len_utf8();
        }
    }
    out.write_all(&json.as_bytes()[start..])?;
    writeln!(out)?;
    Ok(())
}

fn load_input(path: &Path) -> Result<Input> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .context("Cannot open the explicitly supplied analysis input")?
        .take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .context("Cannot read the explicitly supplied analysis input")?;
    if bytes.len() > MAX_INPUT_BYTES {
        bail!("Analysis input exceeds 64 KiB (65536 bytes).");
    }
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
    for (label, kind) in [
        ("Vocabulary", CheckKind::Vocabulary),
        ("Inflection", CheckKind::Inflection),
        ("Particles", CheckKind::Particles),
        ("Nominal です", CheckKind::Nominal),
        ("Scope", CheckKind::Scope),
    ] {
        let check = report.evaluation.check(kind);
        writeln!(out, "{label}: {}", state_label(check.state))?;
        writeln!(out, "  Coverage: {}", check.coverage)?;
        for finding in &check.findings {
            writeln!(
                out,
                "  bytes {}..{}: \"{}\" — {}",
                finding.span.start,
                finding.span.end,
                original[finding.span.clone()].escape_debug(),
                finding.reason
            )?;
        }
    }
    for aspect in report.evaluation.unassessed {
        let label = match aspect {
            UnassessedAspect::Naturalness => "Naturalness",
            UnassessedAspect::MultiwordExpressions => "Multiword expressions",
            UnassessedAspect::ContextualReadingAndSense => "Contextual reading and sense",
        };
        writeln!(out, "{label}: not assessed")?;
    }
    Ok(())
}

fn state_label(state: CheckState) -> &'static str {
    match state {
        CheckState::Completed(CheckOutcome::Pass) => "Pass",
        CheckState::Completed(CheckOutcome::Fail) => "Fail",
        CheckState::Completed(CheckOutcome::Inconclusive) => "Inconclusive",
        CheckState::NotRun => "NotRun",
    }
}
