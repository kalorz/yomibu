use anyhow::anyhow;
use chrono::SecondsFormat;
use clap::{
    Parser, Subcommand,
    error::{ContextKind, ContextValue},
};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};
use yomibu::{
    App,
    adapters::{
        openai::{Client as OpenAiClient, ProviderError},
        stores::FileLearningStore,
    },
    app::SyncReport,
    story::StoryGenerationOptions,
    summary::{Accuracy, Summary},
    wanikani::Client,
};

mod analyze;
mod candidate_report;
mod cli_support;
mod dictionary;
mod story_command;

#[cfg(test)]
mod story_tests;

#[derive(Parser)]
#[command(
    name = "yomibu",
    version,
    bin_name = "yomibu",
    about = "Generate experimental stories, preview requests, prepare retrieval, analyze text, or sync/inspect WaniKani (unofficial tool)"
)]
struct Cli {
    /// Sync/status directory containing wanikani.json (default: $HOME/.yomibu; ignored by analyze/story commands).
    #[arg(long, global = true, value_name = "PATH")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import or fully verify an app-managed dictionary; no implicit downloads.
    Dictionary {
        #[command(subcommand)]
        command: dictionary::DictionaryCommand,
    },
    /// Preview a topic-based story generation plan offline using cached embeddings.
    PreviewStory {
        #[command(flatten)]
        story: story_command::StoryArgs,
        /// Number of candidates to request in one provider call.
        #[arg(long, default_value = "2")]
        candidates: std::num::NonZeroUsize,
        #[arg(long)]
        json: bool,
    },
    /// Explicitly prepare/cache lexical and brief embeddings; no generation.
    PrepareRetrieval {
        #[command(flatten)]
        story: story_command::StoryArgs,
        #[command(flatten)]
        embedding: story_command::EmbeddingArgs,
    },
    /// Generate experimental sentences from either learner inventory source.
    GenerateStory {
        #[arg(long, required = true)]
        allow_model_call: bool,
        #[command(flatten)]
        story: story_command::StoryArgs,
        #[command(flatten)]
        embedding: story_command::EmbeddingArgs,
        #[command(flatten)]
        dictionary: dictionary::DictionaryArgs,
        /// Number of candidates to request in one provider call.
        #[arg(long, default_value = "2")]
        candidates: std::num::NonZeroUsize,
        #[arg(long)]
        json: bool,
    },
    /// Run bounded offline checks on one manually supplied sentence.
    Analyze {
        #[command(flatten)]
        dictionary: dictionary::DictionaryArgs,
        /// Version-1 JSON sentence, grammar declarations, and explicit bindings.
        #[arg(long, value_name = "PATH")]
        input: PathBuf,
        /// Emit structured analysis, original UTF-8 spans, and evaluation results.
        #[arg(long)]
        json: bool,
    },
    /// Refresh the complete cache using WANIKANI_API_TOKEN.
    Sync,
    /// Show cached observations without accessing the network.
    Status,
}

fn main() -> ExitCode {
    ExitCode::from(entry(std::env::args_os(), OpenAiClient::new))
}

fn entry(
    args: impl IntoIterator<Item = impl Into<std::ffi::OsString> + Clone>,
    make_client: impl FnOnce(&str) -> Result<OpenAiClient, ProviderError>,
) -> u8 {
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            if error.use_stderr() {
                let _ = write!(io::stderr().lock(), "{}", escape_argument_error(error));
                return 2;
            }
            return if error.print().is_ok() { 0 } else { 1 };
        }
    };
    match run(cli, make_client) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "error: {error}");
            1
        }
    }
}

fn escape_argument_error(mut error: clap::Error) -> clap::Error {
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

fn run(
    cli: Cli,
    make_client: impl FnOnce(&str) -> Result<OpenAiClient, ProviderError>,
) -> anyhow::Result<()> {
    match cli.command {
        Command::Dictionary { command } => dictionary::run(command, &mut io::stdout().lock())
            .map_err(|error| anyhow!("{}", format!("{error:#}").escape_debug()))?,
        Command::PreviewStory {
            story,
            candidates,
            json,
        } => story_command::run_preview(
            &story,
            StoryGenerationOptions {
                candidate_count: candidates.get(),
            },
            json,
            &mut io::stdout().lock(),
        )
        .map_err(|error| anyhow!("{}", format!("{error:#}").escape_debug()))?,
        Command::PrepareRetrieval { story, embedding } => {
            story_command::run_prepare(&story, &embedding, &mut io::stdout().lock())
                .map_err(|error| anyhow!("{}", format!("{error:#}").escape_debug()))?
        }
        Command::GenerateStory {
            story,
            embedding,
            dictionary,
            candidates,
            json,
            ..
        } => story_command::generate_story_command(
            &story,
            &embedding,
            StoryGenerationOptions {
                candidate_count: candidates.get(),
            },
            &dictionary,
            json,
            make_client,
            &mut io::stdout().lock(),
        )
        .map_err(|error| anyhow!("{}", format!("{error:#}").escape_debug()))?,
        Command::Analyze {
            dictionary,
            input,
            json,
        } => analyze::run(&dictionary, &input, json, &mut io::stdout().lock())
            .map_err(|error| anyhow!("{}", format!("{error:#}").escape_debug()))?,
        Command::Sync => {
            let data_dir = resolve_data_dir(cli.data_dir)?;
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
            let data_dir = resolve_data_dir(cli.data_dir)?;
            let summary = App::new(FileLearningStore::new(&data_dir)).status()?;
            write_status(&mut io::stdout().lock(), &summary)?;
        }
    }
    Ok(())
}

fn resolve_data_dir(data_dir: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    data_dir
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(|home| PathBuf::from(home).join(".yomibu"))
        })
        .ok_or_else(|| anyhow!("HOME is unavailable; specify --data-dir PATH."))
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
                ResponseTemplate::new(200).set_body_raw(include_str!("../../../tests/fixtures/wanikani/user.json"), "application/json")
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
