pub mod components;
mod documents;
mod invocation;
mod options;
mod patch;
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
    "YOMIBU_SYNC",
];

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
parse_setting!(KnowledgePolicy, "Choose lesson-started or recorded-pass.");

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ProcessOverrides {
    pub application: ApplicationOverrides,
    #[serde(flatten)]
    pub invocation: Invocation,
    #[serde(skip)]
    pub request: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ApplicationOverrides {
    pub inventory: Option<PathBuf>,
    pub wanikani_cache: Option<PathBuf>,
    pub cache_max_age_seconds: Option<u64>,
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
    pub sync: bool,
}
#[derive(Debug, Clone)]
pub struct PipelineSettings {
    pub components: components::Components,
    pub selection: SelectionSettings,
    pub knowledge_policy: LearnerKnowledgePolicy,
    pub model: String,
    pub options: components::Options,
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
    #[error("Invalid configuration at {path}: {error}")]
    ComponentFile {
        path: PathBuf,
        error: components::OptionError,
    },
    #[error("Invalid value for {name}; check its format in yomibu help story.")]
    Environment { name: String },
    #[error("Invalid value for {name}: {error}")]
    ComponentEnvironment {
        name: String,
        error: components::OptionError,
    },
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
        let mut file = documents::load(&path, explicit, operation)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        for value in [
            &mut file.application.inventory,
            &mut file.application.wanikani_cache,
        ]
        .into_iter()
        .flatten()
        {
            if value.is_relative() {
                *value = parent.join(&*value);
            }
        }
        let env = environment_settings(input.environment, operation)?;
        Self::resolve(data_dir, file, env, input.flags, operation)
    }

    /// Resolve built-in defaults and typed overrides without I/O or ambient discovery.
    /// `data_dir` supplies path defaults; it need not exist and is never inspected.
    /// Relative override paths are kept as supplied.
    pub fn from_overrides(
        data_dir: PathBuf,
        overrides: ProcessOverrides,
        operation: &Operation,
    ) -> Result<Self, ConfigError> {
        Self::resolve(
            data_dir,
            ProcessOverrides::default(),
            ProcessOverrides::default(),
            overrides,
            operation,
        )
    }

    fn resolve(
        data_dir: PathBuf,
        mut file: ProcessOverrides,
        mut env: ProcessOverrides,
        mut flags: ProcessOverrides,
        operation: &Operation,
    ) -> Result<Self, ConfigError> {
        macro_rules! setting {
            ($field:ident) => {
                flags
                    .application
                    .$field
                    .take()
                    .or(env.application.$field.take())
                    .or(file.application.$field.take())
            };
        }
        let topic_conflict = !matches!(flags.invocation.story.topic, Patch::Inherit)
            || !matches!(env.invocation.story.topic, Patch::Inherit);
        let request = flags.request.or(env.request);
        let mut resolved = Self {
            application: std::sync::Arc::new(ApplicationSettings {
                inventory: setting!(inventory),
                wanikani_cache: setting!(wanikani_cache),
                cache_max_age: Duration::from_secs(setting!(cache_max_age_seconds).unwrap_or(3600)),
                sync: if operation.uses_setting("application.sync") {
                    setting!(sync)
                } else {
                    None
                }
                .unwrap_or(true),
                data_dir,
            }),
            pipeline: PipelineSettings {
                components: components::Components::default(),
                selection: SelectionSettings {
                    steps: vec![SelectionStep::LexicalTopic, SelectionStep::SeededOrder],
                },
                knowledge_policy: LearnerKnowledgePolicy::default(),
                model: StoryGenerationOptions::default().model,
                options: components::Options::default(),
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
        for source in [file.invocation, env.invocation, flags.invocation] {
            resolved.apply(source, operation)?;
        }
        resolved.validate(operation, topic_conflict)?;
        Ok(resolved)
    }
    pub fn generation(&self) -> Result<StoryGenerationOptions, components::OptionError> {
        use yomibu_components::openai_story_generation as openai;
        openai::generation_options(
            self.pipeline.options.for_component(&openai::COMPONENT),
            StoryGenerationOptions {
                model: self.pipeline.model.clone(),
                format: self.story.format,
                candidate_count: self.story.candidates,
            },
        )
    }
}

fn environment_settings(
    environment: BTreeMap<String, String>,
    operation: &Operation,
) -> Result<ProcessOverrides, ConfigError> {
    let mut input = ProcessOverrides::default();
    for (name, text) in environment {
        if let Some(setting) = components::options().find(|s| s.name().environment() == name) {
            if operation.uses_setting(&options::path(setting)) {
                setting
                    .parse(&text)
                    .and_then(|value| {
                        input
                            .invocation
                            .pipeline
                            .options
                            .insert(setting.name(), Patch::Set(value))
                    })
                    .map_err(|error| ConfigError::ComponentEnvironment { name, error })?;
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
            "YOMIBU_SYNC" => set!(application.sync),
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
}
#[derive(Debug, Clone)]
pub struct SelectionSettings {
    pub steps: Vec<SelectionStep>,
}
impl SelectionSettings {
    fn validate(&self) -> Result<(), ConfigError> {
        use SelectionStep::*;
        if !matches!(
            self.steps.as_slice(),
            [LexicalTopic, SeededOrder] | [SeededOrder]
        ) {
            return Err(ConfigError::InvalidSetting(
                "Selection requires [lexical-topic, seeded-order] or [seeded-order].",
            ));
        }
        Ok(())
    }
}

pub fn environment_names() -> Vec<String> {
    ENVIRONMENT_SETTINGS
        .iter()
        .map(|name| (*name).into())
        .chain(components::options().map(|s| s.name().environment()))
        .collect()
}
