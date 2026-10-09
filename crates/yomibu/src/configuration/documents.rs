use super::{ConfigError, ProcessOverrides};
use crate::application::Operation;
use std::{io::Read, path::Path};

const APPLICATION: &[&str] = &[
    "application.sync",
    "application.inventory",
    "application.wanikani_cache",
    "application.dictionary_dir",
    "application.embedding_cache",
    "application.cache_max_age_seconds",
    "application.allow_embedding_call",
];
const PIPELINE: &[&str] = &[
    "pipeline.components.source",
    "pipeline.components.learning_store",
    "pipeline.components.embedding_cache",
    "pipeline.components.preparation",
    "pipeline.components.generation",
    "pipeline.components.analysis",
    "pipeline.components.assessment",
    "pipeline.selection.steps",
    "pipeline.selection.embedding_steps",
    "pipeline.selection.embeddings",
    "pipeline.assessment.enabled",
    "pipeline.model",
    "pipeline.knowledge_policy",
    "pipeline.options.openai.model",
    "pipeline.embedding.provider",
    "pipeline.options.http-embeddings.model",
    "pipeline.options.http-embeddings.revision",
    "pipeline.options.http-embeddings.dimensions",
    "pipeline.options.http-embeddings.endpoint",
];
const STORY: &[&str] = &[
    "story.targets.vocabulary",
    "story.targets.grammar",
    "story.topic",
    "story.select",
    "story.seed",
    "story.format",
    "story.candidates",
];

pub(super) fn load(
    path: &Path,
    explicit: bool,
    operation: &Operation,
) -> Result<ProcessOverrides, ConfigError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut input = ProcessOverrides::default();
    for (path, explicit, fields, scope) in [
        (path.to_owned(), explicit, APPLICATION, "application"),
        (
            parent.join("default-pipeline.toml"),
            false,
            PIPELINE,
            "pipeline",
        ),
        (parent.join("default-story.toml"), false, STORY, "story"),
    ] {
        if scope == "story" && !operation.uses_setting("story.topic") {
            continue;
        }
        let invalid = || ConfigError::Invalid { path: path.clone() };
        let table = prune(read(&path, explicit)?, "", fields, operation).ok_or_else(invalid)?;
        let source: ProcessOverrides = table.try_into().map_err(|_| invalid())?;
        match scope {
            "application" => input.application = source.application,
            "pipeline" => input.invocation.pipeline = source.invocation.pipeline,
            _ => input.invocation.story = source.invocation.story,
        }
    }
    Ok(input)
}

fn prune(
    table: toml::Table,
    prefix: &str,
    fields: &[&str],
    operation: &Operation,
) -> Option<toml::Table> {
    let mut out = toml::Table::new();
    for (key, value) in table {
        if key.contains('.') {
            return None;
        }
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if fields.contains(&path.as_str()) {
            if operation.uses_setting(&path) {
                if matches!(
                    path.as_str(),
                    "application.inventory"
                        | "application.wanikani_cache"
                        | "application.dictionary_dir"
                        | "application.embedding_cache"
                ) && value.as_table().is_some_and(|table| {
                    table.len() == 1
                        && table.get("clear").and_then(toml::Value::as_bool) == Some(true)
                }) {
                    continue;
                }
                out.insert(key, value);
            }
        } else if fields
            .iter()
            .any(|name| name.starts_with(&format!("{path}.")))
        {
            let toml::Value::Table(table) = value else {
                return None;
            };
            out.insert(key, prune(table, &path, fields, operation)?.into());
        } else {
            return None;
        }
    }
    Some(out)
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
