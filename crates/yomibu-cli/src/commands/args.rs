use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use yomibu::configuration::{
    EmbeddingProvider, KnowledgePolicy, ProcessOverrides, StoryFormat, components,
    modules::ModuleId,
};

#[derive(Parser)]
#[command(
    name = "yomibu",
    bin_name = "yomibu",
    version,
    about = "Create experimental Japanese readings from familiar vocabulary"
)]
pub(crate) struct Cli {
    /// Local storage (default: $HOME/.yomibu).
    #[arg(long, global = true, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
    /// Configuration file (default: <data-dir>/config.toml).
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,
    /// Emit a single structured JSON report.
    #[arg(long, global = true)]
    pub json: bool,
    /// Print module states, progress, and timings (stderr with --json).
    #[arg(long, global = true)]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Generate a passage of 3–5 short sentences with one AI request.
    Story(StoryArgs),
    /// Show an exact generation request offline without calling an AI model.
    PreviewStory(StoryArgs),
    /// Explicitly prepare and cache vocabulary/topic embeddings.
    PrepareRetrieval(StoryArgs),
    /// Run bounded offline checks on one supplied sentence.
    Analyze {
        #[command(flatten)]
        dictionary: DictionaryArgs,
        #[arg(long, value_name = "PATH")]
        input: PathBuf,
    },
    /// Import or fully verify a pinned dictionary offline.
    Dictionary {
        #[command(subcommand)]
        command: DictionaryCommand,
    },
    /// Explicitly refresh the complete WaniKani cache.
    Sync,
    /// Show cached source observations without network access.
    Status,
}

#[derive(Args, Default)]
pub(crate) struct StoryArgs {
    /// Optional scene or subject. Otherwise create a scene around familiar words.
    #[arg(long, conflicts_with = "request")]
    topic: Option<String>,
    /// Advanced story request JSON with optional topic and explicit targets.
    #[arg(long, value_name = "PATH")]
    request: Option<PathBuf>,
    /// Shared text-model fallback; env: YOMIBU_MODEL.
    #[arg(long)]
    model: Option<String>,
    /// Override the model for generation.
    #[arg(long = components::GENERATION_MODEL.name.cli(), value_parser = |value: &str| components::GENERATION_MODEL.parse(value))]
    generation_model: Option<String>,
    /// Output format: passage (default) or sentence.
    #[arg(long)]
    format: Option<StoryFormat>,
    /// Number of alternatives in the single generation request (default: 1).
    #[arg(long)]
    candidates: Option<std::num::NonZeroUsize>,
    /// Enable an optional module; repeat for multiple modules.
    #[arg(long, value_name = "MODULE")]
    enable: Vec<ModuleId>,
    /// Disable an optional module; repeat for multiple modules.
    #[arg(long, value_name = "MODULE")]
    disable: Vec<ModuleId>,
    /// Manual inventory, optionally supplementing an explicit WaniKani cache.
    #[arg(long, value_name = "PATH")]
    inventory: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    wanikani_cache: Option<PathBuf>,
    /// WaniKani eligibility: lesson-started (default) or recorded-pass.
    #[arg(long)]
    knowledge_policy: Option<KnowledgePolicy>,
    /// Selected words including targets (1..=16; default: 12).
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=16))]
    select: Option<u8>,
    /// Repeatable vocabulary sampling seed.
    #[arg(long)]
    seed: Option<u64>,
    #[command(flatten)]
    dictionary: DictionaryArgs,
    #[arg(long, value_name = "PATH")]
    embedding_cache: Option<PathBuf>,
    /// Encoder: lexical-baseline, local, or openai. Does not enable embeddings.
    #[arg(long)]
    embedding_provider: Option<EmbeddingProvider>,
    #[arg(long = components::EMBEDDING_MODEL.name.cli(), value_parser = |value: &str| components::EMBEDDING_MODEL.parse(value))]
    embedding_model: Option<String>,
    #[arg(long = components::EMBEDDING_REVISION.name.cli(), value_parser = |value: &str| components::EMBEDDING_REVISION.parse(value))]
    embedding_revision: Option<String>,
    #[arg(long = components::EMBEDDING_DIMENSIONS.name.cli(), value_parser = embedding_dimensions)]
    embedding_dimensions: Option<usize>,
    /// Numeric loopback OpenAI-compatible endpoint for a local encoder.
    #[arg(long = components::EMBEDDING_ENDPOINT.name.cli(), value_parser = |value: &str| components::EMBEDDING_ENDPOINT.parse(value))]
    embedding_endpoint: Option<String>,
    /// Authorize hosted embedding calls sending vocabulary and topic text.
    #[arg(long)]
    allow_embedding_call: bool,
}

fn embedding_dimensions(value: &str) -> Result<usize, &'static str> {
    let dimensions = components::EMBEDDING_DIMENSIONS.parse(value)?;
    if (1..=4096).contains(&dimensions) {
        Ok(dimensions)
    } else {
        Err("Embedding dimensions must be between 1 and 4096.")
    }
}

impl StoryArgs {
    pub fn overrides(self) -> ProcessOverrides {
        let mut input = ProcessOverrides {
            request: self.request,
            enable: self.enable,
            disable: self.disable,
            ..Default::default()
        };
        let app = &mut input.application;
        app.inventory = self.inventory;
        app.wanikani_cache = self.wanikani_cache;
        app.dictionary_dir = self.dictionary.dictionary_dir;
        app.embedding_cache = self.embedding_cache;
        app.allow_embedding_call = self.allow_embedding_call.then_some(true);
        let pipeline = &mut input.invocation.pipeline;
        pipeline.model = self.model;
        pipeline.options.generation.model = self.generation_model.into();
        pipeline.knowledge_policy = self.knowledge_policy;
        pipeline.embedding.provider = self.embedding_provider.into();
        pipeline.options.embeddings.model = self.embedding_model.into();
        pipeline.options.embeddings.revision = self.embedding_revision.into();
        pipeline.options.embeddings.dimensions = self.embedding_dimensions.into();
        pipeline.options.embeddings.endpoint = self.embedding_endpoint.into();
        let story = &mut input.invocation.story;
        story.topic = self.topic.into();
        story.format = self.format;
        story.candidates = self.candidates.map(std::num::NonZeroUsize::get);
        story.select = self.select.map(usize::from);
        story.seed = self.seed.into();
        input
    }
}
#[derive(Args, Default)]
pub(crate) struct DictionaryArgs {
    /// Managed dictionary root (default: <data-dir>/dictionaries); files must remain unchanged.
    #[arg(long, value_name = "PATH")]
    pub dictionary_dir: Option<PathBuf>,
}
impl DictionaryArgs {
    pub fn overrides(self) -> ProcessOverrides {
        let mut input = ProcessOverrides::default();
        input.application.dictionary_dir = self.dictionary_dir;
        input
    }
}
#[derive(Subcommand)]
pub(crate) enum DictionaryCommand {
    /// Copy and fully verify a bundle and publisher notices offline.
    Import {
        #[arg(long, value_name = "PATH")]
        bundle: PathBuf,
        #[arg(long, value_name = "PATH")]
        dictionary_dir: Option<PathBuf>,
    },
    /// Fully verify the installed dictionary and publisher notices offline.
    Verify {
        #[arg(long, value_name = "PATH")]
        dictionary_dir: Option<PathBuf>,
    },
}
