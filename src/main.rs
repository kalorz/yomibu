use anyhow::anyhow;
use chrono::SecondsFormat;
use clap::{Parser, Subcommand};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};
use yomibu::{
    App,
    adapters::stores::FileLearningStore,
    app::SyncReport,
    summary::{Accuracy, Summary},
    wanikani::Client,
};

#[derive(Parser)]
#[command(
    version,
    about = "Synchronize and inspect cached observations (unofficial WaniKani tool)"
)]
struct Cli {
    /// Directory containing wanikani.json (default: $HOME/.yomibu).
    #[arg(long, global = true, value_name = "PATH")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Refresh the complete cache using WANIKANI_API_TOKEN.
    Sync,
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
        Command::Sync => {
            let token = std::env::var("WANIKANI_API_TOKEN")
                .ok()
                .filter(|token| !token.trim().is_empty())
                .ok_or_else(|| {
                    anyhow!("Set WANIKANI_API_TOKEN in the environment before running yomibu sync.")
                })?;
            let report = synchronize(&data_dir, Client::new(&token)?)?;
            write_status(&mut io::stdout().lock(), &report.summary)?;
        }
        Command::Status => {
            let summary = App::new(FileLearningStore::new(&data_dir)).status()?;
            write_status(&mut io::stdout().lock(), &summary)?;
        }
    }
    Ok(())
}

fn synchronize(data_dir: &Path, client: Client) -> anyhow::Result<SyncReport> {
    let mut app = App::new(FileLearningStore::new(data_dir)).with_source(client);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(runtime.block_on(app.sync())?)
}

fn write_status(out: &mut impl Write, summary: &Summary) -> io::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};
    use yomibu::cache;

    #[test]
    fn composes_sync_under_lock_and_renders_the_persisted_sync_data() {
        let setup = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let server = setup.block_on(MockServer::start());
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = dir.path().to_path_buf();
        setup.block_on(async {
            Mock::given(path("/v2/user")).respond_with(move |_: &wiremock::Request| {
                assert!(matches!(cache::SyncGuard::acquire(&lock_dir), Err(cache::WriteError::Locked)));
                ResponseTemplate::new(200).set_body_raw(include_str!("../tests/fixtures/wanikani/user.json"), "application/json")
            }).expect(1).mount(&server).await;
            for endpoint in ["assignments", "review_statistics"] {
                Mock::given(path(format!("/v2/{endpoint}"))).respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"object":"collection", "pages":{"next_url":null}, "data":[]})
                )).expect(1).mount(&server).await;
            }
        });
        let client =
            Client::with_base_url("synthetic-cli-credential", &format!("{}/v2/", server.uri()))
                .unwrap();
        let report = synchronize(dir.path(), client).unwrap();
        assert_eq!(
            report.summary,
            cache::load(dir.path()).unwrap().summarize().unwrap()
        );
        let mut output = Vec::new();
        write_status(&mut output, &report.summary).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("User: テスト (level 5)"));
        assert!(text.contains("Meaning accuracy: no reviews"));
        assert!(cache::SyncGuard::acquire(dir.path()).is_ok());
    }
}
