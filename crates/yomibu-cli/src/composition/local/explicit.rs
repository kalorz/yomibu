use super::{ApplicationError, LocalApp, SetupIssue};
use yomibu_app::application::sync::SyncReport;
use yomibu_ext::wanikani;

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
        let store = self
            .config
            .pipeline
            .components
            .learning_store
            .open(&self.config.data_dir);
        yomibu_app::application::sync::App::new(store)
            .with_source(&mut source)
            .sync()
            .await
            .map_err(|error| match error {
                yomibu_app::application::sync::SyncError::Source(error) => error.into(),
                yomibu_app::application::sync::SyncError::Write(error) => error.into(),
                yomibu_app::application::sync::SyncError::InvalidData(error) => error.into(),
            })
    }
}
