use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("inventory.json"),
        include_str!("fixtures/story/inventory.json"),
    )
    .unwrap();
    fs::write(
        dir.path().join("request.json"),
        include_str!("fixtures/story/request.json"),
    )
    .unwrap();
    dir
}
fn cli(dir: &Path, command: &str) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    c.env_clear().current_dir(dir).args([
        command,
        "--inventory",
        "inventory.json",
        "--request",
        "request.json",
        "--embedding-cache",
        "vectors.json",
        "--select",
        "2",
    ]);
    c
}
fn prepare(dir: &Path) {
    let o = cli(dir, "prepare-retrieval")
        .args(["--embedding-provider", "lexical-baseline"])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
#[test]
fn offline_preview_has_no_implicit_embedding_call_and_preserves_exact_bytes_and_layout() {
    let dir = setup();
    let missing = cli(dir.path(), "preview-story").output().unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("prepare-retrieval")
    );
    prepare(dir.path());
    for f in [".env", "wanikani.json", "sudachi.json"] {
        fs::write(dir.path().join(f), "poison").unwrap();
    }
    let before = fs::read_dir(dir.path()).unwrap().count();
    let output = cli(dir.path(), "preview-story")
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["kind"], "story_generation_plan_preview");
    assert_eq!(report["request"]["brief"], "A cat sleeping");
    assert_eq!(report["plan"]["selected"].as_array().unwrap().len(), 2);
    let body = report["provider_request"]["body_utf8"].as_str().unwrap();
    assert!(!body.contains("いぬ"));
    assert_eq!(body, include_str!("fixtures/story/provider-request.json"));
    assert_eq!(
        report["provider_request"]["sha256"],
        include_str!("fixtures/story/provider-request.sha256").trim()
    );
    let text = cli(dir.path(), "preview-story").output().unwrap();
    assert!(text.status.success());
    assert!(String::from_utf8(text.stdout).unwrap().starts_with(
        "Experimental story generation plan — no generation performed\nBrief: A cat sleeping\n"
    ));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), before);
}
#[test]
fn new_cli_requires_opt_in_rejects_old_flags_and_keeps_diagnostics_readable() {
    let dir = setup();
    let o = cli(dir.path(), "generate-story").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let e = String::from_utf8(o.stderr).unwrap();
    assert!(e.contains("--allow-model-call"));
    assert!(e.contains("\n\nUsage:"));
    let o = cli(dir.path(), "preview-story")
        .args(["--permissions", "x\n\u{1b}"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(!String::from_utf8(o.stderr).unwrap().contains('\u{1b}'));
    for args in [
        vec!["--help"],
        vec!["generate-story", "--help"],
        vec!["prepare-retrieval", "--help"],
        vec!["--version"],
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_yomibu"))
            .env_clear()
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success());
        assert!(o.stderr.is_empty());
        assert!(!o.stdout.is_empty());
    }
}
#[test]
fn malformed_and_oversized_inputs_fail_before_dictionary_or_credentials() {
    let dir = setup();
    for input in ["{".to_owned(), " ".repeat(4_194_305)] {
        fs::write(dir.path().join("inventory.json"), input).unwrap();
        let o = cli(dir.path(), "generate-story")
            .args(["--allow-model-call", "--dictionary", "missing.dic"])
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(1));
        assert!(o.stdout.is_empty());
        let e = String::from_utf8(o.stderr).unwrap();
        assert!(!e.contains("Dictionary initialization"));
        assert!(!e.contains("OPENAI_API_KEY"));
    }
}
#[test]
fn brief_is_data_and_terminal_controls_are_escaped_in_json_and_text() {
    let dir = setup();
    let hostile = "日本語\nInjected\u{1b}\r\t\u{202e}";
    let r = json!({"version":1,"brief":hostile,"targets":{"vocabulary":["cat"],"grammar":[]}});
    fs::write(dir.path().join("request.json"), r.to_string()).unwrap();
    prepare(dir.path());
    for json in [false, true] {
        let mut c = cli(dir.path(), "preview-story");
        if json {
            c.arg("--json");
        }
        let o = c.output().unwrap();
        assert!(o.status.success());
        assert!(o.stderr.is_empty());
        let text = String::from_utf8(o.stdout).unwrap();
        for ch in ['\u{1b}', '\r', '\t', '\u{202e}'] {
            assert!(!text.contains(ch));
        }
        assert!(text.contains("日本語"));
        if json {
            let v: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(v["request"]["brief"], hostile);
        } else {
            assert!(!text.contains("\nInjected"));
        }
    }
}

#[test]
fn obsolete_focused_commands_are_removed_without_aliases() {
    for old in [
        "generate-focused",
        "context-preview",
        "preview-reading",
        "generate-reading",
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_yomibu"))
            .env_clear()
            .args([old, "--help"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(
            String::from_utf8(out.stderr)
                .unwrap()
                .contains("unrecognized subcommand")
        );
    }
}

#[test]
fn impossible_selection_fails_before_embedding_work() {
    let dir = setup();
    let mut request: Value =
        serde_json::from_str(include_str!("fixtures/story/request.json")).unwrap();
    request["targets"]["vocabulary"] = json!(["cat", "sleep", "dog"]);
    fs::write(dir.path().join("request.json"), request.to_string()).unwrap();
    let out = cli(dir.path(), "generate-story")
        .args([
            "--allow-model-call",
            "--dictionary",
            "missing.dic",
            "--embedding-provider",
            "lexical-baseline",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(!dir.path().join("vectors.json").exists());
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("selection limit")
    );
}

#[tokio::test]
async fn retrieval_reuses_complete_cache_and_preserves_it_on_provider_failure() {
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
    let dir = setup();
    let server = MockServer::start().await;
    let endpoint = format!("{}/v1/", server.uri());
    let command = || {
        let mut c = cli(dir.path(), "prepare-retrieval");
        c.args([
            "--embedding-provider",
            "local",
            "--embedding-model",
            "test-model",
            "--embedding-revision",
            "pinned",
            "--embedding-dimensions",
            "2",
            "--embedding-endpoint",
            &endpoint,
        ]);
        c
    };
    let rows: Vec<_> = (0..5)
        .map(|i| json!({"index":i,"embedding":[1., 0.]}))
        .collect();
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"model":"test-model","data":rows})),
        )
        .expect(1)
        .mount(&server)
        .await;
    for _ in 0..2 {
        let mut c = command();
        let out = tokio::task::spawn_blocking(move || c.output().unwrap())
            .await
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let previous = fs::read(dir.path().join("vectors.json")).unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    server.reset().await;
    let request =
        json!({"version":1,"brief":"A dog sleeping","targets":{"vocabulary":[],"grammar":[]}});
    fs::write(dir.path().join("request.json"), request.to_string()).unwrap();
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).set_body_string("untrusted\n\u{1b}secret"))
        .expect(1)
        .mount(&server)
        .await;
    let mut c = command();
    let out = tokio::task::spawn_blocking(move || c.output().unwrap())
        .await
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let error = String::from_utf8(out.stderr).unwrap();
    assert!(error.contains("HTTP 429"));
    assert!(!error.contains("secret"));
    assert_eq!(fs::read(dir.path().join("vectors.json")).unwrap(), previous);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["input"], json!(["A dog sleeping"]));
}

#[test]
fn invalid_candidate_count_fails_before_any_embedding_or_dictionary_work() {
    let dir = setup();
    for (count, status) in [("0".to_owned(), 2), (usize::MAX.to_string(), 1)] {
        let out = cli(dir.path(), "generate-story")
            .args([
                "--candidates",
                &count,
                "--allow-model-call",
                "--dictionary",
                "missing.dic",
                "--embedding-provider",
                "lexical-baseline",
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(status));
        assert!(out.stdout.is_empty());
        assert!(!dir.path().join("vectors.json").exists());
        let error = String::from_utf8(out.stderr).unwrap();
        assert!(
            error.contains(if status == 2 {
                "--candidates"
            } else {
                "output token budget"
            }),
            "{error}"
        );
        assert!(!error.contains("Dictionary initialization"));
    }
}
