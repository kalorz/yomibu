#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("inventory.json"),
        include_str!("../../../tests/fixtures/story/inventory.json"),
    )
    .unwrap();
    fs::write(
        dir.path().join("request.json"),
        include_str!("../../../tests/fixtures/story/request.json"),
    )
    .unwrap();
    dir
}
fn cli(dir: &Path, command: &str) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    c.env_clear()
        .current_dir(dir)
        .env("YOMIBU_DATA_DIR", dir.join("data"))
        .args([
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
    assert!(missing.status.success());
    assert!(missing.stderr.is_empty());
    assert!(!dir.path().join("vectors.json").exists());
    prepare(dir.path());
    let inventory = yomibu::inventory::LearnerInventory::from_manual(
        serde_json::from_slice(&fs::read(dir.path().join("inventory.json")).unwrap()).unwrap(),
    )
    .unwrap();
    let request =
        serde_json::from_slice(&fs::read(dir.path().join("request.json")).unwrap()).unwrap();
    let cache: yomibu::retrieval::EmbeddingCache =
        serde_json::from_slice(&fs::read(dir.path().join("vectors.json")).unwrap()).unwrap();
    let plan = yomibu::story::plan_generation(
        &inventory,
        &request,
        &cache,
        &cache.model,
        2,
        Default::default(),
    )
    .unwrap();
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
    assert_eq!(report["request"]["topic"], "A cat sleeping");
    assert_eq!(
        report["selection"]["vocabulary_ids"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let body = report["provider_request"]["body_utf8"].as_str().unwrap();
    assert!(!body.contains("いぬ"));
    assert_eq!(body, plan.prepared_request().body_utf8());
    assert_eq!(
        report["provider_request"]["sha256"],
        format!("{:x}", Sha256::digest(body.as_bytes()))
    );
    let text = cli(dir.path(), "preview-story").output().unwrap();
    assert!(text.status.success());
    assert!(String::from_utf8(text.stdout).unwrap().starts_with(
        "Experimental story generation plan — no generation performed\nTopic: A cat sleeping\n"
    ));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), before);
}

#[test]
fn preview_falls_back_for_partial_embedding_settings_and_reports_a_safe_warning() {
    let dir = setup();
    let topic = "日本語\nInjected\u{1b}";
    fs::write(
        dir.path().join("request.json"),
        json!({"version":1,"topic":topic,"targets":{"vocabulary":["cat"],"grammar":[]}})
            .to_string(),
    )
    .unwrap();
    prepare(dir.path());
    let before = fs::read(dir.path().join("vectors.json")).unwrap();
    for json in [false, true] {
        let mut command = cli(dir.path(), "preview-story");
        command.args([
            "--enable",
            "embeddings",
            "--embedding-model",
            "another-model",
        ]);
        if json {
            command.arg("--json");
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.starts_with("warning: Embeddings:"), "{stderr}");
        assert!(stderr.contains("--embedding-provider"));
        assert_eq!(stderr.lines().count(), 1);
        assert!(!stdout.contains('\u{1b}'));
        assert!(!stderr.contains('\u{1b}'));
        assert!(!stdout.contains("\nInjected"));
        assert!(stdout.contains("日本語"));
        if json {
            let report: Value = serde_json::from_str(&stdout).unwrap();
            assert_eq!(report["selection"]["selector_revision"], "builtin-v1");
            assert_eq!(report["request"]["topic"], topic);
            assert_eq!(report["warnings"].as_array().unwrap().len(), 1);
        } else {
            assert!(
                stdout
                    .starts_with("Experimental story generation plan — no generation performed\n")
            );
            assert!(stdout.contains("\nSelector revision: builtin-v1\n"));
            assert!(stdout.ends_with("Generation requests made: 0\n"));
        }
    }
    assert_eq!(fs::read(dir.path().join("vectors.json")).unwrap(), before);
    assert!(!dir.path().join("data").exists());
}
#[test]
fn story_invocation_authorizes_generation_and_rejects_the_removed_opt_in_flag() {
    let dir = setup();
    let o = cli(dir.path(), "story")
        .arg("--allow-model-call")
        .output()
        .unwrap();
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
        vec!["story", "--help"],
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
        let o = cli(dir.path(), "story")
            .args(["--dictionary", "missing.dic"])
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
fn topic_is_data_and_terminal_controls_are_escaped_in_json_and_text() {
    let dir = setup();
    let hostile = "日本語\nInjected\u{1b}\r\t\u{202e}";
    let r = json!({"version":1,"topic":hostile,"targets":{"vocabulary":["cat"],"grammar":[]}});
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
            assert_eq!(v["request"]["topic"], hostile);
        } else {
            assert!(!text.contains("\nInjected"));
        }
    }
}

#[test]
fn obsolete_generation_commands_are_removed_without_aliases() {
    for old in [
        "generate-story",
        "generate-candidates",
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
        serde_json::from_str(include_str!("../../../tests/fixtures/story/request.json")).unwrap();
    request["targets"]["vocabulary"] = json!(["cat", "sleep", "dog"]);
    fs::write(dir.path().join("request.json"), request.to_string()).unwrap();
    let out = cli(dir.path(), "story")
        .args(["--openai-api-key", "unused"])
        .args([
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
        json!({"version":1,"topic":"A dog sleeping","targets":{"vocabulary":[],"grammar":[]}});
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
        let out = cli(dir.path(), "story")
            .args([
                "--candidates",
                &count,
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
                "output budget"
            }),
            "{error}"
        );
        assert!(!error.contains("Dictionary initialization"));
    }
}

#[tokio::test]
async fn oversized_combined_embedding_document_fails_before_any_provider_call() {
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
    let dir = setup();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(|request: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let rows: Vec<_> = (0..body["input"].as_array().unwrap().len())
                .map(|i| json!({"index":i,"embedding":[1., 0.]}))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({"model":"test-model","data":rows}))
        })
        .mount(&server)
        .await;
    let mut inventory: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap();
    for i in 0..33 {
        inventory["vocabulary"].as_array_mut().unwrap().push(json!({
            "id":format!("extra-{i}"),"written_form":format!("word-{i}"),
            "readings":["ねこ"],"meanings":["cat"],"direct_object":null
        }));
    }
    inventory["vocabulary"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["meanings"] = json!(vec!["x".repeat(1024); 32]);
    fs::write(dir.path().join("inventory.json"), inventory.to_string()).unwrap();
    let endpoint = format!("{}/v1/", server.uri());
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
    let out = tokio::task::spawn_blocking(move || c.output().unwrap())
        .await
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(server.received_requests().await.unwrap().is_empty());
    assert!(!dir.path().join("vectors.json").exists());
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("32768-byte")
    );
}

#[test]
fn generation_credentials_are_explicit_after_local_preflight_without_discovery_or_writes() {
    let dir = setup();
    prepare(dir.path());
    fs::write(dir.path().join(".env"), "OPENAI_API_KEY=synthetic-unused").unwrap();
    let before = fs::read_dir(dir.path()).unwrap().count();
    let dictionary = test_dictionary::bundle().join("system_core.dic");
    for key in [None, Some("synthetic-secret\nInjected")] {
        let mut command = cli(dir.path(), "story");
        command
            .args(["--dictionary"])
            .arg(&dictionary)
            .args(["--data-dir", "ignored"]);
        if let Some(key) = key {
            command.env("YOMIBU_OPENAI_API_KEY", key);
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(if key.is_none() {
                "YOMIBU_OPENAI_API_KEY"
            } else {
                "credential"
            }),
            "{stderr}"
        );
        assert!(!stderr.contains("synthetic-secret"));
        assert!(!stderr.contains("synthetic-unused"));
        assert!(!stderr.contains("Injected"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), before);
    }
}
