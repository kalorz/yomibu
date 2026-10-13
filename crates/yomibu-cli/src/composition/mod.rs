pub(crate) mod configuration;
mod construction;
pub(crate) mod credential_input;
pub(crate) mod credentials;
pub(crate) mod local;
pub(crate) mod options;
mod selection;
#[cfg(test)]
mod tests;
pub(crate) use credentials::{CredentialError, Credentials, Secret};
pub(crate) use local::{ApplicationError, LocalApp, ServiceEndpoints, SetupIssue};

#[derive(Debug)]
pub(crate) enum Operation {
    Story,
    Sync,
}
impl Operation {
    pub(crate) fn uses_setting(&self, name: &str) -> bool {
        use Operation::*;
        if let Some(option) = name.strip_prefix("pipeline.options.") {
            return matches!(
                option.split_once('.').map(|(component, _)| component),
                Some(yomibu_ext::openai::ID)
            ) && matches!(self, Story);
        }
        match name {
            "pipeline.components.source" => matches!(self, Sync),
            "pipeline.components.learning_store" => matches!(self, Story | Sync),
            _ => matches!(self, Story),
        }
    }
}
