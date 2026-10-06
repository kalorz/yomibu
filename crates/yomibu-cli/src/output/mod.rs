//! Terminal-safe rendering; shared serializable report data belongs to the library.
use anyhow::{Result, anyhow};
use clap::error::{ContextKind, ContextValue};
use serde::Serialize;
use std::io::Write;
use yomibu::evaluation::{CheckKind, CheckOutcome, CheckState, Evaluation, UnassessedAspect};

pub(crate) mod analysis;
pub(crate) mod candidate;
pub(crate) mod dictionary;
pub(crate) mod status;
pub(crate) mod story;

pub(crate) fn escape_argument_error(mut error: clap::Error) -> clap::Error {
    // Escape supplied values before Clap adds its diagnostic layout.
    for kind in [
        ContextKind::InvalidArg,
        ContextKind::InvalidValue,
        ContextKind::InvalidSubcommand,
    ] {
        if let Some(ContextValue::String(value)) = error.get(kind) {
            let escaped = value.escape_debug().to_string();
            error.insert(kind, ContextValue::String(escaped));
        }
    }
    error
}

pub(crate) fn command_error(error: anyhow::Error) -> anyhow::Error {
    anyhow!("{}", format!("{error:#}").escape_debug())
}

pub(crate) fn write_json(out: &mut impl Write, report: &impl Serialize) -> Result<()> {
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

pub(crate) fn write_checks(
    out: &mut impl Write,
    original: &str,
    evaluation: &Evaluation,
) -> std::io::Result<()> {
    for (label, kind) in [
        ("Vocabulary", CheckKind::Vocabulary),
        ("Inflection", CheckKind::Inflection),
        ("Particles", CheckKind::Particles),
        ("Nominal です", CheckKind::Nominal),
        ("Scope", CheckKind::Scope),
    ] {
        let check = evaluation.check(kind);
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
    for aspect in evaluation.unassessed {
        let label = match aspect {
            UnassessedAspect::Naturalness => "Naturalness",
            UnassessedAspect::MultiwordExpressions => "Multiword expressions",
            UnassessedAspect::ContextualReadingAndSense => "Contextual reading and sense",
        };
        writeln!(out, "{label}: not assessed")?;
    }
    Ok(())
}

pub(crate) fn state_label(state: CheckState) -> &'static str {
    match state {
        CheckState::Completed(CheckOutcome::Pass) => "Pass",
        CheckState::Completed(CheckOutcome::Fail) => "Fail",
        CheckState::Completed(CheckOutcome::Inconclusive) => "Inconclusive",
        CheckState::NotRun => "NotRun",
    }
}
