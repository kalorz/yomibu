use super::selection::select_configured;
use super::{
    local::ApplicationError,
    progress::{ProgressEvent, RunProgress, Step},
};
use crate::configuration::components::EMBEDDING_KEY;
use crate::configuration::modules::{ModuleId, ModuleState};
use crate::configuration::{Configuration, EmbeddingProvider};
use crate::reports::run::Warning;
use yomibu_components::{
    embedding_vocabulary_selection::prepare_embedding_inputs,
    http_embeddings::{self, HttpEmbedder},
    lexical_embeddings::LexicalEmbedder,
};
use yomibu_core::{
    capabilities::Embedder,
    component::options::OptionError,
    domain::{
        embedding::{EmbeddingCache, EmbeddingModelIdentity},
        inventory::LearnerInventory,
        story::{StoryError, StoryRequest, StoryVocabularySelection},
    },
    pipeline::embeddings::prepare_cache,
};

pub(super) async fn prepare_embeddings(
    config: &Configuration,
    credentials: &super::Credentials,
    inventory: &LearnerInventory,
    request: &StoryRequest,
) -> Result<EmbeddingCache, ApplicationError> {
    let inputs = prepare_embedding_inputs(inventory, request)?;
    let file = config
        .pipeline
        .components
        .embedding_cache
        .open(&config.application.embedding_cache);
    let previous = file.load()?;
    let identity = embedding_model(config, previous.as_ref())?;
    let previous = match previous {
        Some(cache) if cache.vectors(&identity, &inputs).is_ok() => return Ok(cache),
        previous => previous,
    };
    let cache = match config.pipeline.embedding_provider {
        Some(EmbeddingProvider::LexicalBaseline) => {
            prepare_cache(&LexicalEmbedder::new(), &inputs, previous.as_ref()).await?
        }
        Some(EmbeddingProvider::Local) => {
            prepare_cache(
                &HttpEmbedder::local(
                    config
                        .pipeline
                        .options
                        .for_component(&http_embeddings::COMPONENT),
                    identity,
                )?,
                &inputs,
                previous.as_ref(),
            )
            .await?
        }
        Some(EmbeddingProvider::Openai) => {
            if !config.application.allow_embedding_call {
                return Err(ApplicationError::ResourceConfiguration(
                    "Hosted embeddings require --allow-embedding-call.",
                ));
            }
            let key = credentials
                .resolve(EMBEDDING_KEY)?
                .ok_or(ApplicationError::MissingCredential(EMBEDDING_KEY))?;
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
        .application
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
    let cache = config
        .pipeline
        .components
        .embedding_cache
        .open(&config.application.embedding_cache)
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
    let options = config
        .pipeline
        .options
        .for_component(&http_embeddings::COMPONENT);
    let identity = match config.pipeline.embedding_provider {
        Some(EmbeddingProvider::LexicalBaseline) => LexicalEmbedder::new().model_identity().clone(),
        Some(provider) => http_embeddings::model_identity(options, match provider {
            EmbeddingProvider::Local => "local",
            EmbeddingProvider::Openai => "openai",
            EmbeddingProvider::LexicalBaseline => "local-baseline",
        })?,
        None if http_embeddings::has_model_options(options)? => {
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
    credentials: &super::Credentials,
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
    let result = prepare_embeddings(config, credentials, inventory, request).await;
    progress.finish(Step::Embeddings, started);
    match result {
        Ok(cache) => {
            progress.state(ModuleId::Embeddings, ModuleState::Available);
            Some(cache)
        }
        Err(error) => {
            progress.warnings.push(Warning::embedding_fallback(
                &error,
                &config.pipeline.selection,
            ));
            progress.state(
                ModuleId::Embeddings,
                match error {
                    ApplicationError::ResourceConfiguration(_)
                    | ApplicationError::Options(OptionError::Missing { .. })
                    | ApplicationError::MissingCredential(_) => ModuleState::NotConfigured,
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
            warnings.push(Warning::embedding_fallback(
                &error,
                &config.pipeline.selection,
            ));
            None
        }
    }
}

pub(super) fn select_for_request<'a>(
    settings: &crate::configuration::SelectionSettings,
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    cache: Option<&EmbeddingCache>,
    limit: usize,
    seed: u64,
) -> Result<(StoryVocabularySelection<'a>, Option<StoryError>), StoryError> {
    match cache
        .map(|cache| select_configured(settings, inventory, request, Some(cache), limit, seed))
        .transpose()
    {
        Ok(Some(selection)) => Ok((selection, None)),
        result => Ok((
            select_configured(settings, inventory, request, None, limit, seed)?,
            result.err(),
        )),
    }
}
