pub mod components;
pub mod credentials;
mod documents;
mod invocation;
pub mod modules;
mod patch;
use self::modules::{MODULES, ModuleId};
use crate::application::Operation;
pub use invocation::*;
pub use patch::Patch;
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
use yomibu_core::domain::knowledge::LearnerKnowledgePolicy;
pub use yomibu_core::domain::story::StoryFormat;
use yomibu_core::domain::story::StoryGenerationOptions;

pub const ENVIRONMENT_SETTINGS: &[&str] = &[
    "YOMIBU_MODEL",
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
parse_setting!(
    ModuleId,
    "Choose a supported module shown in yomibu help story."
);

/// Typed process-input capture. Persisted documents use the scoped schemas.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub source: Option<components::Source>,
    pub learning_store: Option<components::LearningStore>,
    pub embedding_cache_component: Option<components::EmbeddingCache>,
    pub preparation: Option<components::Preparation>,
    pub generation_component: Option<components::Generation>,
    pub analysis: Option<components::Analysis>,
    pub assessment_component: Option<components::Assessment>,
    pub credential_bindings: BTreeMap<String, Patch<credentials::CredentialBinding>>,
    pub sync: Option<bool>,
    pub embeddings: Option<bool>,
    pub assessment: Option<bool>,
    pub steps: Option<Vec<SelectionStep>>,
    pub embedding_steps: Option<Vec<SelectionStep>>,
    pub vocabulary_targets: Option<Vec<String>>,
    pub grammar_targets: Option<Vec<String>>,
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

#[derive(Debug, Clone)]
pub struct Configuration {
    pub application: std::sync::Arc<ApplicationSettings>,
    pub pipeline: PipelineSettings,
    pub story: StorySettings,
}
#[derive(Debug)]
pub struct ApplicationSettings {
    pub data_dir: PathBuf,
    pub inventory: Option<PathBuf>,
    pub wanikani_cache: Option<PathBuf>,
    pub cache_max_age: Duration,
    pub dictionary_dir: PathBuf,
    pub(crate) dictionary_dir_explicit: bool,
    pub embedding_cache: PathBuf,
    pub allow_embedding_call: bool,
    pub credential_bindings: credentials::CredentialBindings,
    pub sync: bool,
}
#[derive(Debug, Clone)]
pub struct PipelineSettings {
    pub components: components::Components,
    pub selection: SelectionSettings,
    pub knowledge_policy: LearnerKnowledgePolicy,
    pub model: String,
    pub generation_model: Option<String>,
    pub embedding_provider: Option<EmbeddingProvider>,
    pub embedding_model: Option<String>,
    pub embedding_revision: Option<String>,
    pub embedding_dimensions: Option<usize>,
    pub embedding_endpoint: String,
    pub embeddings: bool,
    pub assessment: bool,
}
#[derive(Debug, Clone)]
pub struct StorySettings {
    pub request: Option<PathBuf>,
    pub topic: Option<String>,
    pub select: usize,
    pub seed: Option<u64>,
    pub format: StoryFormat,
    pub candidates: usize,
    pub vocabulary_targets: Vec<String>,
    pub grammar_targets: Vec<String>,
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
    pub fn load(mut input: ConfigurationInput, operation: &Operation) -> Result<Self, ConfigError> {
        let data_dir = input
            .data_dir
            .or_else(|| input.home.map(|home| home.join(".yomibu")))
            .ok_or(ConfigError::MissingHome)?;
        let explicit = input.config.is_some();
        let path = input.config.unwrap_or_else(|| data_dir.join("config.toml"));
        let mut file = documents::load(&path, explicit, operation)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        for value in [
            &mut file.inventory,
            &mut file.wanikani_cache,
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
        let mut env = environment_settings(input.environment, operation)?;
        let mut enabled: BTreeMap<_, _> = MODULES
            .iter()
            .map(|module| (module.id, module.default_enabled))
            .collect();
        if operation.uses_setting("enable") {
            for source in [&file, &env, &input.flags] {
                for (id, value) in [
                    (ModuleId::Sync, source.sync),
                    (ModuleId::Embeddings, source.embeddings),
                    (ModuleId::Assessment, source.assessment),
                ] {
                    if let Some(value) = value {
                        enabled.insert(id, value);
                    }
                }
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
                input
                    .flags
                    .$field
                    .take()
                    .or(env.$field.take())
                    .or(file.$field.take())
            };
        }
        let topic_conflict = input.flags.topic.is_some() || env.topic.is_some();
        let request = setting!(request);
        let credential_bindings = credentials::CredentialBindings::resolve(std::mem::take(
            &mut file.credential_bindings,
        ))?;
        let dictionary_dir = setting!(dictionary_dir);
        let mut resolved = Self {
            application: std::sync::Arc::new(ApplicationSettings {
                inventory: setting!(inventory),
                wanikani_cache: setting!(wanikani_cache),
                cache_max_age: Duration::from_secs(setting!(cache_max_age_seconds).unwrap_or(3600)),
                dictionary_dir_explicit: dictionary_dir.is_some(),
                dictionary_dir: dictionary_dir.unwrap_or_else(|| data_dir.join("dictionaries")),
                embedding_cache: setting!(embedding_cache)
                    .unwrap_or_else(|| data_dir.join("embeddings.json")),
                allow_embedding_call: setting!(allow_embedding_call).unwrap_or(false),
                credential_bindings,
                sync: enabled[&ModuleId::Sync],
                data_dir,
            }),
            pipeline: PipelineSettings {
                components: components::Components::default(),
                selection: SelectionSettings {
                    steps: vec![SelectionStep::LexicalTopic, SelectionStep::SeededOrder],
                    embedding_steps: vec![SelectionStep::EmbeddingRank],
                },
                knowledge_policy: LearnerKnowledgePolicy::default(),
                model: StoryGenerationOptions::default().model,
                generation_model: None,
                embedding_provider: None,
                embedding_model: None,
                embedding_revision: None,
                embedding_dimensions: None,
                embedding_endpoint: "http://127.0.0.1:11434/v1/".into(),
                embeddings: false,
                assessment: true,
            },
            story: StorySettings {
                request,
                topic: None,
                select: 12,
                seed: None,
                format: StoryFormat::default(),
                candidates: 1,
                vocabulary_targets: Vec::new(),
                grammar_targets: Vec::new(),
            },
        };
        for source in [file, env, input.flags] {
            resolved.apply(source.into_invocation(), operation);
        }
        resolved.pipeline.embeddings = enabled[&ModuleId::Embeddings];
        resolved.pipeline.assessment = enabled[&ModuleId::Assessment];
        resolved.validate(operation)?;
        if operation.uses_setting("topic") && topic_conflict && resolved.story.request.is_some() {
            return Err(ConfigError::InvalidSetting(
                "--topic and --request conflict; put the topic in the request file.",
            ));
        }
        Ok(resolved)
    }
    pub fn generation(&self) -> StoryGenerationOptions {
        StoryGenerationOptions {
            model: self
                .pipeline
                .generation_model
                .clone()
                .unwrap_or_else(|| self.pipeline.model.clone()),
            format: self.story.format,
            candidate_count: self.story.candidates,
        }
    }
    pub fn enabled(&self, id: ModuleId) -> bool {
        match id {
            ModuleId::Knowledge | ModuleId::Generation => true,
            ModuleId::Sync => self.application.sync,
            ModuleId::Embeddings => self.pipeline.embeddings,
            ModuleId::Assessment => self.pipeline.assessment,
        }
    }
}

fn environment_settings(
    environment: BTreeMap<String, String>,
    operation: &Operation,
) -> Result<Settings, ConfigError> {
    let mut values = serde_json::Map::new();
    for (name, text) in environment {
        let field = if let Some((_, field)) = component_fields()
            .into_iter()
            .find(|(option, _)| option.environment() == name)
        {
            field.to_owned()
        } else if ENVIRONMENT_SETTINGS.contains(&name.as_str()) {
            name.trim_start_matches("YOMIBU_").to_ascii_lowercase()
        } else {
            continue;
        };
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelectionStep {
    LexicalTopic,
    SeededOrder,
    EmbeddingRank,
}
#[derive(Debug, Clone)]
pub struct SelectionSettings {
    pub steps: Vec<SelectionStep>,
    pub embedding_steps: Vec<SelectionStep>,
}
impl SelectionSettings {
    fn validate(&self) -> Result<(), ConfigError> {
        use SelectionStep::*;
        if !matches!(
            self.steps.as_slice(),
            [LexicalTopic, SeededOrder] | [SeededOrder]
        ) || !matches!(
            self.embedding_steps.as_slice(),
            [EmbeddingRank] | [EmbeddingRank, SeededOrder]
        ) {
            return Err(ConfigError::InvalidSetting(
                "Selection requires [lexical-topic, seeded-order] or [seeded-order]; embedding selection requires [embedding-rank] or [embedding-rank, seeded-order].",
            ));
        }
        Ok(())
    }
}

fn component_fields() -> [(yomibu_core::capabilities::options::OptionName, &'static str); 5] {
    [
        (components::GENERATION_MODEL.name, "generation_model"),
        (components::EMBEDDING_MODEL.name, "embedding_model"),
        (components::EMBEDDING_REVISION.name, "embedding_revision"),
        (
            components::EMBEDDING_DIMENSIONS.name,
            "embedding_dimensions",
        ),
        (components::EMBEDDING_ENDPOINT.name, "embedding_endpoint"),
    ]
}
pub fn environment_names() -> Vec<String> {
    ENVIRONMENT_SETTINGS
        .iter()
        .map(|name| (*name).into())
        .chain(
            component_fields()
                .into_iter()
                .map(|(option, _)| option.environment()),
        )
        .collect()
}
