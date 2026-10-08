use super::{ConfigError, Settings};
use crate::application::Operation;
use std::{io::Read, path::Path};

const APPLICATION: &[(&str, &str)] = &[
    ("application.credentials", "credential_bindings"),
    ("application.sync", "sync"),
    ("application.inventory", "inventory"),
    ("application.wanikani_cache", "wanikani_cache"),
    ("application.dictionary_dir", "dictionary_dir"),
    ("application.embedding_cache", "embedding_cache"),
    ("application.cache_max_age_seconds", "cache_max_age_seconds"),
    ("application.allow_embedding_call", "allow_embedding_call"),
];
const PIPELINE: &[(&str, &str)] = &[
    ("pipeline.components.source", "source"),
    ("pipeline.components.learning_store", "learning_store"),
    (
        "pipeline.components.embedding_cache",
        "embedding_cache_component",
    ),
    ("pipeline.components.preparation", "preparation"),
    ("pipeline.components.generation", "generation_component"),
    ("pipeline.components.analysis", "analysis"),
    ("pipeline.components.assessment", "assessment_component"),
    ("pipeline.selection.steps", "steps"),
    ("pipeline.selection.embedding_steps", "embedding_steps"),
    ("pipeline.selection.embeddings", "embeddings"),
    ("pipeline.assessment.enabled", "assessment"),
    ("pipeline.model", "model"),
    ("pipeline.knowledge_policy", "knowledge_policy"),
    (
        "pipeline.options.openai-story-generation.model",
        "generation_model",
    ),
    ("pipeline.embedding.provider", "embedding_provider"),
    ("pipeline.options.http-embeddings.model", "embedding_model"),
    (
        "pipeline.options.http-embeddings.revision",
        "embedding_revision",
    ),
    (
        "pipeline.options.http-embeddings.dimensions",
        "embedding_dimensions",
    ),
    (
        "pipeline.options.http-embeddings.endpoint",
        "embedding_endpoint",
    ),
];
const STORY: &[(&str, &str)] = &[
    ("story.targets.vocabulary", "vocabulary_targets"),
    ("story.targets.grammar", "grammar_targets"),
    ("story.topic", "topic"),
    ("story.select", "select"),
    ("story.seed", "seed"),
    ("story.format", "format"),
    ("story.candidates", "candidates"),
];

pub(super) fn load(
    path: &Path,
    explicit: bool,
    operation: &Operation,
) -> Result<Settings, ConfigError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut values = toml::Table::new();
    for (path, explicit, fields, needed) in [
        (path.to_owned(), explicit, APPLICATION, true),
        (parent.join("default-pipeline.toml"), false, PIPELINE, true),
        (
            parent.join("default-story.toml"),
            false,
            STORY,
            operation.uses_setting("topic"),
        ),
    ] {
        if !needed {
            continue;
        }
        let invalid = || ConfigError::Invalid { path: path.clone() };
        let table = read(&path, explicit)?;
        collect(&table, "", fields, operation, &mut values).ok_or_else(invalid)?;
    }
    values
        .try_into()
        .map_err(|_| ConfigError::Invalid { path: path.into() })
}

fn collect(
    table: &toml::Table,
    prefix: &str,
    fields: &[(&str, &str)],
    operation: &Operation,
    out: &mut toml::Table,
) -> Option<()> {
    for (key, value) in table {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if let Some((_, field)) = fields.iter().find(|(name, _)| *name == path) {
            if operation.uses_setting(field) {
                if value.as_table().is_some_and(|table| {
                    table.len() == 1
                        && table.get("clear").and_then(toml::Value::as_bool) == Some(true)
                }) && matches!(
                    *field,
                    "topic"
                        | "seed"
                        | "inventory"
                        | "wanikani_cache"
                        | "dictionary_dir"
                        | "embedding_cache"
                        | "generation_model"
                        | "embedding_provider"
                        | "embedding_model"
                        | "embedding_revision"
                        | "embedding_dimensions"
                        | "embedding_endpoint"
                ) {
                    continue;
                }
                let single = toml::Table::from_iter([((*field).into(), value.clone())]);
                let _: Settings = single.try_into().ok()?;
                out.insert((*field).into(), value.clone());
            }
        } else if fields
            .iter()
            .any(|(name, _)| name.starts_with(&format!("{path}.")))
        {
            collect(value.as_table()?, &path, fields, operation, out)?;
        } else {
            return None;
        }
    }
    Some(())
}

fn read(path: &Path, explicit: bool) -> Result<toml::Table, ConfigError> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if !explicit && error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(toml::Table::new());
        }
        Err(_) => return Err(ConfigError::Read { path: path.into() }),
    };
    let mut bytes = Vec::new();
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| ConfigError::Read { path: path.into() })?;
    if bytes.len() <= 65536
        && let Ok(text) = std::str::from_utf8(&bytes)
        && let Ok(table) = toml::from_str(text)
    {
        return Ok(table);
    }
    Err(ConfigError::Invalid { path: path.into() })
}
