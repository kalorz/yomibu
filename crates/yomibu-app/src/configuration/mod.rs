mod invocation;
mod patch;
pub use invocation::{StoryOverrides, TargetOverrides};
pub use patch::Patch;
pub use yomibu_core::domain::generation::StoryFormat;
use yomibu_core::domain::generation::{PracticeTargets, StoryError, StoryRequest, StoryTopic};

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
    pub fn request(&self) -> Result<StoryRequest, StoryError> {
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
