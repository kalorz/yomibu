use super::{ApplicationError, LocalApp, SetupIssue};
use crate::application::SyncReport;
use yomibu_components::wanikani_source as wanikani;
use yomibu_core::capabilities::{LearningStore, SourceSyncWriter};

impl LocalApp {
    pub async fn sync(&self) -> Result<SyncReport, ApplicationError> {
        let key = self
            .credentials
            .source()?
            .ok_or_else(|| ApplicationError::Setup {
                issues: vec![SetupIssue::Credential(wanikani::API_KEY)],
            })?;
        let mut source = self
            .config
            .pipeline
            .components
            .source
            .client(key, &self.endpoints.wanikani)?;
        let writer = self
            .config
            .pipeline
            .components
            .learning_store
            .open(&self.config.data_dir)
            .begin_sync()?;
        let data = source.fetch().await?;
        let summary = crate::reports::summary::summarize(&data)?;
        writer.replace(data.into())?;
        Ok(SyncReport { summary })
    }
}
