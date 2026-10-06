//! Select/configure an encoder and invoke shared retrieval preparation.
use super::input::{load_inventory, read_json, read_key};
use crate::args::{EmbeddingArgs, EmbeddingProvider, StoryArgs};
use anyhow::{Context, Result, bail};
use std::io::Write;
use yomibu::{
    adapters::{
        embedding_cache_file::EmbeddingCacheFile,
        embeddings::{HttpEmbedder, LexicalEmbedder},
    },
    inventory::LearnerInventory,
    ports::Embedder,
    retrieval::{
        EmbeddingCache, EmbeddingInput, EmbeddingModelIdentity, prepare_cache,
        prepare_embedding_inputs,
    },
    story::StoryRequest,
};

pub(crate) fn prepare(
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
pub(super) fn load_or_prepare_embeddings(
    args: &StoryArgs,
    embedding: &EmbeddingArgs,
    inventory: &LearnerInventory,
    request: &StoryRequest,
) -> Result<EmbeddingCache> {
    let inputs = prepare_embedding_inputs(inventory, request)?;
    let file = EmbeddingCacheFile::new(&args.embedding_cache);
    let previous = file.load()?;
    let cache = match previous {
        Some(cache) if cache_matches(&cache, embedding, &inputs)? => return Ok(cache),
        previous => prepare_embeddings(embedding, &inputs, previous.as_ref())?,
    };
    file.save(&cache)?;
    Ok(cache)
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
    cache: &EmbeddingCache,
    args: &EmbeddingArgs,
    inputs: &[EmbeddingInput],
) -> Result<bool> {
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
