use super::{ConfigError, Invocation, Patch, components};
use crate::composition::Operation;
use std::{io::Read, path::Path};

const PIPELINE: &[&str] = &[
    "pipeline.components.source",
    "pipeline.components.learning_store",
    "pipeline.components.preparation",
    "pipeline.components.generation",
    "pipeline.selection.steps",
];
const STORY: &[&str] = &[
    "story.targets.vocabulary",
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
) -> Result<Invocation, ConfigError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if !read(path, explicit)?.is_empty() {
        return Err(ConfigError::Invalid { path: path.into() });
    }
    let mut input = Invocation::default();
    let paths: Vec<_> = super::components::options()
        .map(super::options::path)
        .collect();
    let pipeline: Vec<_> = PIPELINE
        .iter()
        .copied()
        .chain(paths.iter().map(String::as_str))
        .collect();
    for (path, explicit, fields, scope) in [
        (
            parent.join("default-pipeline.toml"),
            false,
            pipeline.as_slice(),
            "pipeline",
        ),
        (parent.join("default-story.toml"), false, STORY, "story"),
    ] {
        if scope == "story" && !operation.uses_setting("story.topic") {
            continue;
        }
        let invalid = || ConfigError::Invalid { path: path.clone() };
        let table = prune(read(&path, explicit)?, "", fields, operation, &path)?;
        let source: Invocation = table.try_into().map_err(|_| invalid())?;
        match scope {
            "pipeline" => input.pipeline = source.pipeline,
            _ => input.story = source.story,
        }
    }
    Ok(input)
}

fn prune(
    table: toml::Table,
    prefix: &str,
    fields: &[&str],
    operation: &Operation,
    file: &Path,
) -> Result<toml::Table, ConfigError> {
    let invalid = || ConfigError::Invalid { path: file.into() };
    let mut out = toml::Table::new();
    for (key, value) in table {
        if key.contains('.') {
            return Err(invalid());
        }
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if fields.contains(&path.as_str()) {
            if operation.uses_setting(&path) {
                if let Some(setting) =
                    components::options().find(|setting| super::options::path(*setting) == path)
                {
                    let option_error = |error| ConfigError::ComponentFile {
                        path: file.into(),
                        error,
                    };
                    let patch: Patch<components::Value> =
                        value.clone().try_into().map_err(|_| {
                            option_error(components::OptionError::Invalid {
                                name: setting.name(),
                                reason: "Wrong component option type.",
                            })
                        })?;
                    if let Patch::Set(value) = patch {
                        setting.check(&value).map_err(option_error)?;
                    }
                }
                out.insert(key, value);
            }
        } else if fields
            .iter()
            .any(|name| name.starts_with(&format!("{path}.")))
        {
            let toml::Value::Table(table) = value else {
                return Err(invalid());
            };
            out.insert(key, prune(table, &path, fields, operation, file)?.into());
        } else {
            return Err(invalid());
        }
    }
    Ok(out)
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
