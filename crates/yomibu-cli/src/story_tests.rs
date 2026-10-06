//! Executable tests use the real provider adapter and pinned analyzer, with a
//! loopback-only constructor supplied through the existing test subprocess seam.
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output},
};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu::{
    adapters::{embeddings::LexicalEmbedder, openai::Client},
    inventory::{LearnerInventory, ManualInventory},
    ports::Embedder,
    retrieval::{EmbeddingCache, prepare_embedding_inputs},
    story::StoryRequest,
};
const HOSTILE: &str = "猫\u{1b}\r\n\t\u{7f}\u{9b}\u{2028}\u{2029}\u{202e}\u{e0001}";
#[test]
fn cli_child() {
    let Ok(base) = std::env::var("STORY_TEST_BASE_URL") else {
        return;
    };
    let args: Vec<String> =
        serde_json::from_str(&std::env::var("STORY_TEST_ARGS").unwrap()).unwrap();
    std::process::exit(i32::from(super::entry(args, |key| {
        Client::with_base_url(key, &base)
    })));
}
fn input() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap()
}
fn envelope(pair: impl serde::Serialize) -> Value {
    json!({"id":"synthetic","model":HOSTILE,"status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":json!({"candidates":pair}).to_string()}]}]})
}
async fn child(
    server: &MockServer,
    input: &Value,
    json_output: bool,
    managed: Option<&Path>,
) -> Output {
    child_with_count(server, input, json_output, managed, 2).await
}
async fn child_with_count(
    server: &MockServer,
    input: &Value,
    json_output: bool,
    managed: Option<&Path>,
    count: usize,
) -> Output {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("inventory.json"), input.to_string()).unwrap();
    let request: StoryRequest =
        serde_json::from_str(include_str!("../../../tests/fixtures/story/request.json")).unwrap();
    std::fs::write(
        dir.path().join("request.json"),
        serde_json::to_vec(&request).unwrap(),
    )
    .unwrap();
    let inventory = LearnerInventory::from_manual(
        serde_json::from_value::<ManualInventory>(input.clone()).unwrap(),
    )
    .unwrap();
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    let encoder = LexicalEmbedder::new();
    let cache = EmbeddingCache::from_vectors(
        encoder.model_identity().clone(),
        &inputs,
        encoder.embed(&inputs).await.unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("vectors.json"),
        serde_json::to_vec(&cache).unwrap(),
    )
    .unwrap();
    let dictionary =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/a1/current/system_core.dic");
    let mut args: Vec<String> = vec![
        "untrusted\n\u{1b}executable".into(),
        "generate-story".into(),
        "--allow-model-call".into(),
        "--inventory".into(),
        "inventory.json".into(),
        "--request".into(),
        "request.json".into(),
        "--embedding-cache".into(),
        "vectors.json".into(),
        "--select".into(),
        "2".into(),
        if managed.is_some() {
            "--dictionary-dir"
        } else {
            "--dictionary"
        }
        .into(),
        managed.unwrap_or(&dictionary).to_str().unwrap().into(),
    ];
    args.extend(["--candidates".into(), count.to_string()]);
    if json_output {
        args.push("--json".into());
    }
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .current_dir(dir.path())
        .args([
            "--exact",
            "story_tests::cli_child",
            "--nocapture",
            "--quiet",
        ])
        .env("STORY_TEST_BASE_URL", format!("{}/v1/", server.uri()))
        .env("STORY_TEST_ARGS", serde_json::to_string(&args).unwrap())
        .env("OPENAI_API_KEY", "synthetic-secret")
        .env("HTTPS_PROXY", "http://127.0.0.1:1");
    let mut output = tokio::task::spawn_blocking(move || command.output().unwrap())
        .await
        .unwrap();
    let prefix = b"\nrunning 1 test\n";
    assert!(output.stdout.starts_with(prefix));
    output.stdout.drain(..prefix.len());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 3);
    output
}
fn safe(text: &str) {
    for ch in [
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
        assert!(!text.contains(ch), "{text:?}");
    }
    assert!(!text.contains("synthetic-secret"));
}
#[tokio::test]
async fn executable_keeps_original_texts_partial_results_spans_and_safe_output() {
    let server = MockServer::start().await;
    let mut input = input();
    input["grammar_declarations"][0]["description"] = json!(HOSTILE);
    let long = "猫".repeat(101);
    for (pair, json_output, exit) in [
        (["犬は寝ます。", "鳥は寝ます。"], true, 0),
        (["犬は寝ます。", "鳥は寝ます。"], false, 0),
        ([" \n", "猫は寝ます。"], true, 1),
        ([long.as_str(), "猫は寝ます。"], false, 1),
    ] {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(envelope(pair)))
            .expect(1)
            .mount(&server)
            .await;
        let o = child(&server, &input, json_output, None).await;
        assert_eq!(
            o.status.code(),
            Some(exit),
            "{}",
            String::from_utf8_lossy(&o.stderr)
        );
        let stdout = String::from_utf8(o.stdout).unwrap();
        let stderr = String::from_utf8(o.stderr).unwrap();
        safe(&stdout);
        safe(&stderr);
        if exit == 0 {
            assert!(stderr.is_empty());
        } else {
            assert_eq!(
                stderr,
                "error: One or more candidate executions failed; see the experimental report.\n"
            );
        }
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
        if json_output {
            let r: Value = serde_json::from_str(&stdout).unwrap();
            assert_eq!(r["kind"], "experimental_story_candidates");
            assert_eq!(
                r["plan"]["provider_request"]["body_utf8"]
                    .as_str()
                    .unwrap()
                    .as_bytes(),
                server.received_requests().await.unwrap()[0].body
            );
            assert_eq!(r["generation"]["prompt_revision"], "story-inventory-v1");
            for (i, text) in pair.iter().enumerate() {
                assert_eq!(r["candidates"][i]["text"], *text);
            }
            if exit == 1 {
                assert_eq!(
                    r["candidates"][0]["assessment"]["status"],
                    "execution_error"
                );
                assert_eq!(r["candidates"][0]["targets"][0]["status"], "not_run");
            } else {
                assert_eq!(
                    r["candidates"][0]["plan_departures"][0]["span"],
                    json!({"start":0,"end":3})
                );
                assert_eq!(
                    r["candidates"][1]["assessment"]["evaluation"]["vocabulary"]["findings"][0]["span"],
                    json!({"start":0,"end":3})
                );
            }
        } else {
            assert!(
                stdout.starts_with("Experimental sentence candidates — not accepted exercises\n")
            );
            for s in [
                "Candidate 1:",
                "Candidate 2:",
                "Target vocabulary",
                "completeness complete",
                "Dictionary SHA-256:",
                "Contextual reading and sense: not assessed\n",
            ] {
                assert!(stdout.contains(s), "{stdout}");
            }
            if exit == 1 {
                assert!(stdout.contains("Scope: NotRun\n"));
            } else {
                assert!(stdout.contains(
                    "bytes 0..3: \"鳥\" — whole word is outside the learner inventory\n"
                ));
            }
        }
    }
}
#[tokio::test]
async fn managed_story_uses_same_request_and_real_assessment() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("managed");
    yomibu::adapters::dictionary::import_bundle(
        &root,
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/a1/current"),
    )
    .unwrap();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(envelope(["猫は寝ます。", "猫は寝ます。"])),
        )
        .expect(1)
        .mount(&server)
        .await;
    let output = child(&server, &input(), true, Some(&root)).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let r: Value = serde_json::from_slice(&output.stdout).unwrap();
    for c in r["candidates"].as_array().unwrap() {
        assert_eq!(
            c["analysis"]["provenance"]["dictionary_loading"]["storage"],
            "memory_mapped"
        );
    }
}
#[tokio::test]
async fn request_preflight_is_zero_calls_and_whole_response_failure_has_no_stdout() {
    let server = MockServer::start().await;
    let mut large = input();
    for i in 0..20 {
        large["grammar_declarations"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":format!("extra-{i}"),"description":"\u{1b}".repeat(1024)}));
    }
    let o = child(&server, &large, true, None).await;
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    assert!(server.received_requests().await.unwrap().is_empty());
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("synthetic-secret\n\u{1b}"))
        .expect(1)
        .mount(&server)
        .await;
    let o = child(&server, &input(), true, None).await;
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    assert_eq!(
        String::from_utf8(o.stderr).unwrap(),
        "error: Invalid OpenAI response; no candidates were extracted.\n"
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn configurable_count_renders_every_result_and_preserves_partial_failures() {
    let server = MockServer::start().await;
    for count in [1, 3, 9] {
        for json_output in [true, false] {
            let mut texts = vec!["猫は寝ます。"; count];
            if count > 1 {
                texts[1] = " \n";
            }
            server.reset().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(envelope(&texts)))
                .expect(1)
                .mount(&server)
                .await;
            let out = child_with_count(&server, &input(), json_output, None, count).await;
            assert_eq!(
                out.status.code(),
                Some(if count == 1 { 0 } else { 1 }),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            let stdout = String::from_utf8(out.stdout).unwrap();
            safe(&stdout);
            if json_output {
                let report: Value = serde_json::from_str(&stdout).unwrap();
                assert_eq!(
                    report["plan"]["generation_options"]["candidate_count"],
                    count
                );
                assert!(report["plan"]["request"].get("candidate_count").is_none());
                assert_eq!(report["candidates"].as_array().unwrap().len(), count);
                for (i, text) in texts.iter().enumerate() {
                    assert_eq!(report["candidates"][i]["text"], *text);
                }
            } else {
                assert!(stdout.contains(&format!("Requested candidates: {count}\n")));
                assert_eq!(
                    stdout
                        .lines()
                        .filter(|line| line.starts_with("Candidate "))
                        .count(),
                    count
                );
            }
            if count > 1 {
                assert_eq!(
                    String::from_utf8(out.stderr).unwrap(),
                    "error: One or more candidate executions failed; see the experimental report.\n"
                );
            }
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }
}
