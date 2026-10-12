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
use std::{collections::BTreeMap, path::PathBuf};
pub use yomibu_core::domain::story::StoryFormat;
use yomibu_core::domain::story::{
    PracticeTargets, StoryError, StoryGenerationOptions, StoryRequest, StoryTopic,
};

pub const ENVIRONMENT_SETTINGS: &[&str] = &[
    "YOMIBU_TOPIC",
    "YOMIBU_SELECT",
    "YOMIBU_SEED",
    "YOMIBU_FORMAT",
    "YOMIBU_CANDIDATES",
];

pub struct ConfigurationInput {
    pub data_dir: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub environment: BTreeMap<String, String>,
    pub flags: Invocation,
}

#[derive(Debug, Clone)]
pub struct Configuration {
    pub data_dir: PathBuf,
    pub pipeline: PipelineSettings,
    pub story: StorySettings,
}
#[derive(Debug, Clone)]
pub struct PipelineSettings {
    pub components: components::Components,
    pub selection: SelectionSettings,
    pub options: components::Options,
}
#[derive(Debug, Clone)]
pub struct StorySettings {
    pub topic: Option<String>,
    pub select: usize,
    pub seed: Option<u64>,
    pub format: StoryFormat,
    pub candidates: usize,
    pub vocabulary_targets: Vec<String>,
}

impl StorySettings {
    pub(crate) fn request(&self) -> Result<StoryRequest, StoryError> {
        let request = StoryRequest {
            version: 1,
            topic: self
                .topic
                .as_ref()
                .map(|text| StoryTopic::new(text.clone()))
                .transpose()?,
            targets: PracticeTargets {
                vocabulary: self.vocabulary_targets.clone(),
                grammar: Vec::new(),
            },
        };
        request.validate_shape()?;
        request.validate_selection_limit(self.select)?;
        Ok(request)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Story(#[from] StoryError),
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
        let file = documents::load(&path, explicit, operation)?;
        let env = environment_settings(input.environment, operation)?;
        Self::resolve(data_dir, file, env, input.flags, operation)
    }

    /// Resolve built-in defaults and typed overrides without I/O or ambient discovery.
    /// `data_dir` supplies path defaults; it need not exist and is never inspected.
    pub fn from_overrides(
        data_dir: PathBuf,
        overrides: Invocation,
        operation: &Operation,
    ) -> Result<Self, ConfigError> {
        Self::resolve(
            data_dir,
            Invocation::default(),
            Invocation::default(),
            overrides,
            operation,
        )
    }

    fn resolve(
        data_dir: PathBuf,
        file: Invocation,
        env: Invocation,
        flags: Invocation,
        operation: &Operation,
    ) -> Result<Self, ConfigError> {
        let mut resolved = Self {
            data_dir,
            pipeline: PipelineSettings {
                components: components::Components::default(),
                selection: SelectionSettings {
                    steps: vec![SelectionStep::LexicalTopic, SelectionStep::SeededOrder],
                },
                options: components::Options::default(),
            },
            story: StorySettings {
                topic: None,
                select: 12,
                seed: None,
                format: StoryFormat::default(),
                candidates: 1,
                vocabulary_targets: Vec::new(),
            },
        };
        for source in [file, env, flags] {
            resolved.apply(source, operation)?;
        }
        resolved.validate(operation)?;
        Ok(resolved)
    }
    pub fn generation(&self) -> Result<StoryGenerationOptions, components::OptionError> {
        use yomibu_components::openai_story_generation as openai;
        openai::generation_options(
            self.pipeline.options.for_component(&openai::COMPONENT),
            StoryGenerationOptions {
                format: self.story.format,
                candidate_count: self.story.candidates,
                ..Default::default()
            },
        )
    }
}

fn environment_settings(
    environment: BTreeMap<String, String>,
    operation: &Operation,
) -> Result<Invocation, ConfigError> {
    let mut input = Invocation::default();
    for (name, text) in environment {
        if let Some(setting) = components::options().find(|s| s.name().environment() == name) {
            if operation.uses_setting(&options::path(setting)) {
                setting
                    .parse(&text)
                    .and_then(|value| {
                        input
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
        let story = &mut input.story;
        match name.as_str() {
            "YOMIBU_TOPIC" => set!(story.topic),
            "YOMIBU_SELECT" => set!(story.select),
            "YOMIBU_SEED" => set!(story.seed),
            "YOMIBU_FORMAT" => set!(story.format),
            "YOMIBU_CANDIDATES" => set!(story.candidates),
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
