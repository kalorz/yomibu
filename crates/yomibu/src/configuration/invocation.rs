use super::{
    ConfigError, Configuration, EmbeddingProvider, KnowledgePolicy, Operation, Patch,
    SelectionStep, StoryFormat, components,
};
use serde::Deserialize;

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
    pub options: ComponentOptions,
    pub embedding: EmbeddingOverrides,
    pub assessment: AssessmentOverrides,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ComponentOverrides {
    pub source: Option<components::Source>,
    pub learning_store: Option<components::LearningStore>,
    pub embedding_cache: Option<components::EmbeddingCache>,
    pub preparation: Option<components::Preparation>,
    pub generation: Option<components::Generation>,
    pub analysis: Option<components::Analysis>,
    pub assessment: Option<components::Assessment>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SelectionOverrides {
    pub steps: Option<Vec<SelectionStep>>,
    pub embedding_steps: Option<Vec<SelectionStep>>,
    pub embeddings: Option<bool>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ComponentOptions {
    #[serde(rename = "openai-story-generation")]
    pub generation: GenerationOptions,
    #[serde(rename = "http-embeddings")]
    pub embeddings: EmbeddingOptions,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GenerationOptions {
    pub model: Patch<String>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EmbeddingOptions {
    pub model: Patch<String>,
    pub revision: Patch<String>,
    pub dimensions: Patch<usize>,
    pub endpoint: Patch<String>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EmbeddingOverrides {
    pub provider: Patch<EmbeddingProvider>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AssessmentOverrides {
    pub enabled: Option<bool>,
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
        resolved.apply(input, operation);
        resolved.validate(operation, topic_override)?;
        Ok(resolved)
    }
    pub(super) fn apply(&mut self, input: Invocation, operation: &Operation) {
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
            ($field:ident,$setting:literal) => {
                if operation.uses_setting($setting) {
                    set!(overrides.components.$field, pipeline.components.$field);
                }
            };
        }
        component!(source, "source");
        component!(learning_store, "learning_store");
        component!(embedding_cache, "embedding_cache_component");
        component!(preparation, "preparation");
        component!(generation, "generation_component");
        component!(analysis, "analysis");
        component!(assessment, "assessment_component");
        if operation.uses_setting("knowledge_policy")
            && let Some(value) = overrides.knowledge_policy
        {
            pipeline.knowledge_policy.wanikani = match value {
                KnowledgePolicy::LessonStarted => {
                    yomibu_core::domain::knowledge::WaniKaniKnowledgeRule::LessonStarted
                }
                KnowledgePolicy::RecordedPass => {
                    yomibu_core::domain::knowledge::WaniKaniKnowledgeRule::RecordedPass
                }
            };
        }
        if operation.uses_setting("model") {
            set!(overrides.model, pipeline.model);
            overrides
                .options
                .generation
                .model
                .apply(&mut pipeline.generation_model);
            set!(input.story.format, story.format);
            set!(input.story.candidates, story.candidates);
            input.story.seed.apply(&mut story.seed);
        }
        if operation.uses_setting("steps") {
            set!(overrides.selection.steps, pipeline.selection.steps);
            set!(
                overrides.selection.embedding_steps,
                pipeline.selection.embedding_steps
            );
            set!(overrides.selection.embeddings, pipeline.embeddings);
            set!(overrides.assessment.enabled, pipeline.assessment);
        }
        if operation.uses_setting("embedding_provider") {
            overrides
                .embedding
                .provider
                .apply(&mut pipeline.embedding_provider);
            overrides
                .options
                .embeddings
                .model
                .apply(&mut pipeline.embedding_model);
            overrides
                .options
                .embeddings
                .revision
                .apply(&mut pipeline.embedding_revision);
            overrides
                .options
                .embeddings
                .dimensions
                .apply(&mut pipeline.embedding_dimensions);
            match overrides.options.embeddings.endpoint {
                Patch::Inherit => {}
                Patch::Clear => {
                    pipeline.embedding_endpoint = super::DEFAULT_EMBEDDING_ENDPOINT.into()
                }
                Patch::Set(value) => pipeline.embedding_endpoint = value,
            }
        }
        if operation.uses_setting("topic") {
            input.story.topic.apply(&mut story.topic);
            set!(input.story.select, story.select);
            set!(input.story.targets.vocabulary, story.vocabulary_targets);
            set!(input.story.targets.grammar, story.grammar_targets);
        }
    }
    pub(super) fn validate(
        &self,
        operation: &Operation,
        topic_override: bool,
    ) -> Result<(), ConfigError> {
        if operation.uses_setting("model") {
            yomibu_components::openai_story_generation::validate_options(&self.generation()).map_err(|_|ConfigError::InvalidSetting("Choose a nonblank text model and a positive candidate count that fits the output budget."))?;
        }
        if operation.uses_setting("select") && !(1..=16).contains(&self.story.select) {
            return Err(ConfigError::InvalidSetting(
                "--select must be between 1 and 16.",
            ));
        }
        if operation.uses_setting("steps") {
            self.pipeline.selection.validate()?;
        }
        if operation.uses_setting("topic") && topic_override && self.story.request.is_some() {
            return Err(ConfigError::InvalidSetting(
                "--topic and --request conflict; put the topic in the request file.",
            ));
        }
        Ok(())
    }
}

impl super::Settings {
    pub(super) fn into_invocation(self) -> Invocation {
        fn optional<T>(value: Option<T>) -> Patch<T> {
            value.map(Patch::Set).unwrap_or_default()
        }
        Invocation {
            pipeline: PipelineOverrides {
                components: ComponentOverrides {
                    source: self.source,
                    learning_store: self.learning_store,
                    embedding_cache: self.embedding_cache_component,
                    preparation: self.preparation,
                    generation: self.generation_component,
                    analysis: self.analysis,
                    assessment: self.assessment_component,
                },
                knowledge_policy: self.knowledge_policy,
                model: self.model,
                selection: SelectionOverrides {
                    steps: self.steps,
                    embedding_steps: self.embedding_steps,
                    embeddings: self.embeddings,
                },
                options: ComponentOptions {
                    generation: GenerationOptions {
                        model: optional(self.generation_model),
                    },
                    embeddings: EmbeddingOptions {
                        model: optional(self.embedding_model),
                        revision: optional(self.embedding_revision),
                        dimensions: optional(self.embedding_dimensions),
                        endpoint: optional(self.embedding_endpoint),
                    },
                },
                embedding: EmbeddingOverrides {
                    provider: optional(self.embedding_provider),
                },
                assessment: AssessmentOverrides {
                    enabled: self.assessment,
                },
            },
            story: StoryOverrides {
                topic: optional(self.topic),
                seed: optional(self.seed),
                select: self.select,
                format: self.format,
                candidates: self.candidates,
                targets: TargetOverrides {
                    vocabulary: self.vocabulary_targets,
                    grammar: self.grammar_targets,
                },
            },
        }
    }
}
