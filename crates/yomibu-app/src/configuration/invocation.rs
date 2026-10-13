use super::{Patch, StoryFormat, StorySettings};
use serde::Deserialize;

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
}

impl StorySettings {
    pub fn for_invocation(&self, input: StoryOverrides) -> Self {
        let mut settings = self.clone();
        input.seed.apply(&mut settings.seed);
        input.topic.apply(&mut settings.topic);
        if let Some(value) = input.format {
            settings.format = value;
        }
        if let Some(value) = input.candidates {
            settings.candidates = value;
        }
        if let Some(value) = input.select {
            settings.select = value;
        }
        if let Some(value) = input.targets.vocabulary {
            settings.vocabulary_targets = value;
        }
        settings
    }
}
