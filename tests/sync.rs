use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};
use yomibu::{
    cache::{self, SyncGuard},
    wanikani::Client,
};

const TOKEN: &str = "synthetic-flow-credential";
fn fixture(endpoint: &str) -> Value {
    serde_json::from_str(match endpoint {
        "user" => include_str!("fixtures/wanikani/user.json"),
        "assignments" => include_str!("fixtures/wanikani/assignments.json"),
        "review_statistics" => include_str!("fixtures/wanikani/review_statistics.json"),
        "subjects" => include_str!("fixtures/wanikani/subjects.json"),
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
    let guard = SyncGuard::acquire(dir)?;
    let snapshot = client.fetch().await?;
    guard.replace(&snapshot)?;
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
    for line in include_str!("fixtures/mixed-status.txt")
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
    let snapshot = cache::load(dir.path()).unwrap();
    assert!(
        snapshot.subjects.is_empty()
            && snapshot.assignments.is_empty()
            && snapshot.review_statistics.is_empty()
            && snapshot.unavailable_subjects.is_empty()
    );
    let text = offline_status(dir.path());
    assert!(text.contains("Synchronized kanji: 0"));
    assert!(text.contains("Reading accuracy: no reviews"));
    assert!(!dir.path().join("config.toml").exists());
}
