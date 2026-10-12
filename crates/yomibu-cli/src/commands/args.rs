use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use yomibu::configuration::{KnowledgePolicy, ProcessOverrides, StoryFormat, modules::ModuleId};

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
    /// Use only explicit credentials; do not read macOS Keychain.
    #[arg(long, global = true, help_heading = "Credentials")]
    pub no_keychain: bool,
    /// Print module states, progress, and timings.
    #[arg(long, global = true)]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Save credentials in macOS Keychain; without a target, prompt for missing configured credentials.
    Auth {
        /// Component namespace for missing credentials, or exact credential to set or replace. Enter skips.
        #[arg(value_name = "CREDENTIAL", value_parser = clap::builder::PossibleValuesParser::new(super::auth::targets()))]
        credential: Option<String>,
    },
    /// Generate a passage of 3–5 short sentences with one AI request.
    Story(StoryArgs),
    /// Explicitly refresh the complete WaniKani cache.
    Sync,
}

#[derive(Args, Default)]
pub(crate) struct StoryArgs {
    /// Optional scene or subject. Otherwise create a scene around familiar words.
    #[arg(long, conflicts_with = "request")]
    topic: Option<String>,
    #[command(flatten)]
    options: super::options::ComponentArgs,
    /// Advanced story request JSON with optional topic and explicit targets.
    #[arg(long, value_name = "PATH")]
    request: Option<PathBuf>,
    /// Shared text-model fallback; env: YOMIBU_MODEL.
    #[arg(long)]
    model: Option<String>,
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
        let pipeline = &mut input.invocation.pipeline;
        pipeline.model = self.model;
        pipeline.options = self.options.0;
        pipeline.knowledge_policy = self.knowledge_policy;
        let story = &mut input.invocation.story;
        story.topic = self.topic.into();
        story.format = self.format;
        story.candidates = self.candidates.map(std::num::NonZeroUsize::get);
        story.select = self.select.map(usize::from);
        story.seed = self.seed.into();
        input
    }
}
