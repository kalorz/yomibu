//! Command-line inputs only; execution lives in commands.
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "yomibu",
    version,
    bin_name = "yomibu",
    about = "Generate experimental stories, preview requests, prepare retrieval, analyze text, or sync/inspect WaniKani (unofficial tool)"
)]
pub(crate) struct Cli {
    /// Sync/status directory containing wanikani.json (default: $HOME/.yomibu; ignored by analyze/story commands).
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) data_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Import or fully verify an app-managed dictionary; no implicit downloads.
    Dictionary {
        #[command(subcommand)]
        command: DictionaryCommand,
    },
    /// Preview a topic-based story generation plan offline using cached embeddings.
    PreviewStory {
        #[command(flatten)]
        story: StoryArgs,
        /// Number of candidates to request in one provider call.
        #[arg(long, default_value = "2")]
        candidates: std::num::NonZeroUsize,
        #[arg(long)]
        json: bool,
    },
    /// Explicitly prepare/cache lexical and brief embeddings; no generation.
    PrepareRetrieval {
        #[command(flatten)]
        story: StoryArgs,
        #[command(flatten)]
        embedding: EmbeddingArgs,
    },
    /// Generate experimental sentences from either learner inventory source.
    GenerateStory {
        #[arg(long, required = true)]
        allow_model_call: bool,
        #[command(flatten)]
        story: StoryArgs,
        #[command(flatten)]
        embedding: EmbeddingArgs,
        #[command(flatten)]
        dictionary: DictionaryArgs,
        /// Number of candidates to request in one provider call.
        #[arg(long, default_value = "2")]
        candidates: std::num::NonZeroUsize,
        #[arg(long)]
        json: bool,
    },
    /// Run bounded offline checks on one manually supplied sentence.
    Analyze {
        #[command(flatten)]
        dictionary: DictionaryArgs,
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

#[derive(Args)]
pub(crate) struct StoryArgs {
    /// Manual inventory; may supplement an explicitly selected WaniKani cache.
    #[arg(long, value_name = "PATH", required_unless_present = "wanikani_cache")]
    pub(crate) inventory: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    pub(crate) wanikani_cache: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "lesson-started")]
    pub(crate) knowledge_policy: KnowledgePolicy,
    #[arg(long, value_name = "PATH")]
    pub(crate) request: PathBuf,
    #[arg(long, value_name = "PATH")]
    pub(crate) embedding_cache: PathBuf,
    /// Maximum selected vocabulary, including every explicit target (1..=16).
    #[arg(long,default_value_t=12,value_parser=clap::value_parser!(u8).range(1..=16))]
    pub(crate) select: u8,
}
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum KnowledgePolicy {
    LessonStarted,
    RecordedPass,
}
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum EmbeddingProvider {
    LexicalBaseline,
    Local,
    Openai,
}
#[derive(Args)]
pub(crate) struct EmbeddingArgs {
    /// Explicit encoder; lexical-baseline is a comparison baseline, not semantic AI.
    #[arg(long, value_enum)]
    pub(crate) embedding_provider: Option<EmbeddingProvider>,
    #[arg(long)]
    pub(crate) embedding_model: Option<String>,
    #[arg(long)]
    pub(crate) embedding_revision: Option<String>,
    #[arg(long,value_parser=clap::value_parser!(u16).range(1..=4096))]
    pub(crate) embedding_dimensions: Option<u16>,
    /// Numeric loopback OpenAI-compatible endpoint, only for the local encoder.
    #[arg(long, default_value = "http://127.0.0.1:11434/v1/")]
    pub(crate) embedding_endpoint: String,
    /// Permit hosted embedding requests sending lexical text and the story brief.
    #[arg(long)]
    pub(crate) allow_embedding_call: bool,
}

#[derive(Args)]
pub(crate) struct DictionaryArgs {
    /// External pinned dictionary: full SHA-256 verification and owned bytes.
    #[arg(long, value_name = "PATH", conflicts_with = "dictionary_dir")]
    pub(crate) dictionary: Option<PathBuf>,
    /// Managed installation (default: $HOME/.yomibu/dictionaries); published files must remain unchanged.
    #[arg(long, value_name = "PATH")]
    pub(crate) dictionary_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
pub(crate) enum DictionaryCommand {
    /// Copy and fully verify a dictionary and its publisher notices entirely offline.
    Import {
        /// Directory containing system_core.dic, LEGAL and LICENSE-2.0.txt.
        #[arg(long, value_name = "PATH")]
        bundle: PathBuf,
        /// Managed installation root (default: $HOME/.yomibu/dictionaries).
        #[arg(long, value_name = "PATH")]
        dictionary_dir: Option<PathBuf>,
    },
    /// Fully verify the current dictionary and both notices without changing files.
    Verify {
        #[arg(long, value_name = "PATH")]
        dictionary_dir: Option<PathBuf>,
    },
}
