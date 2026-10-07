//! Configure files, dictionary, credentials and I/O runtime around shared story generation.
use super::{
    input::{MAX_STORY_REQUEST_FILE_BYTES, load_inventory, read_json, read_key},
    retrieval,
};
use crate::{
    args::{DictionaryArgs, EmbeddingArgs, StoryArgs},
    output,
};
use anyhow::{Context, Result, bail};
use std::io::Write;
use yomibu::{
    adapters::{
        embedding_cache_file::EmbeddingCacheFile,
        openai::{Client, ProviderError},
    },
    inventory::LearnerInventory,
    story::{
        StoryGenerationOptions, StoryGenerationPlan, StoryGenerationResult, StoryRequest,
        generate_story, plan_generation,
    },
};

pub(crate) fn generate(
    args: &StoryArgs,
    embedding: &EmbeddingArgs,
    options: StoryGenerationOptions,
    dictionary: &DictionaryArgs,
    json: bool,
    make_client: impl FnOnce(&str) -> Result<Client, ProviderError>,
    out: &mut impl Write,
) -> Result<()> {
    let (inventory, request) = load_generation_inputs(args, options)?;
    let cache = retrieval::load_or_prepare_embeddings(args, embedding, &inventory, &request)?;
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

    output::story::write_generation(out, &request, &plan, &result, json)?;
    require_successful_execution(&result)
}

fn load_generation_inputs(
    args: &StoryArgs,
    options: StoryGenerationOptions,
) -> Result<(LearnerInventory, StoryRequest)> {
    options.validate()?;
    let inventory = load_inventory(args)?;
    let request: StoryRequest =
        read_json(&args.request, "Story request", MAX_STORY_REQUEST_FILE_BYTES)?;
    request.validate_selection_limit(usize::from(args.select))?;
    request.validate(&inventory)?;
    Ok((inventory, request))
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
pub(crate) fn preview(
    args: &StoryArgs,
    options: StoryGenerationOptions,
    json: bool,
    out: &mut impl Write,
) -> Result<()> {
    options.validate()?;
    let inventory = load_inventory(args)?;
    let request: StoryRequest =
        read_json(&args.request, "Story request", MAX_STORY_REQUEST_FILE_BYTES)?;
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
    output::story::write_preview(out, &request, &plan, json)
}
