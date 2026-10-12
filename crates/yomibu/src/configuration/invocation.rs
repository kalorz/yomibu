use super::{
    ConfigError, Configuration, KnowledgePolicy, Operation, Patch, SelectionStep, StoryFormat,
    components,
};
use serde::Deserialize;
use yomibu_core::domain::knowledge::LearnerKnowledgePolicy;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Invocation {
    pub pipeline: PipelineOverrides,
    pub story: StoryOverrides,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PipelineOverrides {
    pub components: ComponentOverrides,
    pub knowledge_policy: Option<KnowledgePolicy>,
    pub model: Option<String>,
    pub selection: SelectionOverrides,
    pub options: super::OptionOverrides,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ComponentOverrides {
    pub source: Option<components::Source>,
    pub learning_store: Option<components::LearningStore>,
    pub preparation: Option<components::Preparation>,
    pub generation: Option<components::Generation>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SelectionOverrides {
    pub steps: Option<Vec<SelectionStep>>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StoryOverrides {
    pub topic: Patch<String>,
    pub seed: Patch<u64>,
    pub select: Option<usize>,
    pub format: Option<StoryFormat>,
    pub candidates: Option<usize>,
    pub targets: TargetOverrides,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TargetOverrides {
    pub vocabulary: Option<Vec<String>>,
    pub grammar: Option<Vec<String>>,
}

impl Configuration {
    pub fn for_invocation(
        &self,
        input: Invocation,
        operation: &Operation,
    ) -> Result<Self, ConfigError> {
        let topic_override = !matches!(input.story.topic, Patch::Inherit);
        let mut resolved = self.clone();
        resolved.apply(input, operation)?;
        resolved.validate(operation, topic_override)?;
        Ok(resolved)
    }
    pub(super) fn apply(
        &mut self,
        input: Invocation,
        operation: &Operation,
    ) -> Result<(), ConfigError> {
        let pipeline = &mut self.pipeline;
        let story = &mut self.story;
        let overrides = input.pipeline;
        macro_rules! set {
            ($input:expr,$target:expr) => {
                if let Some(value) = $input {
                    $target = value;
                }
            };
        }
        macro_rules! component {
            ($field:ident) => {
                if operation.uses_setting(concat!("pipeline.components.", stringify!($field))) {
                    set!(overrides.components.$field, pipeline.components.$field);
                }
            };
        }
        component!(source);
        component!(learning_store);
        component!(preparation);
        component!(generation);
        if operation.uses_setting("pipeline.knowledge_policy")
            && let Some(value) = overrides.knowledge_policy
        {
            pipeline.knowledge_policy = match value {
                KnowledgePolicy::LessonStarted => LearnerKnowledgePolicy::LessonStarted,
                KnowledgePolicy::RecordedPass => LearnerKnowledgePolicy::RecordedPass,
            };
        }
        if operation.uses_setting("pipeline.model") {
            set!(overrides.model, pipeline.model);
            set!(input.story.format, story.format);
            set!(input.story.candidates, story.candidates);
            input.story.seed.apply(&mut story.seed);
        }
        if operation.uses_setting("pipeline.selection.steps") {
            set!(overrides.selection.steps, pipeline.selection.steps);
        }
        overrides.options.apply(&mut pipeline.options, operation)?;
        if operation.uses_setting("story.topic") {
            input.story.topic.apply(&mut story.topic);
            set!(input.story.select, story.select);
            set!(input.story.targets.vocabulary, story.vocabulary_targets);
            set!(input.story.targets.grammar, story.grammar_targets);
        }
        Ok(())
    }
    pub(super) fn validate(
        &self,
        operation: &Operation,
        topic_override: bool,
    ) -> Result<(), ConfigError> {
        if operation.uses_setting("pipeline.model") {
            yomibu_components::openai_story_generation::validate_options(&self.generation()?).map_err(|_|ConfigError::InvalidSetting("Choose a nonblank text model and a positive candidate count that fits the output budget."))?;
        }
        if operation.uses_setting("story.select") && !(1..=16).contains(&self.story.select) {
            return Err(ConfigError::InvalidSetting(
                "--select must be between 1 and 16.",
            ));
        }
        if operation.uses_setting("pipeline.selection.steps") {
            self.pipeline.selection.validate()?;
        }
        if operation.uses_setting("story.topic") && topic_override && self.story.request.is_some() {
            return Err(ConfigError::InvalidSetting(
                "--topic and --request conflict; put the topic in the request file.",
            ));
        }
        Ok(())
    }
}
