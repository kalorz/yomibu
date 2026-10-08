use serde_json::{Value, json};
use std::process::{Command, Output};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};
use yomibu::application::ServiceEndpoints;
const HOSTILE: &str = "猫\u{1b}\r\n\t\u{7f}\u{9b}\u{2028}\u{2029}\u{202e}\u{e0001}";

#[test]
fn cli_child() {
    let Ok(base) = std::env::var("STORY_TEST_BASE_URL") else {
        return;
    };
    let args: Vec<String> =
        serde_json::from_str(&std::env::var("STORY_TEST_ARGS").unwrap()).unwrap();
    std::process::exit(i32::from(super::entry(
        args,
        ServiceEndpoints {
            wanikani: format!("{base}/v2/"),
            openai: format!("{base}/v1/"),
        },
    )));
}
async fn child(
    server: &MockServer,
    dir: &std::path::Path,
    options: &[&str],
    wk_key: bool,
) -> Output {
    let mut args = vec![
        "untrusted\n\u{1b}executable".to_owned(),
        "story".into(),
        "--seed".into(),
        "7".into(),
        "--openai-api-key".into(),
        "flag-ai-key".into(),
    ];
    args.extend(options.iter().map(|value| (*value).to_owned()));
    child_with_args(server, dir, args, wk_key).await
}
async fn child_with_args(
    server: &MockServer,
    dir: &std::path::Path,
    args: Vec<String>,
    wk_key: bool,
) -> Output {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .current_dir(dir)
        .env("HOME", dir)
        .args([
            "--exact",
            "story_tests::cli_child",
            "--nocapture",
            "--quiet",
        ])
        .env("STORY_TEST_BASE_URL", server.uri())
        .env("STORY_TEST_ARGS", serde_json::to_string(&args).unwrap())
        .env("YOMIBU_OPENAI_API_KEY", "environment-ai-key");
    if wk_key {
        command.env("YOMIBU_WANIKANI_API_KEY", "synthetic-wk-key");
    }
    let mut output = tokio::task::spawn_blocking(move || command.output().unwrap())
        .await
        .unwrap();
    let prefix = b"\nrunning 1 test\n";
    assert!(output.stdout.starts_with(prefix), "{:?}", output);
    output.stdout.drain(..prefix.len());
    output
}
fn safe(text: &str) {
    for character in [
        '\u{1b}',
        '\r',
        '\t',
        '\u{7f}',
        '\u{9b}',
        '\u{2028}',
        '\u{2029}',
        '\u{202e}',
        '\u{e0001}',
    ] {
        assert!(!text.contains(character), "{text:?}");
    }
    for key in ["flag-ai-key", "environment-ai-key", "synthetic-wk-key"] {
        assert!(!text.contains(key), "{text:?}");
    }
}
fn envelope(sentences: &[&str]) -> Value {
    json!({"id":"synthetic", "model":HOSTILE, "status":"completed", "output":[{"type":"message", "role":"assistant", "status":"completed", "content":[{"type":"output_text", "text":json!({"candidates":[{"sentences":sentences}]}).to_string()}]}]})
}
async fn generation(server: &MockServer, sentences: &[&str]) {
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(envelope(sentences)))
        .expect(1)
        .mount(server)
        .await;
}
async fn source(server: &MockServer) {
    for (endpoint, body) in [
        (
            "user",
            include_str!("../../../tests/fixtures/wanikani/user.json"),
        ),
        (
            "assignments",
            include_str!("../../../tests/fixtures/wanikani/assignments.json"),
        ),
        (
            "review_statistics",
            include_str!("../../../tests/fixtures/wanikani/review_statistics.json"),
        ),
        (
            "subjects",
            include_str!("../../../tests/fixtures/wanikani/subjects.json"),
        ),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("/v2/{endpoint}")))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .expect(1)
            .mount(server)
            .await;
    }
}

#[tokio::test]
async fn fresh_install_generates_once_in_every_presentation_with_identical_requests_and_correct_streams()
 {
    let mut previous_body = None;
    for (json_output, verbose) in [(false, false), (false, true), (true, false), (true, true)] {
        let server = MockServer::start().await;
        source(&server).await;
        generation(&server, &["猫です。", "朝です。", "寝ます。"]).await;
        let dir = tempfile::tempdir().unwrap();
        let mut flags = vec![];
        if json_output {
            flags.push("--json");
        }
        if verbose {
            flags.push("--verbose");
        }
        let output = child(&server, dir.path(), &flags, true).await;
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        safe(&stdout);
        safe(&stderr);
        assert!(dir.path().join(".yomibu/wanikani.json").exists());
        assert!(!dir.path().join(".yomibu/embeddings.json").exists());
        assert!(!dir.path().join(".yomibu/dictionaries").exists());
        if json_output {
            let report: Value = serde_json::from_str(&stdout).unwrap();
            assert_eq!(
                report["generated"]["passages"][0]["text"],
                "猫です。朝です。寝ます。"
            );
            assert!(report["request"].get("topic").is_none());
            assert_eq!(report["generated"]["provenance"]["request_count"], 1);
            assert_eq!(
                report["assessments"][0]["sentences"][0]["assessment"]["assessment"]["status"],
                "not_run"
            );
            assert!(
                report["timings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|timing| timing["elapsed_ms"].as_f64().unwrap() >= 0.)
            );
            assert_eq!(stderr.contains("[Generation] completed in"), verbose);
        } else {
            if verbose {
                assert!(stdout.contains("[Generation] completed in"));
                assert!(stdout.contains("Sudachi assessment: Missing config"));
            } else {
                assert_eq!(stdout, "猫です。朝です。寝ます。\n");
            }
            assert!(stderr.is_empty());
        }
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 5);
        let generated = requests
            .iter()
            .find(|request| request.method.as_str() == "POST")
            .unwrap();
        assert_eq!(
            generated.headers.get("authorization").unwrap(),
            "Bearer flag-ai-key"
        );
        if let Some(body) = &previous_body {
            assert_eq!(body, &generated.body);
        }
        previous_body = Some(generated.body.clone());
    }
}

#[tokio::test]
async fn generated_controls_are_escaped_and_optional_failure_warns_without_losing_text() {
    for json_output in [true, false] {
        let server = MockServer::start().await;
        generation(&server, &[HOSTILE, "猫です。", "寝ます。"]).await;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("inventory.json"),
            include_bytes!("../../../tests/fixtures/story/inventory.json"),
        )
        .unwrap();
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(dir.path().join("incomplete"))
            .unwrap();
        let mut flags = vec![
            "--inventory",
            "inventory.json",
            "--dictionary-dir",
            "incomplete",
        ];
        if json_output {
            flags.push("--json");
        }
        let output = child(&server, dir.path(), &flags, false).await;
        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        safe(&stdout);
        safe(&stderr);
        assert!(stderr.starts_with("warning: Sudachi assessment:"));
        assert!(stdout.contains("猫"));
        assert!(!stdout.contains("\n\t"));
        if json_output {
            let report: Value = serde_json::from_str(&stdout).unwrap();
            assert_eq!(
                report["generated"]["passages"][0]["text"],
                format!("{HOSTILE}猫です。寝ます。")
            );
        }
    }
}

#[tokio::test]
async fn sentence_assessment_with_a_real_dictionary_reports_available_evidence_and_keeps_generation_model()
 {
    let server = MockServer::start().await;
    generation(&server, &["猫は寝ます。"]).await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("inventory.json"),
        include_bytes!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    let dictionary = crate::test_dictionary::installation();
    let output = child(
        &server,
        dir.path(),
        &[
            "--inventory",
            "inventory.json",
            "--dictionary-dir",
            dictionary.to_str().unwrap(),
            "--format",
            "sentence",
            "--generation-model",
            "chosen-model",
            "--json",
        ],
        false,
    )
    .await;
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["generated"]["provenance"]["requested_model"],
        "chosen-model"
    );
    assert_eq!(
        report["assessments"][0]["sentences"][0]["assessment"]["assessment"]["status"],
        "completed"
    );
    assert_eq!(
        report["assessments"][0]["sentences"][0]["span"],
        json!({"start":0,"end":18})
    );
}

#[tokio::test]
async fn provider_errors_and_invalid_passages_make_one_attempt_and_do_not_reflect_response_bodies()
{
    for sentences in [
        None,
        Some(vec!["猫です。", "寝ます。"]),
        Some(vec!["", "猫です。", "寝ます。"]),
    ] {
        let server = MockServer::start().await;
        let response = match sentences {
            Some(sentences) => ResponseTemplate::new(200).set_body_json(envelope(&sentences)),
            None => {
                ResponseTemplate::new(401).set_body_string("environment-ai-key\u{1b}\nInjected")
            }
        };
        Mock::given(method("POST"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("inventory.json"),
            include_bytes!("../../../tests/fixtures/story/inventory.json"),
        )
        .unwrap();
        let output = child(
            &server,
            dir.path(),
            &["--inventory", "inventory.json", "--json"],
            false,
        )
        .await;
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        safe(&stderr);
        assert!(stderr.starts_with("error:"));
    }
}

#[tokio::test]
async fn explicit_sync_json_contains_one_complete_publication_report() {
    let server = MockServer::start().await;
    source(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let output = child_with_args(
        &server,
        dir.path(),
        vec!["yomibu".into(), "sync".into(), "--json".into()],
        true,
    )
    .await;
    assert_eq!(output.status.code(), Some(0), "{:?}", output);
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["persistence"], "durable");
    assert_eq!(report["summary"]["username"], "テスト");
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
    assert!(dir.path().join(".yomibu/wanikani.json").exists());
}
