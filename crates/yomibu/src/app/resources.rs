use super::{
    config::{Configuration, EmbeddingProvider},
    local::ApplicationError,
};
use crate::{
    adapters::{
        dictionary::ManagedInstallation,
        embedding_cache_file::EmbeddingCacheFile,
        embeddings::{HttpEmbedder, LexicalEmbedder},
        sudachi::SudachiAnalyzer,
    },
    inventory::LearnerInventory,
    ports::Embedder,
    retrieval::{EmbeddingCache, EmbeddingModelIdentity, prepare_cache, prepare_embedding_inputs},
    story::StoryRequest,
};

pub(super) unsafe fn load_analyzer(
    config: &Configuration,
) -> Result<SudachiAnalyzer, ApplicationError> {
    if let Some(path) = &config.dictionary {
        return Ok(SudachiAnalyzer::load(path)?);
    }
    let installation = ManagedInstallation::open(&config.dictionary_dir)?;
    // App-managed selection uses the importer's immutable generations. External
    // dictionary paths use verified owned bytes; managed files must never change
    // while mapped. Receipts and permissions cannot establish that guarantee.
    Ok(unsafe { SudachiAnalyzer::load_managed(installation) }?)
}

pub(super) async fn prepare_embeddings(
    config: &Configuration,
    key: Option<&str>,
    inventory: &LearnerInventory,
    request: &StoryRequest,
) -> Result<EmbeddingCache, ApplicationError> {
    let inputs = prepare_embedding_inputs(inventory, request)?;
    let file = EmbeddingCacheFile::new(&config.embedding_cache);
    let previous = file.load()?;
    let identity = match config.embedding_provider {
        Some(EmbeddingProvider::LexicalBaseline) => LexicalEmbedder::new().model_identity().clone(),
        Some(provider) => EmbeddingModelIdentity {
            provider: match provider {
                EmbeddingProvider::Local => "local",
                EmbeddingProvider::Openai => "openai",
                EmbeddingProvider::LexicalBaseline => "local-baseline",
            }.into(),
            model: config.embedding_model.clone().ok_or(
                ApplicationError::ResourceConfiguration("Supply --embedding-model.")
            )?,
            revision: config.embedding_revision.clone().ok_or(
                ApplicationError::ResourceConfiguration("Supply --embedding-revision with a pinned encoder revision.")
            )?,
            dimensions: config.embedding_dimensions.ok_or(
                ApplicationError::ResourceConfiguration("Supply --embedding-dimensions.")
            )?,
            encoding_revision: "plain-v1".into(),
        },
        None if config.embedding_model.is_some() || config.embedding_revision.is_some()
            || config.embedding_dimensions.is_some() => {
            return Err(ApplicationError::ResourceConfiguration(
                "Select --embedding-provider when supplying embedding model settings.",
            ));
        }
        None => previous.as_ref().map(|cache| cache.model.clone()).ok_or(
            ApplicationError::ResourceConfiguration(
                "Select --embedding-provider and configure its model; the text model does not supply embeddings.",
            )
        )?,
    };
    let previous = match previous {
        Some(cache) if cache.vectors(&identity, &inputs).is_ok() => return Ok(cache),
        previous => previous,
    };
    let cache = match config.embedding_provider {
        Some(EmbeddingProvider::LexicalBaseline) => {
            prepare_cache(&LexicalEmbedder::new(), &inputs, previous.as_ref()).await?
        }
        Some(EmbeddingProvider::Local) => {
            prepare_cache(
                &HttpEmbedder::local(&config.embedding_endpoint, identity)?,
                &inputs,
                previous.as_ref(),
            )
            .await?
        }
        Some(EmbeddingProvider::Openai) => {
            if !config.allow_embedding_call {
                return Err(ApplicationError::ResourceConfiguration(
                    "Hosted embeddings require --allow-embedding-call.",
                ));
            }
            let key = key.ok_or(ApplicationError::ResourceConfiguration(
                "Hosted embeddings need --openai-api-key or YOMIBU_OPENAI_API_KEY with embedding access.",
            ))?;
            prepare_cache(
                &HttpEmbedder::openai(key, identity)?,
                &inputs,
                previous.as_ref(),
            )
            .await?
        }
        None => {
            return Err(ApplicationError::ResourceConfiguration(
                "Configure --embedding-provider to refresh missing or stale vectors.",
            ));
        }
    };
    if let Some(parent) = config
        .embedding_cache
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|_| {
            ApplicationError::Resource(
                "Cannot create the embedding cache directory; check permissions.",
            )
        })?;
    }
    file.save(&cache)?;
    Ok(cache)
}
