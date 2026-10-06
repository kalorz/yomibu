//! Terminal presentation of cached observations.
use chrono::SecondsFormat;
use std::io::{self, Write};
use yomibu::summary::{Accuracy, Summary};

pub(crate) fn write_status(out: &mut impl Write, summary: &Summary) -> io::Result<()> {
    writeln!(out, "Cached WaniKani observations")?;
    writeln!(out, "User: {} (level {})", summary.username, summary.level)?;
    writeln!(
        out,
        "Sync started: {}",
        summary
            .sync_started_at
            .to_rfc3339_opts(SecondsFormat::AutoSi, true)
    )?;
    writeln!(
        out,
        "Sync completed: {}",
        summary
            .sync_completed_at
            .to_rfc3339_opts(SecondsFormat::AutoSi, true)
    )?;
    writeln!(out, "Synchronized kanji: {}", summary.kanji)?;
    writeln!(
        out,
        "Synchronized vocabulary: {} (kana-only: {})",
        summary.vocabulary, summary.kana_vocabulary
    )?;
    writeln!(out, "Hidden subjects: {}", summary.hidden_subjects)?;
    writeln!(
        out,
        "Unavailable content (access limit): {}",
        summary.unavailable_content
    )?;
    writeln!(out, "Raw SRS stages (assignments):")?;
    if summary.srs_stages.is_empty() {
        writeln!(out, "  no assignments")?;
    }
    for (system, stages) in &summary.srs_stages {
        match system {
            Some(id) => write!(out, "  System {id}: ")?,
            None => write!(out, "  System unavailable (content excluded): ")?,
        }
        for (index, (stage, count)) in stages.iter().enumerate() {
            if index > 0 {
                write!(out, ", ")?;
            }
            write!(out, "stage {stage}: {count}")?;
        }
        writeln!(out)?;
    }
    write_accuracy(out, "Reading", &summary.reading_accuracy)?;
    write_accuracy(out, "Meaning", &summary.meaning_accuracy)
}

fn write_accuracy(out: &mut impl Write, label: &str, accuracy: &Accuracy) -> io::Result<()> {
    match accuracy.percentage() {
        Some(percent) => writeln!(
            out,
            "{label} accuracy: {percent:.2}% ({}/{})",
            accuracy.correct(),
            accuracy.total()
        ),
        None => writeln!(out, "{label} accuracy: no reviews"),
    }
}
