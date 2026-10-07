//! Environment and runtime composition for the existing sync/status use cases.
use crate::output::status::write_status;
use anyhow::anyhow;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use yomibu::{
    App, adapters::sources::wanikani::Client, adapters::stores::FileLearningStore, app::SyncReport,
};

pub(super) fn sync(data_dir: Option<PathBuf>, out: &mut impl Write) -> anyhow::Result<()> {
    let data_dir = resolve_data_dir(data_dir)?;
    let token = std::env::var("WANIKANI_API_TOKEN")
        .ok()
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| {
            anyhow!("Set WANIKANI_API_TOKEN in the environment before running yomibu sync.")
        })?;
    let report = synchronize(&data_dir, Client::new(&token)?)?;
    write_status(out, &report.summary)?;
    Ok(())
}

pub(super) fn status(data_dir: Option<PathBuf>, out: &mut impl Write) -> anyhow::Result<()> {
    let data_dir = resolve_data_dir(data_dir)?;
    let summary = App::new(FileLearningStore::new(&data_dir)).status()?;
    write_status(out, &summary)?;
    Ok(())
}

fn resolve_data_dir(data_dir: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    data_dir
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(|home| PathBuf::from(home).join(".yomibu"))
        })
        .ok_or_else(|| anyhow!("HOME is unavailable; specify --data-dir PATH."))
}

fn synchronize(data_dir: &Path, client: Client) -> anyhow::Result<SyncReport> {
    let mut app = App::new(FileLearningStore::new(data_dir)).with_source(client);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(runtime.block_on(app.sync())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};
    use yomibu::adapters::stores::file::cache;

    #[test]
    fn composes_sync_under_lock_and_renders_the_persisted_sync_data() {
        let setup = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let server = setup.block_on(MockServer::start());
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = dir.path().to_path_buf();
        setup.block_on(async {
            Mock::given(path("/v2/user")).respond_with(move |_: &wiremock::Request| {
                assert!(matches!(cache::SyncGuard::acquire(&lock_dir), Err(cache::WriteError::Locked)));
                ResponseTemplate::new(200).set_body_raw(include_str!("../../../../tests/fixtures/wanikani/user.json"), "application/json")
            }).expect(1).mount(&server).await;
            for endpoint in ["assignments", "review_statistics"] {
                Mock::given(path(format!("/v2/{endpoint}"))).respond_with(ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"object":"collection", "pages":{"next_url":null}, "data":[]})
                )).expect(1).mount(&server).await;
            }
        });
        let client =
            Client::with_base_url("synthetic-cli-credential", &format!("{}/v2/", server.uri()))
                .unwrap();
        let report = synchronize(dir.path(), client).unwrap();
        assert_eq!(
            report.summary,
            cache::load(dir.path()).unwrap().summarize().unwrap()
        );
        let mut output = Vec::new();
        write_status(&mut output, &report.summary).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("User: テスト (level 5)"));
        assert!(text.contains("Meaning accuracy: no reviews"));
        assert!(cache::SyncGuard::acquire(dir.path()).is_ok());
    }
}
