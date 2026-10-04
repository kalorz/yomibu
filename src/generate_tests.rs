//! Subprocess checks use the production CLI entry with a concrete loopback client.
//! Only this test module reads the G1_TEST_* variables; none ship in the executable.
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output},
};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu::adapters::openai::Client;

const INPUT: &str = include_str!("../tests/fixtures/generation/dog-cat.json");
const HOSTILE: &str = "猫\u{1b}[31m\r\n\t\u{007f}\u{009b}\u{2028}\u{2029}\u{202e}\u{e0001}";

#[test]
fn cli_child() {
    let Ok(base) = std::env::var("G1_TEST_BASE_URL") else {
        return;
    };
    let args: Vec<String> = serde_json::from_str(&std::env::var("G1_TEST_ARGS").unwrap()).unwrap();
    let code = super::entry(args, |key| Client::with_base_url(key, &base));
    std::process::exit(i32::from(code));
}

async fn child(server: &MockServer, input: &Value, json_output: bool) -> Output {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("input.json"), input.to_string()).unwrap();
    let dictionary =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a1/current/system_core.dic");
    let mut args = vec![
        "hostile\n\u{1b}executable".to_owned(),
        "generate-candidates".into(),
        "--allow-model-call".into(),
        "--dictionary".into(),
        dictionary.to_str().unwrap().into(),
        "--input".into(),
        "input.json".into(),
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
            "generate_tests::cli_child",
            "--nocapture",
            "--quiet",
        ])
        .env("G1_TEST_BASE_URL", format!("{}/v1/", server.uri()))
        .env("G1_TEST_ARGS", serde_json::to_string(&args).unwrap())
        .env("OPENAI_API_KEY", "synthetic-cli-secret")
        .env("HTTPS_PROXY", "http://127.0.0.1:1");
    let mut output = tokio::task::spawn_blocking(move || command.output().unwrap())
        .await
        .unwrap();
    // libtest's own prefix is outside the shared CLI entry under test.
    let prefix = b"\nrunning 1 test\n";
    assert!(
        output.stdout.starts_with(prefix),
        "{:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    output.stdout.drain(..prefix.len());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    output
}

fn envelope(pair: [&str; 2]) -> Value {
    json!({"id":"resp_synthetic","model":HOSTILE,"status":"completed","service_tier":"default",
        "output":[{"type":"message","role":"assistant","status":"completed","content":[
            {"type":"output_text","text":json!({"candidates":pair}).to_string()}]}]})
}

fn assert_safe(text: &str) {
    for control in [
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
        assert!(!text.contains(control), "raw {control:?}: {text:?}");
    }
    assert!(!text.contains("synthetic-cli-secret"));
}

#[tokio::test]
async fn executable_reports_completed_and_partial_results_safely() {
    let server = MockServer::start().await;
    let mut input: Value = serde_json::from_str(INPUT).unwrap();
    input["grammar"]
        .as_array_mut()
        .unwrap()
        .push(json!(HOSTILE));
    input["grammar"]
        .as_array_mut()
        .unwrap()
        .push(json!(HOSTILE));
    for (pair, json_output, exit) in [
        (["犬です。", "猫です。"], true, 0),
        (["犬です。", "猫です。"], false, 0),
        ([" \n", "猫です。"], true, 1),
        ([HOSTILE, " \n"], false, 1),
    ] {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(envelope(pair)))
            .expect(1)
            .mount(&server)
            .await;
        let output = child(&server, &input, json_output).await;
        assert_eq!(
            output.status.code(),
            Some(exit),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_safe(&stdout);
        assert_safe(&stderr);
        if exit == 0 {
            assert!(stderr.is_empty());
        } else {
            assert_eq!(
                stderr,
                "error: One or both candidate executions failed; see the experimental report.\n"
            );
        }
        if json_output {
            let report: Value = serde_json::from_str(&stdout).unwrap();
            assert_eq!(report["version"], 1);
            assert_eq!(report["kind"], "experimental_sentence_candidates");
            assert_eq!(report["input"], input);
            assert_eq!(report["generation"]["returned_model"], HOSTILE);
            assert!(report["generation"]["usage"].is_null());
            for (index, text) in pair.iter().enumerate() {
                let candidate = &report["candidates"][index];
                assert_eq!(candidate["index"], index + 1);
                assert_eq!(candidate["text"], *text);
                if text.trim().is_empty() {
                    assert!(candidate["analysis"].is_null());
                    assert_eq!(candidate["assessment"]["status"], "execution_error");
                    assert_eq!(candidate["assessment"]["stage"], "sentence");
                    assert_eq!(candidate["assessment"]["code"], "blank_sentence");
                    assert!(candidate["assessment"].get("outcome").is_none());
                    let checks = candidate["assessment"]["checks"].as_array().unwrap();
                    assert_eq!(checks.len(), 5);
                    assert!(checks.iter().all(|check| check["state"] == "NotRun"));
                } else {
                    assert_eq!(candidate["analysis"]["sentence"], *text);
                    assert_eq!(candidate["assessment"]["status"], "completed");
                    assert_eq!(
                        candidate["assessment"]["outcome"],
                        json!({"Completed":"Pass"})
                    );
                    assert_eq!(
                        candidate["assessment"]["evaluation"]["unassessed"]
                            .as_array()
                            .unwrap()
                            .len(),
                        3
                    );
                }
            }
        } else {
            assert!(
                stdout.starts_with("Experimental sentence candidates — not accepted exercises\n")
            );
            assert!(stdout.contains("Candidate 1:"));
            assert!(stdout.contains("Candidate 2:"));
            assert!(stdout.contains("猫"));
            assert!(stdout.contains("Naturalness: not assessed\n"));
            assert!(stdout.contains("Provider-reported model:"));
            assert!(stdout.contains("Dictionary SHA-256:"));
            if exit == 0 {
                assert!(stdout.contains("Vocabulary: Pass\n  Coverage:"));
                assert!(stdout.contains("Overall: Pass (completed)\n"));
            } else {
                assert!(stdout.contains("Scope: NotRun\n"));
                assert!(stdout.contains("Execution error (sentence/blank_sentence):"));
            }
        }
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn executable_whole_response_failure_has_no_stdout_and_no_retry() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("synthetic-cli-secret\n\u{1b}"))
        .expect(1)
        .mount(&server)
        .await;
    let input: Value = serde_json::from_str(INPUT).unwrap();
    let output = child(&server, &input, true).await;
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "error: Invalid OpenAI response; no candidates were extracted.\n"
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn executable_object_failures_keep_original_spans_alongside_uncertainty() {
    let server = MockServer::start().await;
    let mut input: Value =
        serde_json::from_str(include_str!("../tests/fixtures/analyze/object.json")).unwrap();
    input.as_object_mut().unwrap().remove("sentence");
    input["bindings"]["grammar"] = json!([]);
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(envelope(["犬は水を飲みます。", "犬は水を飲みます。"])),
        )
        .expect(1)
        .mount(&server)
        .await;
    let output = child(&server, &input, true).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    for candidate in report["candidates"].as_array().unwrap() {
        let assessment = &candidate["assessment"];
        assert_eq!(assessment["outcome"], json!({"Completed":"Fail"}));
        let evaluation = &assessment["evaluation"];
        assert_eq!(
            evaluation["scope"]["state"],
            json!({"Completed":"Inconclusive"})
        );
        assert_eq!(
            evaluation["scope"]["findings"][0]["span"],
            json!({"start":6,"end":24})
        );
        let spans: Vec<_> = evaluation["particles"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["span"].clone())
            .collect();
        for span in [
            json!({"start":3,"end":6}),
            json!({"start":9,"end":12}),
            json!({"start":6,"end":24}),
        ] {
            assert!(spans.contains(&span));
        }
        assert_eq!(
            evaluation["inflection"]["findings"][0]["span"],
            json!({"start":18,"end":24})
        );
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[test]
fn typed_downstream_error_reports_keep_available_analysis_without_completed_judgments() {
    use yomibu::{
        adapters::sudachi::AnalysisError,
        analysis::{AnalysisProvenance, Sentence, SentenceAnalysis},
        evaluation::EvaluationError,
        generation::{CandidateAssessment, CandidateError},
    };
    let analysis = SentenceAnalysis {
        sentence: Sentence::new("猫です。").unwrap(),
        units: vec![],
        provenance: AnalysisProvenance {
            analyzer_revision: "synthetic-boundary-test",
            dictionary_version: "synthetic-boundary-test",
            dictionary_sha256: "synthetic-boundary-test",
            configuration_sha256: "synthetic-boundary-test".into(),
        },
    };
    // These are report-boundary cases, not claims about causing real Sudachi failures.
    for (assessment, stage, code, has_analysis) in [
        (
            CandidateAssessment::ExecutionError {
                analysis: None,
                error: CandidateError::Analysis(AnalysisError::InvalidSpan),
            },
            "analysis",
            "invalid_analysis_span",
            false,
        ),
        (
            CandidateAssessment::ExecutionError {
                analysis: Some(analysis),
                error: CandidateError::Evaluation(EvaluationError::InvalidAnalysis),
            },
            "evaluation",
            "invalid_analysis",
            true,
        ),
    ] {
        let report = super::generate::CandidateReport::new(1, "猫です。", &assessment);
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(!json["analysis"].is_null(), has_analysis);
        assert_eq!(json["assessment"]["status"], "execution_error");
        assert_eq!(json["assessment"]["stage"], stage);
        assert_eq!(json["assessment"]["code"], code);
        assert!(json["assessment"].get("outcome").is_none());
        assert!(json["assessment"].get("evaluation").is_none());
        let kinds: Vec<_> = json["assessment"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|check| {
                assert_eq!(check["state"], "NotRun");
                check["kind"].as_str().unwrap()
            })
            .collect();
        assert_eq!(
            kinds,
            ["Vocabulary", "Inflection", "Particles", "Nominal", "Scope"]
        );
    }
}
