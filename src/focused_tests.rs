//! Real executable composition with an explicit loopback client; no production
//! endpoint override or environment discovery is added to the shipped binary.
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output},
};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu::adapters::openai::Client;
const HOSTILE: &str = "猫\u{1b}\r\n\t\u{7f}\u{9b}\u{2028}\u{2029}\u{202e}\u{e0001}";
#[test]
fn cli_child() {
    let Ok(base) = std::env::var("G2_TEST_BASE_URL") else {
        return;
    };
    let args: Vec<String> = serde_json::from_str(&std::env::var("G2_TEST_ARGS").unwrap()).unwrap();
    std::process::exit(i32::from(super::entry(args, |key| {
        Client::with_base_url(key, &base)
    })));
}
async fn child(server: &MockServer, input: &Value, json_output: bool) -> Output {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("permissions.json"), input.to_string()).unwrap();
    for file in [".env", "sudachi.json", "wanikani.json"] {
        std::fs::write(dir.path().join(file), "poisoned ambient file").unwrap();
    }
    let dictionary =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a1/current/system_core.dic");
    let mut args = vec![
        "untrusted\n\u{1b}executable".to_owned(),
        "generate-focused".into(),
        "--allow-model-call".into(),
        "--permissions".into(),
        "permissions.json".into(),
        "--focus-entry".into(),
        "1".into(),
        "--dictionary".into(),
        dictionary.to_str().unwrap().into(),
        "--data-dir".into(),
        "ignored".into(),
    ];
    if json_output {
        args.push("--json".into());
    }
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .current_dir(dir.path())
        .args([
            "--exact",
            "focused_tests::cli_child",
            "--nocapture",
            "--quiet",
        ])
        .env("G2_TEST_BASE_URL", format!("{}/v1/", server.uri()))
        .env("G2_TEST_ARGS", serde_json::to_string(&args).unwrap())
        .env("OPENAI_API_KEY", "synthetic-secret")
        .env("HTTPS_PROXY", "http://127.0.0.1:1");
    let mut output = tokio::task::spawn_blocking(move || command.output().unwrap())
        .await
        .unwrap();
    let prefix = b"\nrunning 1 test\n";
    assert!(output.stdout.starts_with(prefix));
    output.stdout.drain(..prefix.len());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 4);
    output
}
fn envelope(pair: [&str; 2]) -> Value {
    json!({"id":"synthetic","model":HOSTILE,"status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":json!({"candidates":pair}).to_string()}]}]})
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
async fn full_executable_reports_both_original_candidates_and_all_evidence_safely() {
    let server = MockServer::start().await;
    let mut input: Value =
        serde_json::from_str(include_str!("../tests/fixtures/focused/pet-rest.json")).unwrap();
    input["grammar"]
        .as_array_mut()
        .unwrap()
        .push(json!(HOSTILE));
    input["bindings"]["vocabulary"][3]["sense"] = json!("unselected-inventory-sentinel");
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
        let o = child(&server, &input, json_output).await;
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
        assert!(!stdout.contains("unselected-inventory-sentinel"));
        if exit == 0 {
            assert!(stderr.is_empty());
        } else {
            assert_eq!(
                stderr,
                "error: One or both candidate executions failed; see the experimental report.\n"
            );
        }
        let calls = server.received_requests().await.unwrap();
        assert_eq!(calls.len(), 1);
        if json_output {
            let r: Value = serde_json::from_str(&stdout).unwrap();
            assert_eq!(r["kind"], "experimental_focused_sentence_candidates");
            assert!(r.get("input").is_none());
            assert_eq!(r["context"]["grammar"][2], HOSTILE);
            assert_eq!(
                r["context"]["request"]["body_utf8"]
                    .as_str()
                    .unwrap()
                    .as_bytes(),
                calls[0].body
            );
            assert_eq!(r["generation"]["prompt_revision"], "g2-focused-sentence-v1");
            for (i, text) in pair.iter().enumerate() {
                let c = &r["candidates"][i];
                assert_eq!(c["text"], *text);
                if text.trim().is_empty() {
                    assert_eq!(c["assessment"]["status"], "execution_error");
                    assert_eq!(c["focus_occurrence"]["status"], "not_run");
                    assert_eq!(c["context_usage"]["status"], "not_run");
                } else {
                    assert_eq!(c["assessment"]["status"], "completed");
                    assert_eq!(c["focus_occurrence"]["status"], "observed");
                }
            }
            if exit == 0 {
                assert_eq!(
                    r["candidates"][0]["context_usage"]["units"][0]["status"],
                    "unselected_permission_evidence"
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
                "Focus occurrence:",
                "Context usage:",
                "Dictionary SHA-256:",
                "Contextual reading and sense: not assessed\n",
            ] {
                assert!(stdout.contains(s), "{stdout}");
            }
            if exit == 0 {
                assert!(stdout.contains("Vocabulary: Fail\n"));
                assert!(stdout.contains("bytes 0..3: \"鳥\" — whole word is not permitted\n"));
            } else {
                assert!(stdout.contains("Scope: NotRun\n"));
            }
        }
    }
}
#[tokio::test]
async fn executable_keeps_observed_focus_and_reports_partial_completeness() {
    let server = MockServer::start().await;
    let input: Value =
        serde_json::from_str(include_str!("../tests/fixtures/focused/pet-rest.json")).unwrap();
    for json_output in [true, false] {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(envelope(["qzxvは寝ます。", "猫は寝ます。"])),
            )
            .expect(1)
            .mount(&server)
            .await;
        let output = child(&server, &input, json_output).await;
        assert_eq!(output.status.code(), Some(0), "{:?}", output);
        assert!(output.stderr.is_empty());
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
        let stdout = String::from_utf8(output.stdout).unwrap();
        safe(&stdout);
        if json_output {
            let report: Value = serde_json::from_str(&stdout).unwrap();
            let candidates = report["candidates"].as_array().unwrap();
            assert_eq!(candidates.len(), 2);
            for (candidate, completeness) in candidates.iter().zip(["partial", "complete"]) {
                assert_eq!(candidate["assessment"]["status"], "completed");
                assert_eq!(candidate["focus_occurrence"]["status"], "observed");
                assert_eq!(candidate["focus_occurrence"]["count"], 1);
                assert_eq!(candidate["focus_occurrence"]["completeness"], completeness);
            }
            assert_eq!(candidates[0]["text"], "qzxvは寝ます。");
            assert_eq!(
                candidates[0]["focus_occurrence"]["occurrences"][0]["span"],
                json!({"start":7,"end":10})
            );
            assert_eq!(
                candidates[0]["focus_occurrence"]["uncertainties"],
                json!([
                    {"span":{"start":0,"end":4},"reason":"out_of_vocabulary"}
                ])
            );
            assert_eq!(
                candidates[0]["context_usage"]["units"][0]["status"],
                "unresolved"
            );
        } else {
            assert!(
                stdout.starts_with("Experimental sentence candidates — not accepted exercises\n")
            );
            assert!(stdout.contains("qzxvは寝ます。"));
            assert!(stdout.contains("猫は寝ます。"));
            assert!(stdout.contains("Focus occurrence: Observed; compatible occurrences: 1\nFocus assessment completeness: Partial\n  bytes 7..10: RegularStem; reading ネ\n  bytes 0..4: OutOfVocabulary\nContext usage: Completed\n"), "{stdout}");
            assert!(stdout.contains("Focus occurrence: Observed; compatible occurrences: 1\nFocus assessment completeness: Complete\n"));
            assert_eq!(
                stdout
                    .matches("Contextual reading and sense: not assessed\n")
                    .count(),
                2
            );
        }
    }
}
#[tokio::test]
async fn executable_preflight_is_zero_requests_and_whole_response_failure_keeps_stdout_empty() {
    let server = MockServer::start().await;
    let input: Value =
        serde_json::from_str(include_str!("../tests/fixtures/focused/pet-rest.json")).unwrap();
    let mut unavailable = input.clone();
    unavailable["bindings"]["grammar"] = json!([]);
    let o = child(&server, &unavailable, true).await;
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        serde_json::from_slice::<Value>(&o.stdout).unwrap()["status"],
        "unavailable"
    );
    assert!(server.received_requests().await.unwrap().is_empty());
    let mut oversize = input.clone();
    oversize["grammar"] = json!(vec!["\u{1b}".repeat(1024); 3]);
    let o = child(&server, &oversize, true).await;
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    assert!(
        String::from_utf8(o.stderr)
            .unwrap()
            .contains("exceeds 16384 bytes")
    );
    assert!(server.received_requests().await.unwrap().is_empty());
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("synthetic-secret\n\u{1b}"))
        .expect(1)
        .mount(&server)
        .await;
    let o = child(&server, &input, true).await;
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    assert_eq!(
        String::from_utf8(o.stderr).unwrap(),
        "error: Invalid OpenAI response; no candidates were extracted.\n"
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}
