use super::selection::{select_builtin_vocabulary, select_vocabulary};
use super::{
    local::ApplicationError,
    progress::{ProgressEvent, RunProgress, Step},
};
use crate::configuration::modules::{ModuleId, ModuleState};
use crate::configuration::{Configuration, EmbeddingProvider};
use crate::reports::run::Warning;
use yomibu_components::{
    embedding_vocabulary_selection::prepare_embedding_inputs,
    file_embedding_cache::EmbeddingCacheFile, http_embeddings::HttpEmbedder,
    lexical_embeddings::LexicalEmbedder,
};
use yomibu_core::{
    capabilities::Embedder,
    domain::{
        embedding::{EmbeddingCache, EmbeddingModelIdentity},
        inventory::LearnerInventory,
        story::{StoryError, StoryRequest, StoryVocabularySelection},
    },
    pipeline::embeddings::prepare_cache,
};

pub(super) async fn prepare_embeddings(
    config: &Configuration,
    key: Option<&str>,
    inventory: &LearnerInventory,
    request: &StoryRequest,
) -> Result<EmbeddingCache, ApplicationError> {
    let inputs = prepare_embedding_inputs(inventory, request)?;
    let file = EmbeddingCacheFile::new(&config.embedding_cache);
    let previous = file.load()?;
    let identity = embedding_model(config, previous.as_ref())?;
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

fn load_cached_embeddings(config: &Configuration) -> Result<EmbeddingCache, ApplicationError> {
    let cache = EmbeddingCacheFile::new(&config.embedding_cache)
        .load()?
        .ok_or(yomibu_core::domain::embedding::EmbeddingError::Missing)?;
    if cache.model != embedding_model(config, Some(&cache))? {
        return Err(yomibu_core::domain::embedding::EmbeddingError::Missing.into());
    }
    Ok(cache)
}

fn embedding_model(
    config: &Configuration,
    previous: Option<&EmbeddingCache>,
) -> Result<EmbeddingModelIdentity, ApplicationError> {
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
        None => previous.map(|cache| cache.model.clone()).ok_or(
            ApplicationError::ResourceConfiguration(
                "Select --embedding-provider and configure its model; the text model does not supply embeddings.",
            )
        )?,
    };
    Ok(identity)
}

pub(super) async fn prepare_optional<F: FnMut(ProgressEvent)>(
    config: &Configuration,
    key: Option<&str>,
    inventory: &LearnerInventory,
    request: &StoryRequest,
    progress: &mut RunProgress<F>,
) -> Option<EmbeddingCache> {
    if !config.enabled(ModuleId::Embeddings) {
        progress.skip(Step::Embeddings, "Embeddings disabled");
        return None;
    }
    if request.topic.is_none() {
        progress.state(
            ModuleId::Embeddings,
            ModuleState::Skipped {
                reason: "No topic; query retrieval is unnecessary".into(),
            },
        );
        progress.skip(Step::Embeddings, "No topic");
        return None;
    }
    let started = progress.start(Step::Embeddings);
    let result = prepare_embeddings(config, key, inventory, request).await;
    progress.finish(Step::Embeddings, started);
    match result {
        Ok(cache) => {
            progress.state(ModuleId::Embeddings, ModuleState::Available);
            Some(cache)
        }
        Err(error) => {
            progress.warnings.push(Warning::embedding_fallback(&error));
            progress.state(
                ModuleId::Embeddings,
                match error {
                    ApplicationError::ResourceConfiguration(_) => ModuleState::NotConfigured,
                    _ => ModuleState::Unavailable {
                        error: error.to_string(),
                    },
                },
            );
            None
        }
    }
}

pub(super) fn load_optional(
    config: &Configuration,
    request: &StoryRequest,
    warnings: &mut Vec<Warning>,
) -> Option<EmbeddingCache> {
    if !config.enabled(ModuleId::Embeddings) || request.topic.is_none() {
        return None;
    }
    match load_cached_embeddings(config) {
        Ok(cache) => Some(cache),
        Err(error) => {
            warnings.push(Warning::embedding_fallback(&error));
            None
        }
    }
}

pub(super) fn select_for_request<'a>(
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    cache: Option<&EmbeddingCache>,
    limit: usize,
    seed: u64,
) -> Result<(StoryVocabularySelection<'a>, Option<StoryError>), StoryError> {
    match cache
        .map(|cache| select_vocabulary(inventory, request, cache, &cache.model, limit))
        .transpose()
    {
        Ok(Some(selection)) => Ok((selection, None)),
        result => Ok((
            select_builtin_vocabulary(inventory, request, limit, seed)?,
            result.err(),
        )),
    }
}
