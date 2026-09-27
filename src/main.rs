use anyhow::anyhow;
use chrono::SecondsFormat;
use clap::{Parser, Subcommand};
use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};
use yomibu::{
    cache,
    summary::{Accuracy, Summary},
};

#[derive(Parser)]
#[command(version, about = "Inspect cached WaniKani learner observations")]
struct Cli {
    /// Directory containing wanikani.json (default: $HOME/.yomibu).
    #[arg(long, global = true, value_name = "PATH")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show cached observations without accessing the network.
    Status,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let data_dir = cli
        .data_dir
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(|home| PathBuf::from(home).join(".yomibu"))
        })
        .ok_or_else(|| anyhow!("HOME is unavailable; specify --data-dir PATH."))?;
    match cli.command {
        Command::Status => {
            let snapshot = cache::load(&data_dir)?;
            let summary = snapshot.summarize()?;
            write_status(&mut io::stdout().lock(), &summary)?;
        }
    }
    Ok(())
}

fn write_status(out: &mut impl Write, summary: &Summary<'_>) -> io::Result<()> {
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
