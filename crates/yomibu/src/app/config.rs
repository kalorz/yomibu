use super::{
    Operation,
    modules::{MODULES, ModuleId},
};
use crate::{
    knowledge::{LearnerKnowledgePolicy, WaniKaniKnowledgeRule},
    story::{StoryFormat, StoryGenerationOptions},
};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read, path::PathBuf, time::Duration};

pub const ENVIRONMENT_SETTINGS: &[&str] = &[
    "YOMIBU_MODEL",
    "YOMIBU_GENERATION_MODEL",
    "YOMIBU_INVENTORY",
    "YOMIBU_WANIKANI_CACHE",
    "YOMIBU_KNOWLEDGE_POLICY",
    "YOMIBU_REQUEST",
    "YOMIBU_TOPIC",
    "YOMIBU_SELECT",
    "YOMIBU_SEED",
    "YOMIBU_FORMAT",
    "YOMIBU_CANDIDATES",
    "YOMIBU_CACHE_MAX_AGE_SECONDS",
    "YOMIBU_DICTIONARY_DIR",
    "YOMIBU_EMBEDDING_CACHE",
    "YOMIBU_EMBEDDING_PROVIDER",
    "YOMIBU_EMBEDDING_MODEL",
    "YOMIBU_EMBEDDING_REVISION",
    "YOMIBU_EMBEDDING_DIMENSIONS",
    "YOMIBU_EMBEDDING_ENDPOINT",
    "YOMIBU_ALLOW_EMBEDDING_CALL",
    "YOMIBU_ENABLE",
    "YOMIBU_DISABLE",
];

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddingProvider {
    LexicalBaseline,
    Local,
    Openai,
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnowledgePolicy {
    LessonStarted,
    RecordedPass,
}

macro_rules! parse_setting {
    ($type:ty, $message:literal) => {
        impl std::str::FromStr for $type {
            type Err = &'static str;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                serde_json::from_value(serde_json::Value::String(value.into()))
                    .map_err(|_| $message)
            }
        }
    };
}
parse_setting!(
    EmbeddingProvider,
    "Choose lexical-baseline, local, or openai."
);
parse_setting!(KnowledgePolicy, "Choose lesson-started or recorded-pass.");
parse_setting!(StoryFormat, "Choose passage or sentence.");
parse_setting!(
    ModuleId,
    "Choose a supported module shown in yomibu help story."
);

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub model: Option<String>,
    pub generation_model: Option<String>,
    pub inventory: Option<PathBuf>,
    pub wanikani_cache: Option<PathBuf>,
    pub knowledge_policy: Option<KnowledgePolicy>,
    pub request: Option<PathBuf>,
    pub topic: Option<String>,
    pub select: Option<usize>,
    pub seed: Option<u64>,
    pub format: Option<StoryFormat>,
    pub candidates: Option<usize>,
    pub cache_max_age_seconds: Option<u64>,
    pub dictionary_dir: Option<PathBuf>,
    pub embedding_cache: Option<PathBuf>,
    pub embedding_provider: Option<EmbeddingProvider>,
    pub embedding_model: Option<String>,
    pub embedding_revision: Option<String>,
    pub embedding_dimensions: Option<usize>,
    pub embedding_endpoint: Option<String>,
    pub allow_embedding_call: Option<bool>,
    pub enable: Vec<ModuleId>,
    pub disable: Vec<ModuleId>,
}

pub struct ConfigurationInput {
    pub data_dir: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub environment: BTreeMap<String, String>,
    pub flags: Settings,
}

#[derive(Debug)]
pub struct Configuration {
    pub data_dir: PathBuf,
    pub inventory: Option<PathBuf>,
    pub wanikani_cache: Option<PathBuf>,
    pub knowledge_policy: LearnerKnowledgePolicy,
    pub request: Option<PathBuf>,
    pub topic: Option<String>,
    pub select: usize,
    pub seed: Option<u64>,
    pub generation: StoryGenerationOptions,
    pub cache_max_age: Duration,
    pub dictionary_dir: PathBuf,
    pub(crate) dictionary_dir_explicit: bool,
    pub embedding_cache: PathBuf,
    pub embedding_provider: Option<EmbeddingProvider>,
    pub embedding_model: Option<String>,
    pub embedding_revision: Option<String>,
    pub embedding_dimensions: Option<usize>,
    pub embedding_endpoint: String,
    pub allow_embedding_call: bool,
    enabled: BTreeMap<ModuleId, bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("HOME is unavailable; supply --data-dir PATH.")]
    MissingHome,
    #[error("Cannot read configuration at {path}; check the path and permissions.")]
    Read { path: PathBuf },
    #[error("Invalid configuration at {path}; use supported settings and valid TOML.")]
    Invalid { path: PathBuf },
    #[error("Invalid value for {name}; check its format in yomibu help story.")]
    Environment { name: String },
    #[error("Cannot both enable and disable {module} in the same configuration source.")]
    ConflictingControl { module: &'static str },
    #[error("{0}")]
    InvalidSetting(&'static str),
}

impl Configuration {
    pub fn load(input: ConfigurationInput, operation: &Operation) -> Result<Self, ConfigError> {
        let data_dir = input
            .data_dir
            .or_else(|| input.home.map(|home| home.join(".yomibu")))
            .ok_or(ConfigError::MissingHome)?;
        let explicit = input.config.is_some();
        let path = input.config.unwrap_or_else(|| data_dir.join("config.toml"));
        let mut file: Settings = match std::fs::File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(65537)
                    .read_to_end(&mut bytes)
                    .map_err(|_| ConfigError::Read { path: path.clone() })?;
                if bytes.len() > 65536 {
                    return Err(ConfigError::Invalid { path });
                }
                std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(|text| file_settings(text, operation))
                    .ok_or_else(|| ConfigError::Invalid { path: path.clone() })?
            }
            Err(error) if !explicit && error.kind() == std::io::ErrorKind::NotFound => {
                Settings::default()
            }
            Err(_) => return Err(ConfigError::Read { path }),
        };
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        for value in [
            &mut file.inventory,
            &mut file.wanikani_cache,
            &mut file.request,
            &mut file.dictionary_dir,
            &mut file.embedding_cache,
        ]
        .into_iter()
        .flatten()
        {
            if value.is_relative() {
                *value = parent.join(&*value);
            }
        }
        let env = environment_settings(input.environment, operation)?;
        let mut enabled: BTreeMap<_, _> = MODULES
            .iter()
            .map(|module| (module.id, module.default_enabled))
            .collect();
        if operation.uses_setting("enable") {
            for source in [&file, &env, &input.flags] {
                for id in source.enable.iter().chain(&source.disable) {
                    if !id.metadata().controllable {
                        return Err(ConfigError::InvalidSetting(
                            "Only sync, embeddings, and assessment can be enabled or disabled.",
                        ));
                    }
                    if source.enable.contains(id) && source.disable.contains(id) {
                        return Err(ConfigError::ConflictingControl {
                            module: id.metadata().name,
                        });
                    }
                    enabled.insert(*id, source.enable.contains(id));
                }
            }
        }
        macro_rules! setting {
            ($field:ident) => {
                input.flags.$field.or(env.$field).or(file.$field)
            };
        }
        let generation = StoryGenerationOptions {
            model: setting!(generation_model)
                .or(setting!(model))
                .unwrap_or_else(|| StoryGenerationOptions::default().model),
            candidate_count: setting!(candidates).unwrap_or(1),
            format: setting!(format).unwrap_or_default(),
        };
        if operation.uses_setting("model") {
            generation.validate().map_err(|_| ConfigError::InvalidSetting("Choose a nonblank text model and a positive candidate count that fits the output budget."))?;
        }
        let select = setting!(select).unwrap_or(12);
        if operation.uses_setting("select") && !(1..=16).contains(&select) {
            return Err(ConfigError::InvalidSetting(
                "--select must be between 1 and 16.",
            ));
        }
        let topic = setting!(topic);
        let request = setting!(request);
        if operation.uses_setting("topic") && topic.is_some() && request.is_some() {
            return Err(ConfigError::InvalidSetting(
                "--topic and --request conflict; put the topic in the request file.",
            ));
        }
        let dictionary_dir = setting!(dictionary_dir);
        Ok(Self {
            inventory: setting!(inventory),
            wanikani_cache: setting!(wanikani_cache),
            knowledge_policy: LearnerKnowledgePolicy {
                wanikani: match setting!(knowledge_policy).unwrap_or(KnowledgePolicy::LessonStarted)
                {
                    KnowledgePolicy::LessonStarted => WaniKaniKnowledgeRule::LessonStarted,
                    KnowledgePolicy::RecordedPass => WaniKaniKnowledgeRule::RecordedPass,
                },
            },
            request,
            topic,
            select,
            seed: setting!(seed),
            generation,
            cache_max_age: Duration::from_secs(setting!(cache_max_age_seconds).unwrap_or(3600)),
            dictionary_dir_explicit: dictionary_dir.is_some(),
            dictionary_dir: dictionary_dir.unwrap_or_else(|| data_dir.join("dictionaries")),
            embedding_cache: setting!(embedding_cache)
                .unwrap_or_else(|| data_dir.join("embeddings.json")),
            embedding_provider: setting!(embedding_provider),
            embedding_model: setting!(embedding_model),
            embedding_revision: setting!(embedding_revision),
            embedding_dimensions: setting!(embedding_dimensions),
            embedding_endpoint: setting!(embedding_endpoint)
                .unwrap_or_else(|| "http://127.0.0.1:11434/v1/".into()),
            allow_embedding_call: setting!(allow_embedding_call).unwrap_or(false),
            enabled,
            data_dir,
        })
    }

    pub fn enabled(&self, id: ModuleId) -> bool {
        self.enabled
            .get(&id)
            .copied()
            .unwrap_or(id.metadata().default_enabled)
    }
}

fn file_settings(text: &str, operation: &Operation) -> Option<Settings> {
    let mut values: toml::Table = toml::from_str(text).ok()?;
    if values.keys().any(|name| {
        !ENVIRONMENT_SETTINGS
            .iter()
            .any(|env| env.trim_start_matches("YOMIBU_").to_ascii_lowercase() == *name)
    }) {
        return None;
    }
    values.retain(|name, _| operation.uses_setting(name));
    values.try_into().ok()
}

fn environment_settings(
    environment: BTreeMap<String, String>,
    operation: &Operation,
) -> Result<Settings, ConfigError> {
    let mut values = serde_json::Map::new();
    for (name, text) in environment {
        if !ENVIRONMENT_SETTINGS.contains(&name.as_str()) {
            continue;
        }
        let field = name.trim_start_matches("YOMIBU_").to_ascii_lowercase();
        if !operation.uses_setting(&field) {
            continue;
        }
        let value = match field.as_str() {
            "select" | "seed" | "candidates" | "cache_max_age_seconds" | "embedding_dimensions" => {
                serde_json::Value::from(
                    text.parse::<u64>()
                        .map_err(|_| ConfigError::Environment { name: name.clone() })?,
                )
            }
            "allow_embedding_call" => serde_json::Value::from(
                text.parse::<bool>()
                    .map_err(|_| ConfigError::Environment { name: name.clone() })?,
            ),
            "enable" | "disable" => serde_json::Value::from(
                text.split(',')
                    .map(|s| s.trim().to_owned())
                    .collect::<Vec<_>>(),
            ),
            _ => serde_json::Value::from(text),
        };
        let mut single = serde_json::Map::new();
        single.insert(field.clone(), value.clone());
        serde_json::from_value::<Settings>(single.into())
            .map_err(|_| ConfigError::Environment { name })?;
        values.insert(field, value);
    }
    serde_json::from_value(values.into())
        .map_err(|_| ConfigError::InvalidSetting("Invalid environment configuration."))
}
