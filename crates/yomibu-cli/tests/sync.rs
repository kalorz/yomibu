use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};
use yomibu::{
    App,
    adapters::stores::FileLearningStore,
    cache::{self, SyncGuard},
    wanikani::Client,
};

const TOKEN: &str = "synthetic-flow-credential";
fn fixture(endpoint: &str) -> Value {
    serde_json::from_str(match endpoint {
        "user" => include_str!("../../../tests/fixtures/wanikani/user.json"),
        "assignments" => include_str!("../../../tests/fixtures/wanikani/assignments.json"),
        "review_statistics" => {
            include_str!("../../../tests/fixtures/wanikani/review_statistics.json")
        }
        "subjects" => include_str!("../../../tests/fixtures/wanikani/subjects.json"),
        _ => panic!("unknown fixture"),
    })
    .unwrap()
}
async fn serve(server: &MockServer, endpoint: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/v2/{endpoint}")))
        .and(header("authorization", format!("Bearer {TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}
async fn refresh(client: &mut Client, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    App::new(FileLearningStore::new(dir))
        .with_source(client)
        .sync()
        .await?;
    Ok(())
}
fn offline_status(dir: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_yomibu"))
        .env_clear()
        .arg("status")
        .arg("--data-dir")
        .arg(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

#[tokio::test]
async fn complete_http_sync_to_disk_to_offline_cli_and_repeat_refresh() {
    let server = MockServer::start().await;
    for endpoint in ["user", "assignments", "review_statistics", "subjects"] {
        serve(&server, endpoint, fixture(endpoint)).await;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut client = Client::with_base_url(TOKEN, &format!("{}/v2/", server.uri())).unwrap();
    refresh(&mut client, dir.path()).await.unwrap();
    let bytes = fs::read(dir.path().join("wanikani.json")).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains(TOKEN));
    assert_eq!(cache::load(dir.path()).unwrap().subjects.len(), 4);
    let actual = offline_status(dir.path());
    for line in include_str!("../../../tests/fixtures/mixed-status.txt")
        .lines()
        .filter(|line| !line.starts_with("Sync "))
    {
        assert!(
            actual.lines().any(|actual| actual == line),
            "missing {line} in {actual}"
        );
    }
    // Status remains usable during a writer's fetch interval.
    let guard = SyncGuard::acquire(dir.path()).unwrap();
    assert_eq!(offline_status(dir.path()), actual);
    drop(guard);

    server.reset().await;
    for endpoint in ["user", "assignments", "review_statistics", "subjects"] {
        let mut body = fixture(endpoint);
        if endpoint == "user" {
            body["data"]["id"] = json!("different-account");
        }
        serve(&server, endpoint, body).await;
    }
    let error = refresh(&mut client, dir.path())
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("another --data-dir PATH"), "{error}");
    assert_eq!(fs::read(dir.path().join("wanikani.json")).unwrap(), bytes);

    server.reset().await;
    serve(&server, "user", fixture("user")).await;
    for endpoint in ["assignments", "review_statistics"] {
        serve(
            &server,
            endpoint,
            json!({"object":"collection", "pages":{"next_url":null}, "data":[]}),
        )
        .await;
    }
    refresh(&mut client, dir.path()).await.unwrap();
    drop(server);
    let sync_data = cache::load(dir.path()).unwrap();
    assert!(
        sync_data.subjects.is_empty()
            && sync_data.assignments.is_empty()
            && sync_data.review_statistics.is_empty()
            && sync_data.unavailable_subjects.is_empty()
    );
    let text = offline_status(dir.path());
    assert!(text.contains("Synchronized kanji: 0"));
    assert!(text.contains("Reading accuracy: no reviews"));
    assert!(!dir.path().join("config.toml").exists());
}

#[tokio::test]
async fn later_page_failures_preserve_complete_cached_observations() {
    use wiremock::matchers::query_param;
    let check_endpoint = async |endpoint: &str| {
        for failure in [
            "authentication",
            "malformed",
            "server",
            "rate",
            "terminator",
            "conflict",
            "cycle",
        ] {
            let server = MockServer::start().await;
            for name in ["user", "assignments", "review_statistics", "subjects"] {
                let mut body = fixture(name);
                if name == endpoint {
                    body["pages"]["next_url"] =
                        json!(format!("{}/v2/{endpoint}?page_after_id=999", server.uri()));
                }
                serve(&server, name, body).await;
            }
            let (response, expected, attempts) = match failure {
                "authentication" => (ResponseTemplate::new(403).set_body_string(TOKEN), "Authentication failed", 1),
                "malformed" => (ResponseTemplate::new(200).set_body_string(format!("{{{TOKEN}")), "Invalid response", 1),
                "server" => (ResponseTemplate::new(503).set_body_string(TOKEN), "HTTP 503", 3),
                "rate" => (ResponseTemplate::new(429).insert_header("ratelimit-reset", "9999999999"), "120 seconds", 1),
                "terminator" => (ResponseTemplate::new(200).set_body_json(json!({"object":"collection", "pages":{}, "data":[]})), "Invalid response", 1),
                "conflict" => {
                    let mut body = fixture(endpoint);
                    body["data"][0]["data_updated_at"] = json!("2026-09-27T10:00:00Z");
                    (ResponseTemplate::new(200).set_body_json(body), "Conflicting duplicate", 1)
                }
                "cycle" => (ResponseTemplate::new(200).set_body_json(json!({"object":"collection", "pages":{"next_url":format!("{}/v2/{endpoint}?page_after_id=999", server.uri())}, "data":[]})), "pagination", 1),
                _ => unreachable!(),
            };
            Mock::given(path(format!("/v2/{endpoint}")))
                .and(query_param("page_after_id", "999"))
                .respond_with(response)
                .expect(attempts)
                .with_priority(1)
                .mount(&server)
                .await;
            let dir = tempfile::tempdir().unwrap();
            let bytes = include_bytes!("../../../tests/fixtures/mixed.json");
            fs::write(dir.path().join("wanikani.json"), bytes).unwrap();
            let old = cache::load(dir.path()).unwrap();
            let before_status = offline_status(dir.path());
            let mut client =
                Client::with_base_url(TOKEN, &format!("{}/v2/", server.uri())).unwrap();
            let error = refresh(&mut client, dir.path()).await.unwrap_err();
            let text = format!("{error} {error:?}");
            assert!(text.contains(expected), "{endpoint}/{failure}: {text}");
            assert!(!text.contains(TOKEN));
            assert_eq!(fs::read(dir.path().join("wanikani.json")).unwrap(), bytes);
            assert_eq!(cache::load(dir.path()).unwrap(), old);
            assert_eq!(offline_status(dir.path()), before_status);
            assert!(SyncGuard::acquire(dir.path()).is_ok());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        }
    };
    tokio::join!(
        check_endpoint("assignments"),
        check_endpoint("review_statistics"),
        check_endpoint("subjects"),
    );
}

#[tokio::test]
async fn access_changes_and_missing_statistics_replace_source_state_without_stale_records() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let mut client = Client::with_base_url(TOKEN, &format!("{}/v2/", server.uri())).unwrap();
    for access in [60, 1, 60] {
        server.reset().await;
        for endpoint in ["user", "assignments", "review_statistics", "subjects"] {
            let mut body = fixture(endpoint);
            if endpoint == "user" {
                body["data"]["subscription"]["max_level_granted"] = json!(access);
            }
            if endpoint == "review_statistics" {
                body["data"] = json!([]);
            }
            if endpoint == "subjects" {
                // Subject 4 was review-only; removal must not leave stale content.
                body["data"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|record| record["id"] != 4);
            }
            serve(&server, endpoint, body).await;
        }
        refresh(&mut client, dir.path()).await.unwrap();
        let sync_data = cache::load(dir.path()).unwrap();
        assert!(sync_data.review_statistics.is_empty());
        assert_eq!(sync_data.assignments.len(), 4);
        assert_eq!(
            sync_data.subjects.len() + sync_data.unavailable_subjects.len(),
            4
        );
        assert!(!sync_data.subjects.iter().any(|s| s.id == 4));
        assert!(sync_data.subjects.iter().all(|s| s.level <= access));
        if access == 60 {
            assert!(sync_data.unavailable_subjects.is_empty());
        } else {
            assert!(!sync_data.unavailable_subjects.is_empty());
        }
        assert!(offline_status(dir.path()).contains("Reading accuracy: no reviews"));
    }
}

#[test]
fn refresh_child() {
    let Some(dir) = std::env::var_os("YOMIBU_TEST_FETCH_DIR") else {
        return;
    };
    let base = std::env::var("YOMIBU_TEST_FETCH_BASE").unwrap();
    let mut client = Client::with_base_url(TOKEN, &base).unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(refresh(&mut client, Path::new(&dir)))
        .unwrap();
}

struct FetchChild(std::process::Child);
impl Drop for FetchChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn killing_a_process_during_retrieval_preserves_cache_and_releases_lock() {
    use std::{os::unix::process::ExitStatusExt, process::Stdio, sync::Arc, time::Duration};
    let server = MockServer::start().await;
    let reached = Arc::new(tokio::sync::Notify::new());
    let notify = Arc::clone(&reached);
    Mock::given(path("/v2/user"))
        .respond_with(move |_: &wiremock::Request| {
            notify.notify_one();
            ResponseTemplate::new(200)
                .set_body_json(fixture("user"))
                .set_delay(Duration::from_secs(60))
        })
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let bytes = include_bytes!("../../../tests/fixtures/mixed.json");
    fs::write(dir.path().join("wanikani.json"), bytes).unwrap();
    let before = offline_status(dir.path());
    let mut child = FetchChild(
        Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .args(["--exact", "refresh_child", "--nocapture"])
            .env("YOMIBU_TEST_FETCH_DIR", dir.path())
            .env("YOMIBU_TEST_FETCH_BASE", format!("{}/v2/", server.uri()))
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    tokio::time::timeout(Duration::from_secs(10), reached.notified())
        .await
        .expect("child never requested user");
    assert!(matches!(
        SyncGuard::acquire(dir.path()),
        Err(cache::WriteError::Locked)
    ));
    assert_eq!(offline_status(dir.path()), before);
    child.0.kill().unwrap();
    assert_eq!(
        child.0.wait().unwrap().signal(),
        Some(9),
        "expected SIGKILL"
    );
    assert_eq!(fs::read(dir.path().join("wanikani.json")).unwrap(), bytes);
    assert_eq!(offline_status(dir.path()), before);
    assert!(SyncGuard::acquire(dir.path()).is_ok());
}
