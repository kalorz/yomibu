pub mod components;
mod documents;
mod invocation;
pub mod modules;
mod options;
mod patch;
use self::modules::ModuleId;
use crate::application::Operation;
pub use invocation::*;
pub use options::OptionOverrides;
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
                Self::deserialize(
                    serde::de::value::StrDeserializer::<serde::de::value::Error>::new(value),
                )
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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ProcessOverrides {
    pub application: ApplicationOverrides,
    #[serde(flatten)]
    pub invocation: Invocation,
    #[serde(skip)]
    pub request: Option<PathBuf>,
    #[serde(skip)]
    pub enable: Vec<ModuleId>,
    #[serde(skip)]
    pub disable: Vec<ModuleId>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ApplicationOverrides {
    pub inventory: Option<PathBuf>,
    pub wanikani_cache: Option<PathBuf>,
    pub cache_max_age_seconds: Option<u64>,
    pub dictionary_dir: Option<PathBuf>,
    pub embedding_cache: Option<PathBuf>,
    pub allow_embedding_call: Option<bool>,
    pub sync: Option<bool>,
}

pub struct ConfigurationInput {
    pub data_dir: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub environment: BTreeMap<String, String>,
    pub flags: ProcessOverrides,
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
    pub sync: bool,
}
#[derive(Debug, Clone)]
pub struct PipelineSettings {
    pub components: components::Components,
    pub selection: SelectionSettings,
    pub knowledge_policy: LearnerKnowledgePolicy,
    pub model: String,
    pub options: components::Options,
    pub embedding_provider: Option<EmbeddingProvider>,
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
    #[error(transparent)]
    Options(#[from] components::OptionError),
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
            &mut file.application.inventory,
            &mut file.application.wanikani_cache,
            &mut file.application.dictionary_dir,
            &mut file.application.embedding_cache,
        ]
        .into_iter()
        .flatten()
        {
            if value.is_relative() {
                *value = parent.join(&*value);
            }
        }
        let mut env = environment_settings(input.environment, operation)?;
        if operation.uses_setting("enable") {
            for source in [&mut env, &mut input.flags] {
                for id in source.enable.iter().chain(&source.disable) {
                    let setting = match id {
                        ModuleId::Sync => &mut source.application.sync,
                        ModuleId::Embeddings => {
                            &mut source.invocation.pipeline.selection.embeddings
                        }
                        ModuleId::Assessment => &mut source.invocation.pipeline.assessment.enabled,
                        ModuleId::Knowledge | ModuleId::Generation => {
                            return Err(ConfigError::InvalidSetting(
                                "Only sync, embeddings, and assessment can be enabled or disabled.",
                            ));
                        }
                    };
                    if source.enable.contains(id) && source.disable.contains(id) {
                        return Err(ConfigError::ConflictingControl {
                            module: id.metadata().name,
                        });
                    }
                    *setting = Some(source.enable.contains(id));
                }
            }
        }
        macro_rules! setting {
            ($field:ident) => {
                input
                    .flags
                    .application
                    .$field
                    .take()
                    .or(env.application.$field.take())
                    .or(file.application.$field.take())
            };
        }
        let topic_conflict = !matches!(input.flags.invocation.story.topic, Patch::Inherit)
            || !matches!(env.invocation.story.topic, Patch::Inherit);
        let request = input.flags.request.or(env.request);
        let dictionary_dir = setting!(dictionary_dir);
        let mut options = components::Options::default();
        for setting in components::settings().filter(|s| s.secret().is_none()) {
            if operation.uses_setting(&options::path(setting)) {
                options.clear(setting)?;
            }
        }
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
                sync: if operation.uses_setting("application.sync") {
                    setting!(sync)
                } else {
                    None
                }
                .unwrap_or(ModuleId::Sync.metadata().default_enabled),
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
                options,
                embedding_provider: None,
                embeddings: ModuleId::Embeddings.metadata().default_enabled,
                assessment: ModuleId::Assessment.metadata().default_enabled,
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
        for source in [file.invocation, env.invocation, input.flags.invocation] {
            resolved.apply(source, operation)?;
        }
        resolved.validate(operation, topic_conflict)?;
        Ok(resolved)
    }
    pub fn generation(&self) -> Result<StoryGenerationOptions, ConfigError> {
        use yomibu_components::openai_story_generation as openai;
        Ok(openai::generation_options(
            self.pipeline.options.for_component(&openai::COMPONENT),
            StoryGenerationOptions {
                model: self.pipeline.model.clone(),
                format: self.story.format,
                candidate_count: self.story.candidates,
            },
        )?)
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
) -> Result<ProcessOverrides, ConfigError> {
    let mut input = ProcessOverrides::default();
    for (name, text) in environment {
        if let Some(setting) =
            components::settings().find(|s| s.secret().is_none() && s.name().environment() == name)
        {
            if operation.uses_setting(&options::path(setting)) {
                let value = setting
                    .parse(&text)
                    .map_err(|_| ConfigError::Environment { name: name.clone() })?;
                input
                    .invocation
                    .pipeline
                    .options
                    .insert(setting.name(), Patch::Set(value))
                    .map_err(|_| ConfigError::Environment { name })?;
            }
            continue;
        }
        macro_rules! set {
            ($scope:ident.$($field:ident).+) => {
                set!(concat!(stringify!($scope), $(".", stringify!($field)),+),
                    $scope.$($field).+, text.parse::<_>())
            };
            ($path:expr, $target:expr, $value:expr) => {
                if operation.uses_setting($path) {
                    $target = Some($value.map_err(|_| ConfigError::Environment { name: name.clone() })?).into();
                }
            };
        }
        let application = &mut input.application;
        let pipeline = &mut input.invocation.pipeline;
        let story = &mut input.invocation.story;
        match name.as_str() {
            "YOMIBU_MODEL" => set!(pipeline.model),
            "YOMIBU_INVENTORY" => set!(application.inventory),
            "YOMIBU_WANIKANI_CACHE" => set!(application.wanikani_cache),
            "YOMIBU_KNOWLEDGE_POLICY" => set!(pipeline.knowledge_policy),
            "YOMIBU_REQUEST" => set!("request", input.request, text.parse::<PathBuf>()),
            "YOMIBU_TOPIC" => set!(story.topic),
            "YOMIBU_SELECT" => set!(story.select),
            "YOMIBU_SEED" => set!(story.seed),
            "YOMIBU_FORMAT" => set!(story.format),
            "YOMIBU_CANDIDATES" => set!(story.candidates),
            "YOMIBU_CACHE_MAX_AGE_SECONDS" => set!(application.cache_max_age_seconds),
            "YOMIBU_DICTIONARY_DIR" => set!(application.dictionary_dir),
            "YOMIBU_EMBEDDING_CACHE" => set!(application.embedding_cache),
            "YOMIBU_EMBEDDING_PROVIDER" => set!(pipeline.embedding.provider),
            "YOMIBU_ALLOW_EMBEDDING_CALL" => set!(application.allow_embedding_call),
            "YOMIBU_ENABLE" | "YOMIBU_DISABLE" if operation.uses_setting("enable") => {
                let modules = text
                    .split(',')
                    .map(|id| id.trim().parse())
                    .collect::<Result<_, _>>()
                    .map_err(|_| ConfigError::Environment { name: name.clone() })?;
                if name == "YOMIBU_ENABLE" {
                    input.enable = modules;
                } else {
                    input.disable = modules;
                }
            }
            _ => {}
        }
    }
    Ok(input)
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

pub fn environment_names() -> Vec<String> {
    ENVIRONMENT_SETTINGS
        .iter()
        .map(|name| (*name).into())
        .chain(
            components::settings()
                .filter(|s| s.secret().is_none())
                .map(|s| s.name().environment()),
        )
        .collect()
}
