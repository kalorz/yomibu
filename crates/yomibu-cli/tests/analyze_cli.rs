use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::OnceLock,
};

use serde_json::{Value, json};
use yomibu::{
    adapters::sudachi::SudachiAnalyzer,
    analysis::Sentence,
    evaluation::{CheckKind, EvaluationBindings, evaluate},
    grammar::GrammarDeclarations,
};

const NOMINAL: &str = include_str!("../../../tests/fixtures/analyze/nominal.json");
const UNTRUSTED: &str =
    "犬\u{1b}[31m\r\n\t\u{007f}\u{009b}31m\u{2028}\u{2029}\u{202e}\u{e0001}\\\"";

fn dictionary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/a1/current/system_core.dic")
}

fn analyzer() -> &'static SudachiAnalyzer {
    static ANALYZER: OnceLock<SudachiAnalyzer> = OnceLock::new();
    ANALYZER.get_or_init(|| {
        SudachiAnalyzer::load(dictionary())
            .expect("install the pinned dictionary using scripts/setup_a1_dictionary.py")
    })
}

fn cli(dir: &Path) -> Command {
    cli_with_dictionary(dir, &dictionary())
}

fn cli_with_dictionary(dir: &Path, dictionary: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    command
        .env_clear()
        .current_dir(dir)
        .args(["analyze", "--dictionary"])
        .arg(dictionary)
        .args(["--input", "input.json"]);
    command
}

fn stdout(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn assert_error(output: Output, message: &str) -> String {
    assert!(!output.status.success(), "unexpected success");
    assert!(
        output.stdout.is_empty(),
        "errors must not publish a partial evaluation"
    );
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains(message), "expected {message:?}: {error:?}");
    assert_terminal_safe(&error);
    error
}

fn assert_terminal_safe(text: &str) {
    for character in [
        '\u{1b}',
        '\r',
        '\t',
        '\u{007f}',
        '\u{009b}',
        '\u{2028}',
        '\u{2029}',
        '\u{202e}',
        '\u{e0001}',
    ] {
        assert!(
            !text.contains(character),
            "raw {character:?} in terminal output"
        );
    }
}

fn assert_library_agreement(input: &Value, report: &Value) {
    let sentence = Sentence::new(input["sentence"].as_str().unwrap()).unwrap();
    let grammar = GrammarDeclarations::from_descriptions(
        input["grammar"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap()),
    )
    .unwrap();
    let bindings: EvaluationBindings = serde_json::from_value(input["bindings"].clone()).unwrap();
    let analysis = analyzer().analyze(sentence).unwrap();
    let evaluation = evaluate(&analysis, &grammar, &bindings).unwrap();
    assert_eq!(report["version"], 1);
    assert_eq!(&report["input"], input);
    assert_eq!(report["analysis"], serde_json::to_value(&analysis).unwrap());
    assert_eq!(
        report["evaluation"],
        serde_json::to_value(&evaluation).unwrap()
    );
    assert_eq!(
        report["outcome"],
        serde_json::to_value(evaluation.outcome()).unwrap()
    );
}

#[test]
fn json_matches_direct_library_evaluation_without_credentials_or_writes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("input.json"), NOMINAL).unwrap();
    let input: Value = serde_json::from_str(NOMINAL).unwrap();
    let report: Value =
        serde_json::from_str(&stdout(cli(dir.path()).arg("--json").output().unwrap())).unwrap();
    assert_library_agreement(&input, &report);
    assert_eq!(report["outcome"], json!({"Completed": "Pass"}));
    assert_eq!(report["analysis"]["sentence"], "犬です。");
    assert_eq!(
        report["analysis"]["units"][0]["token"]["span"],
        json!({"start": 0, "end": 3})
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("input.json")).unwrap(),
        NOMINAL
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn text_shows_every_check_reason_and_original_span_for_completed_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    for expected in ["Pass", "Fail", "Inconclusive"] {
        let mut input: Value = serde_json::from_str(NOMINAL).unwrap();
        match expected {
            "Fail" => input["bindings"]["vocabulary"] = json!([]),
            "Inconclusive" => input["sentence"] = json!("犬でした。"),
            _ => {}
        }
        fs::write(dir.path().join("input.json"), input.to_string()).unwrap();
        let text = stdout(cli(dir.path()).output().unwrap());
        assert!(
            text.contains(&format!("Overall: {expected} (completed)")),
            "{text}"
        );
        let original = input["sentence"].as_str().unwrap();
        assert!(
            text.contains(&format!("Sentence: \"{original}\"")),
            "{text}"
        );
        let analysis = analyzer()
            .analyze(Sentence::new(original).unwrap())
            .unwrap();
        let grammar =
            GrammarDeclarations::from_descriptions(["です — manual familiarity"]).unwrap();
        let bindings = serde_json::from_value(input["bindings"].clone()).unwrap();
        let evaluation = evaluate(&analysis, &grammar, &bindings).unwrap();
        for (label, kind) in [
            ("Vocabulary", CheckKind::Vocabulary),
            ("Inflection", CheckKind::Inflection),
            ("Particles", CheckKind::Particles),
            ("Nominal です", CheckKind::Nominal),
            ("Scope", CheckKind::Scope),
        ] {
            let check = evaluation.check(kind);
            let state = serde_json::to_value(check.state).unwrap();
            assert!(
                text.contains(&format!(
                    "{label}: {}",
                    state["Completed"].as_str().unwrap()
                )),
                "{text}"
            );
            assert!(text.contains(check.coverage), "{text}");
            for finding in &check.findings {
                assert!(text.contains(finding.reason), "{text}");
                assert!(
                    text.contains(&format!(
                        "bytes {}..{}: \"{}\"",
                        finding.span.start,
                        finding.span.end,
                        &original[finding.span.clone()]
                    )),
                    "{text}"
                );
            }
        }
        for notice in [
            "not accepted exercises",
            "Naturalness: not assessed",
            "Multiword expressions: not assessed",
            "Contextual reading and sense: not assessed",
        ] {
            assert!(text.contains(notice), "{text}");
        }
        let report: Value =
            serde_json::from_str(&stdout(cli(dir.path()).arg("--json").output().unwrap())).unwrap();
        assert_library_agreement(&input, &report);
        assert_eq!(report["outcome"]["Completed"], expected);
    }
}

#[test]
fn text_escapes_untrusted_sentences_declarations_and_finding_excerpts() {
    let dir = tempfile::tempdir().unwrap();
    let mut input: Value = serde_json::from_str(NOMINAL).unwrap();
    let hostile = UNTRUSTED;
    input["sentence"] = json!(hostile);
    input["grammar"] = json!([hostile, hostile]);
    fs::write(dir.path().join("input.json"), input.to_string()).unwrap();
    let text = stdout(cli(dir.path()).output().unwrap());
    assert!(
        text.contains(&format!("Sentence: \"{}\"", hostile.escape_debug())),
        "{text:?}"
    );
    for id in [1, 2] {
        assert!(
            text.contains(&format!("Grammar {id}: {}", hostile.escape_debug())),
            "{text:?}"
        );
    }
    assert_terminal_safe(&text);
    assert!(
        text.lines()
            .any(|line| line.contains("bytes 0..")
                && line.contains(&hostile.escape_debug().to_string())),
        "{text:?}"
    );
}

fn rejects_input(bytes: impl AsRef<[u8]>, message: &str) {
    let dir = tempfile::tempdir().unwrap();
    let bytes = bytes.as_ref();
    fs::write(dir.path().join("input.json"), bytes).unwrap();
    for json in [false, true] {
        let mut command = cli(dir.path());
        if json {
            command.arg("--json");
        }
        assert_error(command.output().unwrap(), message);
    }
    assert_eq!(fs::read(dir.path().join("input.json")).unwrap(), bytes);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn unsupported_input_versions_are_errors() {
    let mut input: Value = serde_json::from_str(NOMINAL).unwrap();
    input["version"] = json!(2);
    rejects_input(input.to_string(), "Unsupported analysis input version 2");
}

#[test]
fn unknown_input_fields_are_errors() {
    let mut extra: Value = serde_json::from_str(NOMINAL).unwrap();
    extra["unexpected"] = json!(true);
    rejects_input(extra.to_string(), "unknown field");
    extra[UNTRUSTED] = json!(true);
    extra.as_object_mut().unwrap().remove("unexpected");
    rejects_input(extra.to_string(), "unknown field");
}

#[test]
fn invalid_input_errors_include_the_cause_without_evaluation() {
    let input: Value = serde_json::from_str(NOMINAL).unwrap();
    let mut missing = input.clone();
    missing.as_object_mut().unwrap().remove("sentence");
    let mut blank = input.clone();
    blank["sentence"] = json!(" \n　");
    let mut long = input.clone();
    long["sentence"] = json!("犬".repeat(101));
    let mut grammar = input;
    grammar["grammar"] = json!(["　"]);
    for (bytes, message) in [
        (missing.to_string().into_bytes(), "missing field"),
        (b"{broken".to_vec(), "Invalid analysis input JSON"),
        (b"{\"version\":1,\"version\":1}".to_vec(), "duplicate field"),
        (vec![0xff], "Invalid analysis input JSON"),
        (blank.to_string().into_bytes(), "Sentence must not be blank"),
        (long.to_string().into_bytes(), "101 Unicode characters"),
        (
            grammar.to_string().into_bytes(),
            "Grammar entry 1 has a blank description",
        ),
    ] {
        rejects_input(bytes, message);
    }
    let dir = tempfile::tempdir().unwrap();
    assert_error(
        cli(dir.path()).output().unwrap(),
        "explicitly supplied analysis input",
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn sentence_length_error_states_the_limit_without_a_milestone_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut input: Value = serde_json::from_str(NOMINAL).unwrap();
    input["sentence"] = json!("犬".repeat(101));
    fs::write(dir.path().join("input.json"), input.to_string()).unwrap();
    for json in [false, true] {
        let mut command = cli(dir.path());
        if json {
            command.arg("--json");
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        let error = assert_error(output, "101 Unicode characters");
        assert_eq!(
            error,
            "error: Sentence input: Sentence has 101 Unicode characters; limit is 100.\n"
        );
    }
}

#[test]
fn input_reads_are_bounded_at_64_kib() {
    rejects_input(vec![b' '; 65_537], "exceeds 64 KiB");
    let mut input: Value = serde_json::from_str(NOMINAL).unwrap();
    input["version"] = json!(2);
    let mut bytes = input.to_string().into_bytes();
    bytes.resize(65_536, b' ');
    rejects_input(bytes, "Unsupported analysis input version 2");
}

#[test]
fn json_escapes_terminal_controls_without_changing_decoded_input_or_spans() {
    let dir = tempfile::tempdir().unwrap();
    let mut input: Value = serde_json::from_str(NOMINAL).unwrap();
    input["sentence"] = json!(UNTRUSTED);
    input["grammar"] = json!([UNTRUSTED, UNTRUSTED]);
    for field in ["written_form", "reading", "sense"] {
        input["bindings"]["vocabulary"][0][field] = json!(UNTRUSTED);
    }
    fs::write(dir.path().join("input.json"), input.to_string()).unwrap();
    let text = stdout(cli(dir.path()).arg("--json").output().unwrap());
    assert_terminal_safe(&text);
    assert!(text.contains('犬'));
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_library_agreement(&input, &report);
}

#[test]
fn argument_errors_escape_untrusted_values_and_help_still_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let output = cli(dir.path()).arg(UNTRUSTED).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let error = assert_error(output, "unexpected argument");
    assert!(error.contains("\n\nUsage: yomibu analyze"), "{error:?}");
    assert!(
        error.contains("\n\nFor more information, try '--help'.\n"),
        "{error:?}"
    );
    assert!(error.contains(&UNTRUSTED.escape_debug().to_string()));
    let help = stdout(cli(dir.path()).arg("--help").output().unwrap());
    for option in ["--dictionary", "--input", "--json"] {
        assert!(help.contains(option), "{help}");
    }
}

#[test]
fn argument_errors_escape_invalid_subcommands_and_values_without_flattening_help() {
    let dir = tempfile::tempdir().unwrap();
    for (args, message) in [
        (vec![UNTRUSTED], "unrecognized subcommand"),
        (
            vec!["preview-story", "--candidates", UNTRUSTED],
            "invalid digit found in string",
        ),
        (
            vec!["prepare-retrieval", "--knowledge-policy", UNTRUSTED],
            "\n  [possible values: lesson-started, recorded-pass]",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_yomibu"))
            .env_clear()
            .current_dir(dir.path())
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let error = assert_error(output, message);
        assert!(
            error
                .lines()
                .next()
                .unwrap()
                .contains(&UNTRUSTED.escape_debug().to_string()),
            "{error:?}"
        );
        assert!(
            error.contains("\n\nFor more information, try '--help'.\n"),
            "{error:?}"
        );
    }
}

#[test]
#[cfg(unix)]
fn argument_errors_use_a_trusted_executable_name_in_usage() {
    use std::os::unix::process::CommandExt;

    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_yomibu"))
        .env_clear()
        .current_dir(dir.path())
        .arg0(UNTRUSTED)
        .arg("analyze")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let error = assert_error(output, "\n  --input <PATH>\n");
    assert!(
        error.contains("\n\nUsage: yomibu analyze --input <PATH>\n"),
        "{error:?}"
    );
}

#[test]
fn invalid_bindings_are_execution_errors_in_both_output_modes() {
    for (field, value, message) in [
        (
            "vocabulary",
            json!([{"written_form": "犬", "reading": " ", "sense": "dog", "direct_object": false}]),
            "nonblank form, reading, and sense",
        ),
        (
            "grammar",
            json!([{"declaration_id": 2, "rule": "NominalDesu"}]),
            "declaration ID that does not exist",
        ),
        (
            "grammar",
            json!([{"declaration_id": 1, "rule": UNTRUSTED}]),
            "unknown variant",
        ),
        (
            "vocabulary",
            json!([{"written_form": "犬", "reading": "イヌ", "sense": "dog"}]),
            "missing field",
        ),
    ] {
        let mut input: Value = serde_json::from_str(NOMINAL).unwrap();
        input["bindings"][field] = value;
        rejects_input(input.to_string(), message);
    }
}

#[test]
fn input_path_is_required_and_explicit_dictionary_errors_have_no_report() {
    let dir = tempfile::tempdir().unwrap();
    for (args, missing) in [
        (
            vec!["analyze", "--input", "input.json"],
            "explicitly supplied analysis input",
        ),
        (vec!["analyze", "--dictionary", "missing.dic"], "--input"),
    ] {
        assert_error(
            Command::new(env!("CARGO_BIN_EXE_yomibu"))
                .env_clear()
                .current_dir(dir.path())
                .args(args)
                .output()
                .unwrap(),
            missing,
        );
    }
    fs::write(dir.path().join("input.json"), NOMINAL).unwrap();
    let path = dir.path().join("dictionary.dic");
    for message in [
        "Cannot read the explicitly selected dictionary",
        "does not match the pinned",
    ] {
        for json in [false, true] {
            let mut command = cli_with_dictionary(dir.path(), &path);
            if json {
                command.arg("--json");
            }
            assert_error(command.output().unwrap(), message);
        }
        fs::write(&path, b"not a pinned dictionary").unwrap();
    }
    assert_eq!(fs::read(&path).unwrap(), b"not a pinned dictionary");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn object_uncertainty_and_permission_failure_spans_survive_both_presentations() {
    let dir = tempfile::tempdir().unwrap();
    for permitted in [true, false] {
        let mut input: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/analyze/object.json"))
                .unwrap();
        if !permitted {
            input["bindings"]["grammar"] = json!([]);
        }
        fs::write(dir.path().join("input.json"), input.to_string()).unwrap();
        let report: Value =
            serde_json::from_str(&stdout(cli(dir.path()).arg("--json").output().unwrap())).unwrap();
        assert_library_agreement(&input, &report);
        let expected = if permitted { "Inconclusive" } else { "Fail" };
        assert_eq!(report["outcome"]["Completed"], expected);
        assert_eq!(
            report["evaluation"]["particles"]["state"]["Completed"],
            expected
        );
        assert_eq!(
            report["evaluation"]["scope"]["state"]["Completed"],
            "Inconclusive"
        );
        let text = stdout(cli(dir.path()).output().unwrap());
        assert!(
            text.contains(&format!("Overall: {expected} (completed)")),
            "{text}"
        );
        for kind in ["particles", "scope", "inflection"] {
            for finding in report["evaluation"][kind]["findings"].as_array().unwrap() {
                let start = finding["span"]["start"].as_u64().unwrap() as usize;
                let end = finding["span"]["end"].as_u64().unwrap() as usize;
                let original = input["sentence"].as_str().unwrap();
                assert!(
                    text.contains(&format!(
                        "bytes {start}..{end}: \"{}\" — {}",
                        &original[start..end],
                        finding["reason"].as_str().unwrap()
                    )),
                    "{text}"
                );
            }
        }
        assert!(text.contains("bytes 6..24: \"水を飲みます\" — object/predicate combination has no multiword-expression assessment"), "{text}");
        if !permitted {
            for excerpt in [
                "bytes 3..6: \"は\"",
                "bytes 9..12: \"を\"",
                "bytes 18..24: \"ます\"",
            ] {
                assert!(text.contains(excerpt), "{text}");
            }
        }
    }
}

#[test]
fn maximum_sizes_and_poisoned_ambient_state_do_not_trigger_implicit_access_or_writes() {
    let dir = tempfile::tempdir().unwrap();
    let mut input: Value = serde_json::from_str(NOMINAL).unwrap();
    input["sentence"] = json!("犬".repeat(100));
    let mut bytes = input.to_string().into_bytes();
    bytes.resize(65_536, b' ');
    fs::write(dir.path().join("input.json"), &bytes).unwrap();
    let state = dir.path().join(".yomibu");
    fs::create_dir(&state).unwrap();
    for name in ["wanikani.json", "wanikani.lock", "config.toml"] {
        fs::write(state.join(name), b"poisoned, never read").unwrap();
    }
    fs::write(dir.path().join("sudachi.json"), b"invalid ambient config").unwrap();
    let report: Value = serde_json::from_str(&stdout(
        cli(dir.path())
            .env("HOME", dir.path())
            .env("WANIKANI_API_TOKEN", "invalid\nunused")
            .arg("--data-dir")
            .arg(state.join("wanikani.json"))
            .arg("--json")
            .output()
            .unwrap(),
    ))
    .unwrap();
    assert_library_agreement(&input, &report);
    assert_eq!(fs::read(dir.path().join("input.json")).unwrap(), bytes);
    assert_eq!(
        fs::read(dir.path().join("sudachi.json")).unwrap(),
        b"invalid ambient config"
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
    assert_eq!(fs::read_dir(&state).unwrap().count(), 3);
    for name in ["wanikani.json", "wanikani.lock", "config.toml"] {
        assert_eq!(fs::read(state.join(name)).unwrap(), b"poisoned, never read");
    }
}
