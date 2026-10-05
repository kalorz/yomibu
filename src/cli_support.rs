//! Shared executable-only bounded input and terminal-safe presentation.
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    io::{Read, Write},
    path::Path,
};
use yomibu::evaluation::{CheckKind, CheckOutcome, CheckState, Evaluation, UnassessedAspect};

pub(super) fn read_input(path: &Path, kind: &str) -> Result<Vec<u8>> {
    read_input_bounded(path, kind, 65536, "64 KiB (65536 bytes)")
}

pub(super) fn read_input_bounded(
    path: &Path,
    kind: &str,
    limit: usize,
    limit_label: &str,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .with_context(|| {
            format!(
                "Cannot open the explicitly supplied {} input",
                kind.to_ascii_lowercase()
            )
        })?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .with_context(|| {
            format!(
                "Cannot read the explicitly supplied {} input",
                kind.to_ascii_lowercase()
            )
        })?;
    if bytes.len() > limit {
        bail!("{kind} input exceeds {limit_label}.");
    }
    Ok(bytes)
}

pub(super) fn write_json(out: &mut impl Write, report: &impl Serialize) -> Result<()> {
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

pub(super) fn write_checks(
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

pub(super) fn state_label(state: CheckState) -> &'static str {
    match state {
        CheckState::Completed(CheckOutcome::Pass) => "Pass",
        CheckState::Completed(CheckOutcome::Fail) => "Fail",
        CheckState::Completed(CheckOutcome::Inconclusive) => "Inconclusive",
        CheckState::NotRun => "NotRun",
    }
}
