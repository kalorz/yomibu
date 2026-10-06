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
        grammar_file,
        openai::{Client as OpenAiClient, ProviderError},
        stores::FileLearningStore,
    },
    app::SyncReport,
    domain::LexicalContent,
    knowledge::{KnowledgeDecision, LearnerKnowledgePolicy, WaniKaniKnowledgeRule},
    ports::LearningStore,
    preparation::{PracticeTarget, PreparedContext, UnassessedAspect, prepare_context},
    preview::{CheckOutcome, Preview, WordEntry, preview},
    summary::{Accuracy, Summary},
    wanikani::Client,
};

mod analyze;
mod candidate_report;
mod cli_support;
mod dictionary;
mod focused;
mod generate;

#[cfg(test)]
mod focused_tests;
#[cfg(test)]
mod generate_tests;

#[derive(Parser)]
#[command(
    version,
    bin_name = "yomibu",
    about = "Generate experimental candidates, analyze supplied text, prepare practice context, preview entries, or sync/inspect WaniKani (unofficial tool)"
)]
struct Cli {
    /// Sync/status/prepare directory containing wanikani.json (default: $HOME/.yomibu; ignored by preview/analyze/generate-candidates/context-preview/generate-focused).
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
    /// Inspect focused selection and exact request content entirely offline.
    ContextPreview {
        /// Version-1 explicit vocabulary/grammar permissions (at most 4 MiB).
        #[arg(long, value_name = "PATH")]
        permissions: PathBuf,
        /// Positive one-based full-inventory entry number; tuples stay associated.
        #[arg(long, value_name = "N")]
        focus_entry: std::num::NonZeroUsize,
        #[arg(long)]
        json: bool,
    },
    /// Select focused context and request two experimental sentences in one attempt.
    GenerateFocused {
        #[arg(long, required = true)]
        allow_model_call: bool,
        #[arg(long, value_name = "PATH")]
        permissions: PathBuf,
        #[arg(long, value_name = "N")]
        focus_entry: std::num::NonZeroUsize,
        #[command(flatten)]
        dictionary: dictionary::DictionaryArgs,
        #[arg(long)]
        json: bool,
    },
    /// Request two experimental sentences from OpenAI; never accepted exercises.
    GenerateCandidates {
        /// Authorize one paid model attempt sending the supplied permissions/grammar.
        #[arg(long, required = true)]
        allow_model_call: bool,
        #[command(flatten)]
        dictionary: dictionary::DictionaryArgs,
        /// Version-1 JSON grammar declarations and explicit vocabulary/grammar bindings.
        #[arg(long, value_name = "PATH")]
        input: PathBuf,
        /// Emit both candidates, checks, errors, provenance and original UTF-8 spans.
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
    /// Retrieve cached vocabulary evidence for explicit practice targets, without writes.
    Prepare {
        /// Version-1 JSON file containing manual grammar familiarity declarations.
        #[arg(long, value_name = "PATH")]
        grammar_file: PathBuf,
        /// Revisable eligibility rule applied to preserved assignment timestamps.
        #[arg(long, value_enum, default_value = "lesson-started")]
        knowledge_policy: PolicyChoice,
        /// Exact cached word, accepted reading and gloss (repeatable).
        #[arg(long = "target", value_name = "WORD:READING:SENSE", required = true)]
        targets: Vec<String>,
    },
    /// Select supplied word entries; grammar and linguistic correctness are not assessed.
    Preview {
        /// Word entry; split at the first two colons, trim fields, retain meaning colons.
        #[arg(long = "word", value_name = "TEXT:READING:MEANING")]
        words: Vec<String>,
        /// Nonblank manual grammar description (repeatable; not assessed).
        #[arg(long, value_name = "DESCRIPTION")]
        grammar: Vec<String>,
        /// Number of entries to select in input order (positive, within input size).
        #[arg(long, value_name = "N")]
        take: usize,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum PolicyChoice {
    LessonStarted,
    RecordedPass,
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
        Command::ContextPreview {
            permissions,
            focus_entry,
            json,
        } => focused::run_preview(
            &permissions,
            focus_entry.get(),
            json,
            &mut io::stdout().lock(),
        )
        .map_err(|error| anyhow!("{}", format!("{error:#}").escape_debug()))?,
        Command::GenerateFocused {
            permissions,
            focus_entry,
            dictionary,
            json,
            ..
        } => focused::run_generation(
            &permissions,
            focus_entry.get(),
            &dictionary,
            json,
            make_client,
            &mut io::stdout().lock(),
        )
        .map_err(|error| anyhow!("{}", format!("{error:#}").escape_debug()))?,
        Command::GenerateCandidates {
            dictionary,
            input,
            json,
            ..
        } => generate::run(
            &dictionary,
            &input,
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
        Command::Prepare {
            grammar_file,
            knowledge_policy,
            targets,
        } => {
            let targets = targets
                .iter()
                .map(|target| parse_target(target))
                .collect::<Result<Vec<_>, _>>()?;
            let data_dir = resolve_data_dir(cli.data_dir)?;
            let source = FileLearningStore::new(&data_dir).load()?;
            let grammar = grammar_file::load(&grammar_file)?;
            let policy = LearnerKnowledgePolicy {
                wanikani: match knowledge_policy {
                    PolicyChoice::LessonStarted => WaniKaniKnowledgeRule::LessonStarted,
                    PolicyChoice::RecordedPass => WaniKaniKnowledgeRule::RecordedPass,
                },
            };
            let result = prepare_context(&source, &grammar, &policy, &targets)?;
            write_prepared(&mut io::stdout().lock(), &result)?;
        }
        Command::Preview {
            words,
            grammar,
            take,
        } => {
            let words = words
                .iter()
                .map(|word| parse_word(word))
                .collect::<Result<Vec<_>, _>>()?;
            let result = preview(&words, &grammar, take)?;
            write_preview(&mut io::stdout().lock(), &result)?;
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

fn parse_word(value: &str) -> anyhow::Result<WordEntry> {
    let mut fields = value.splitn(3, ':');
    match (fields.next(), fields.next(), fields.next()) {
        (Some(text), Some(reading), Some(meaning)) => Ok(WordEntry {
            text: text.trim().into(),
            reading: reading.trim().into(),
            meaning: meaning.trim().into(),
        }),
        _ => Err(anyhow!(
            "Expected --word TEXT:READING:MEANING with two ASCII colons."
        )),
    }
}

fn parse_target(value: &str) -> anyhow::Result<PracticeTarget> {
    let mut fields = value.splitn(3, ':');
    match (fields.next(), fields.next(), fields.next()) {
        (Some(word), Some(reading), Some(sense)) => Ok(PracticeTarget {
            word: word.trim().into(),
            intended_reading: reading.trim().into(),
            intended_sense: sense.trim().into(),
        }),
        _ => Err(anyhow!(
            "Expected --target WORD:READING:SENSE with two ASCII colons."
        )),
    }
}

fn write_prepared(out: &mut impl Write, result: &PreparedContext<'_>) -> io::Result<()> {
    writeln!(out, "Practice context (not a validated Japanese exercise)")?;
    let knowledge = &result.knowledge;
    writeln!(out, "Learner: {}", knowledge.learner_id.escape_debug())?;
    let policy = match knowledge.policy.wanikani {
        WaniKaniKnowledgeRule::LessonStarted => "lesson-started",
        WaniKaniKnowledgeRule::RecordedPass => "recorded-pass",
    };
    writeln!(out, "Policy: {policy}")?;
    writeln!(
        out,
        "Source sync interval: {} / {}",
        knowledge
            .sync_started_at
            .to_rfc3339_opts(SecondsFormat::AutoSi, true),
        knowledge
            .sync_completed_at
            .to_rfc3339_opts(SecondsFormat::AutoSi, true)
    )?;
    let eligible = knowledge
        .materials
        .iter()
        .filter(|entry| entry.decision == KnowledgeDecision::Eligible)
        .count();
    writeln!(
        out,
        "Eligible cached subjects: {eligible}; excluded: {}",
        knowledge.materials.len() - eligible
    )?;
    for entry in &knowledge.materials {
        write!(out, "  Subject {} ({:?}): ", entry.subject_id, entry.kind)?;
        match entry.decision {
            KnowledgeDecision::Eligible => writeln!(out, "eligible under {policy}")?,
            KnowledgeDecision::Excluded(reason) => writeln!(out, "excluded: {reason:?}")?,
        }
        match entry.material {
            Some(subject) => writeln!(
                out,
                "    Content: available; hidden_at: {:?}",
                subject.hidden_at
            )?,
            None => writeln!(out, "    Content: unavailable (access limit)")?,
        }
        match entry.assignment {
            Some(assignment) => writeln!(
                out,
                "    Assignment: {}; hidden: {}; started_at: {:?}; passed_at: {:?}",
                assignment.id, assignment.hidden, assignment.started_at, assignment.passed_at
            )?,
            None => writeln!(out, "    Assignment: none recorded")?,
        }
        match entry.review_statistic {
            Some(statistic) => writeln!(
                out,
                "    Review statistic: {}; hidden: {}",
                statistic.id, statistic.hidden
            )?,
            None => writeln!(out, "    Review statistic: none recorded")?,
        }
    }
    for selected in &result.targets {
        let target = selected.target;
        writeln!(
            out,
            "  Target: {}:{}:{}",
            target.word.escape_debug(),
            target.intended_reading.escape_debug(),
            target.intended_sense.escape_debug()
        )?;
        writeln!(
            out,
            "  Source subject: {}; assignment: {}",
            selected.subject.id, selected.assignment.id
        )?;
        writeln!(
            out,
            "  Recorded lesson start: {:?}; recorded pass: {:?}",
            selected.assignment.started_at, selected.assignment.passed_at
        )?;
        if let LexicalContent::Vocabulary {
            readings,
            parts_of_speech,
            ..
        } = &selected.subject.lexical
        {
            for reading in readings {
                writeln!(
                    out,
                    "  Reading: {} (primary: {}; accepted: {})",
                    reading.reading.escape_debug(),
                    reading.primary,
                    reading.accepted_answer
                )?;
            }
            for part in parts_of_speech {
                writeln!(out, "  Part of speech: {}", part.escape_debug())?;
            }
        }
        for meaning in &selected.subject.meanings {
            writeln!(
                out,
                "  Gloss: {} (primary: {}; accepted: {})",
                meaning.meaning.escape_debug(),
                meaning.primary,
                meaning.accepted_answer
            )?;
        }
        if selected.examples.is_empty() {
            writeln!(out, "  Examples: none recorded")?;
        }
        for example in selected.examples {
            writeln!(
                out,
                "  Example (source-attached): {} / {}",
                example.japanese.escape_debug(),
                example.english.escape_debug()
            )?;
        }
    }
    for declaration in knowledge.grammar {
        writeln!(
            out,
            "  Grammar {}: {}",
            declaration.id,
            declaration.description.escape_debug()
        )?;
    }
    for aspect in result.unassessed {
        let label = match aspect {
            UnassessedAspect::Grammar => "Grammar",
            UnassessedAspect::ReadingSenseAssociation => "Reading/sense association",
            UnassessedAspect::ExampleSuitability => "Example suitability",
            UnassessedAspect::LinguisticCorrectness => "Linguistic correctness",
        };
        writeln!(out, "{label}: not assessed")?;
    }
    Ok(())
}

fn write_preview(out: &mut impl Write, result: &Preview<'_>) -> io::Result<()> {
    writeln!(
        out,
        "Manual candidate preview (not a validated Japanese exercise)"
    )?;
    for word in result.selected {
        writeln!(
            out,
            "  Word: {}:{}:{}",
            word.text.escape_debug(),
            word.reading.escape_debug(),
            word.meaning.escape_debug()
        )?;
    }
    for description in result.grammar {
        writeln!(out, "  Grammar: {}", description.escape_debug())?;
    }
    for (label, outcome) in [
        ("Supplied-entry membership", result.checks.membership),
        ("Requested entry count", result.checks.count),
        ("Grammar", result.checks.grammar),
        (
            "Readings, meanings, naturalness",
            result.checks.linguistic_correctness,
        ),
    ] {
        let outcome = match outcome {
            CheckOutcome::Pass => "pass",
            CheckOutcome::Fail => "fail",
            CheckOutcome::NotAssessed => "not assessed",
        };
        writeln!(out, "{label}: {outcome}")?;
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
    fn word_syntax_trims_boundaries_and_preserves_internal_content_and_meaning_colons() {
        assert_eq!(
            parse_word(" \u{3000}猫 \t: ねこ : cat: a  feline : ").unwrap(),
            WordEntry {
                text: "猫".into(),
                reading: "ねこ".into(),
                meaning: "cat: a  feline :".into(),
            }
        );
    }

    #[test]
    fn word_syntax_requires_two_ascii_delimiters() {
        for value in ["", "猫", "猫:ねこ", "猫：ねこ：cat"] {
            let error = parse_word(value).unwrap_err();
            assert!(error.to_string().contains("TEXT:READING:MEANING"));
        }
    }

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
