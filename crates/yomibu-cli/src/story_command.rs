//! Complete story command sequences; source-specific work ends at load_inventory.
use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use serde::de::DeserializeOwned;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use yomibu::{
    adapters::{
        embedding_cache_file::EmbeddingCacheFile,
        embeddings::{HttpEmbedder, LexicalEmbedder},
        openai::{Client, ProviderError},
    },
    inventory::{LearnerInventory, ManualInventory},
    knowledge::{LearnerKnowledgePolicy, WaniKaniKnowledgeRule},
    ports::Embedder,
    retrieval::{
        EmbeddingCache, EmbeddingInput, EmbeddingModelIdentity, prepare_cache,
        prepare_embedding_inputs,
    },
    story::{
        StoryGenerationOptions, StoryGenerationPlan, StoryGenerationResult, StoryRequest,
        generate_story, plan_generation,
    },
};
mod report;

#[derive(Args)]
pub(super) struct StoryArgs {
    /// Manual inventory; may supplement an explicitly selected WaniKani cache.
    #[arg(long, value_name = "PATH", required_unless_present = "wanikani_cache")]
    inventory: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    wanikani_cache: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "lesson-started")]
    knowledge_policy: KnowledgePolicy,
    #[arg(long, value_name = "PATH")]
    request: PathBuf,
    #[arg(long, value_name = "PATH")]
    embedding_cache: PathBuf,
    /// Maximum selected vocabulary, including every explicit target (1..=16).
    #[arg(long,default_value_t=12,value_parser=clap::value_parser!(u8).range(1..=16))]
    select: u8,
}
#[derive(Clone, Copy, ValueEnum)]
enum KnowledgePolicy {
    LessonStarted,
    RecordedPass,
}
#[derive(Clone, Copy, ValueEnum)]
pub(super) enum EmbeddingProvider {
    LexicalBaseline,
    Local,
    Openai,
}
#[derive(Args)]
pub(super) struct EmbeddingArgs {
    /// Explicit encoder; lexical-baseline is a comparison baseline, not semantic AI.
    #[arg(long, value_enum)]
    embedding_provider: Option<EmbeddingProvider>,
    #[arg(long)]
    embedding_model: Option<String>,
    #[arg(long)]
    embedding_revision: Option<String>,
    #[arg(long,value_parser=clap::value_parser!(u16).range(1..=4096))]
    embedding_dimensions: Option<u16>,
    /// Numeric loopback OpenAI-compatible endpoint, only for the local encoder.
    #[arg(long, default_value = "http://127.0.0.1:11434/v1/")]
    embedding_endpoint: String,
    /// Permit hosted embedding requests sending lexical text and the story brief.
    #[arg(long)]
    allow_embedding_call: bool,
}

pub(super) fn generate_story_command(
    args: &StoryArgs,
    embedding: &EmbeddingArgs,
    options: StoryGenerationOptions,
    dictionary: &super::dictionary::DictionaryArgs,
    json: bool,
    make_client: impl FnOnce(&str) -> Result<Client, ProviderError>,
    out: &mut impl Write,
) -> Result<()> {
    let (inventory, request) = load_generation_inputs(args, options)?;
    let cache = load_or_prepare_embeddings(args, embedding, &inventory, &request)?;
    let plan = plan_generation(
        &inventory,
        &request,
        &cache,
        &cache.model,
        usize::from(args.select),
        options,
    )?;
    let analyzer = dictionary.load().context("Dictionary initialization")?;
    let result = execute_story_plan(&plan, &analyzer, make_client)?;

    report::write_generation(out, &request, &plan, &result, json)?;
    require_successful_execution(&result)
}

fn load_generation_inputs(
    args: &StoryArgs,
    options: StoryGenerationOptions,
) -> Result<(LearnerInventory, StoryRequest)> {
    options.validate()?;
    let inventory = load_inventory(args)?;
    let request: StoryRequest = read_json(&args.request, "Story request", 65536)?;
    request.validate_selection_limit(usize::from(args.select))?;
    request.validate(&inventory)?;
    Ok((inventory, request))
}

fn load_or_prepare_embeddings(
    args: &StoryArgs,
    embedding: &EmbeddingArgs,
    inventory: &LearnerInventory,
    request: &StoryRequest,
) -> Result<EmbeddingCache> {
    let inputs = prepare_embedding_inputs(inventory, request)?;
    let mut cache = EmbeddingCacheFile::new(&args.embedding_cache).load()?;
    if !cache_matches(cache.as_ref(), embedding, &inputs)? {
        cache = Some(prepare_embeddings(embedding, &inputs, cache.as_ref())?);
        EmbeddingCacheFile::new(&args.embedding_cache)
            .save(cache.as_ref().context("Missing embedding result")?)?;
    }
    cache.context("Embedding cache missing; run prepare-retrieval explicitly.")
}

fn execute_story_plan(
    plan: &StoryGenerationPlan<'_>,
    analyzer: &yomibu::adapters::sudachi::SudachiAnalyzer,
    make_client: impl FnOnce(&str) -> Result<Client, ProviderError>,
) -> Result<StoryGenerationResult> {
    let key = read_key("OPENAI_API_KEY")?;
    let client = make_client(&key)?;
    drop(key);
    let io_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Generation runtime")?;
    Ok(io_runtime.block_on(generate_story(plan, &client, analyzer))?)
}

fn require_successful_execution(result: &StoryGenerationResult) -> Result<()> {
    if result.has_execution_errors() {
        bail!("One or more candidate executions failed; see the experimental report.");
    }
    Ok(())
}
pub(super) fn run_preview(
    args: &StoryArgs,
    options: StoryGenerationOptions,
    json: bool,
    out: &mut impl Write,
) -> Result<()> {
    options.validate()?;
    let inventory = load_inventory(args)?;
    let request: StoryRequest = read_json(&args.request, "Story request", 65536)?;
    request.validate(&inventory)?;
    let cache = EmbeddingCacheFile::new(&args.embedding_cache)
        .load()?
        .context("Embedding cache missing; run prepare-retrieval explicitly.")?;
    let plan = plan_generation(
        &inventory,
        &request,
        &cache,
        &cache.model,
        usize::from(args.select),
        options,
    )?;
    report::write_preview(out, &request, &plan, json)
}

pub(super) fn run_prepare(
    args: &StoryArgs,
    embedding: &EmbeddingArgs,
    out: &mut impl Write,
) -> Result<()> {
    let inventory = load_inventory(args)?;
    let request: StoryRequest = read_json(&args.request, "Story request", 65536)?;
    request.validate_selection_limit(usize::from(args.select))?;
    let inputs = prepare_embedding_inputs(&inventory, &request)?;
    let previous = EmbeddingCacheFile::new(&args.embedding_cache).load()?;
    let cache = prepare_embeddings(embedding, &inputs, previous.as_ref())?;
    EmbeddingCacheFile::new(&args.embedding_cache).save(&cache)?;
    writeln!(
        out,
        "Retrieval prepared: {} vocabulary entries; model {} / {}. No generation performed.",
        inventory.vocabulary.len(),
        cache.model.provider.escape_debug(),
        cache.model.model.escape_debug()
    )?;
    Ok(())
}
fn load_inventory(args: &StoryArgs) -> Result<LearnerInventory> {
    let manual = args
        .inventory
        .as_ref()
        .map(|path| read_json::<ManualInventory>(path, "Manual inventory", 4194304))
        .transpose()?;
    let inventory = if let Some(path) = &args.wanikani_cache {
        #[derive(serde::Deserialize)]
        struct Envelope {
            schema_version: u32,
            snapshot: yomibu::domain::WaniKaniSyncData,
        }
        let source: Envelope = read_json(path, "WaniKani cache", 67108864)?;
        if source.schema_version != 1 {
            bail!("Unsupported WaniKani cache version.");
        }
        let policy = LearnerKnowledgePolicy {
            wanikani: match args.knowledge_policy {
                KnowledgePolicy::LessonStarted => WaniKaniKnowledgeRule::LessonStarted,
                KnowledgePolicy::RecordedPass => WaniKaniKnowledgeRule::RecordedPass,
            },
        };
        let inventory = LearnerInventory::from_wanikani(&source.snapshot, &policy)?;
        match manual {
            Some(manual) => inventory.with_manual(manual)?,
            None => inventory,
        }
    } else {
        LearnerInventory::from_manual(manual.context("Supply --inventory or --wanikani-cache.")?)?
    };
    Ok(inventory)
}
fn read_json<T: DeserializeOwned>(path: &Path, label: &str, limit: usize) -> Result<T> {
    let bytes =
        super::cli_support::read_input_bounded(path, label, limit, &format!("{limit} bytes"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("Invalid {label} JSON"))
}
fn read_key(name: &str) -> Result<String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .with_context(|| {
            format!("Set {name} in the environment before explicitly requesting provider work.")
        })
}
fn model_identity(args: &EmbeddingArgs) -> Result<EmbeddingModelIdentity> {
    let provider=args.embedding_provider.context("Select --embedding-provider to prepare missing embeddings; no automatic model or fallback is selected.")?;
    if matches!(provider, EmbeddingProvider::LexicalBaseline) {
        return Ok(LexicalEmbedder::new().model_identity().clone());
    }
    Ok(EmbeddingModelIdentity {
        provider: match provider {
            EmbeddingProvider::Local => "local",
            _ => "openai",
        }
        .into(),
        model: args
            .embedding_model
            .clone()
            .context("Supply --embedding-model.")?,
        revision: args.embedding_revision.clone().context(
            "Supply --embedding-revision (pinned local revision or dated hosted model identity).",
        )?,
        dimensions: usize::from(
            args.embedding_dimensions
                .context("Supply --embedding-dimensions.")?,
        ),
        encoding_revision: "plain-v1".into(),
    })
}
fn cache_matches(
    cache: Option<&EmbeddingCache>,
    args: &EmbeddingArgs,
    inputs: &[EmbeddingInput],
) -> Result<bool> {
    let Some(cache) = cache else { return Ok(false) };
    let requested = if args.embedding_provider.is_some() {
        model_identity(args)?
    } else {
        cache.model.clone()
    };
    Ok(cache.vectors(&requested, inputs).is_ok())
}
fn prepare_embeddings(
    args: &EmbeddingArgs,
    inputs: &[EmbeddingInput],
    previous: Option<&EmbeddingCache>,
) -> Result<EmbeddingCache> {
    let identity = model_identity(args)?;
    if matches!(args.embedding_provider, Some(EmbeddingProvider::Openai))
        && !args.allow_embedding_call
    {
        bail!("Hosted embeddings require --allow-embedding-call.");
    }
    let io_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Embedding runtime")?;
    let result = match args.embedding_provider {
        Some(EmbeddingProvider::LexicalBaseline) => {
            io_runtime.block_on(prepare_cache(&LexicalEmbedder::new(), inputs, previous))
        }
        Some(EmbeddingProvider::Local) => io_runtime.block_on(prepare_cache(
            &HttpEmbedder::local(&args.embedding_endpoint, identity)?,
            inputs,
            previous,
        )),
        Some(EmbeddingProvider::Openai) => {
            let key = read_key("OPENAI_API_KEY")?;
            let embedder = HttpEmbedder::openai(&key, identity)?;
            drop(key);
            io_runtime.block_on(prepare_cache(&embedder, inputs, previous))
        }
        None => bail!("Select an embedding provider."),
    };
    Ok(result?)
}
