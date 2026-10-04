use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const INPUT: &str = include_str!("fixtures/generation/dog-cat.json");

fn dictionary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a1/current/system_core.dic")
}

fn cli(dir: &Path, dictionary: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    command
        .env_clear()
        .current_dir(dir)
        .args([
            "generate-candidates",
            "--allow-model-call",
            "--input",
            "input.json",
            "--dictionary",
        ])
        .arg(dictionary);
    command
}

fn error(output: Output, code: i32, expected: &str) -> String {
    assert_eq!(output.status.code(), Some(code));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(expected), "{stderr:?}");
    assert!(!stderr.contains('\u{1b}'));
    assert!(!stderr.contains('\r'));
    stderr
}

#[test]
fn opt_in_and_explicit_paths_are_required_and_help_is_readable() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_yomibu"))
        .env_clear()
        .current_dir(dir.path())
        .args([
            "generate-candidates",
            "--input",
            "input.json",
            "--dictionary",
            "dictionary.dic",
        ])
        .output()
        .unwrap();
    let stderr = error(output, 2, "--allow-model-call");
    assert!(stderr.contains("\nUsage: yomibu generate-candidates"));
    assert!(stderr.contains("\nFor more information, try '--help'.\n"));
    for args in [
        vec!["generate-candidates", "--help"],
        vec!["--help"],
        vec!["--version"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_yomibu"))
            .env_clear()
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert!(!output.stdout.is_empty());
    }
}

#[test]
fn new_command_argument_errors_escape_values_without_flattening_layout() {
    let output = Command::new(env!("CARGO_BIN_EXE_yomibu"))
        .env_clear()
        .args(["generate-candidates", "--猫\n\u{1b}bad"])
        .output()
        .unwrap();
    let stderr = error(output, 2, "--猫\\n\\u{1b}bad");
    assert!(stderr.contains("\n\nUsage: yomibu generate-candidates"));
    assert!(stderr.ends_with("\nFor more information, try '--help'.\n"));
    for option in ["--model", "--provider", "--base-url"] {
        let output = Command::new(env!("CARGO_BIN_EXE_yomibu"))
            .env_clear()
            .args(["generate-candidates", option, "supplied"])
            .output()
            .unwrap();
        error(output, 2, "unexpected argument");
    }
}

#[test]
fn strict_input_and_bindings_fail_before_dictionary_or_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let base: Value = serde_json::from_str(INPUT).unwrap();
    let mut malformed = vec![b"{".to_vec(), vec![0xff], b"{\"version\":1,\"version\":1,\"grammar\":[],\"bindings\":{\"vocabulary\":[],\"grammar\":[]}}".to_vec()];
    for field in ["version", "grammar", "bindings"] {
        let mut value = base.clone();
        value.as_object_mut().unwrap().remove(field);
        malformed.push(value.to_string().into_bytes());
    }
    let mut extra = base.clone();
    extra["sentence"] = json!("犬です。");
    malformed.push(extra.to_string().into_bytes());
    for bytes in malformed {
        fs::write(dir.path().join("input.json"), bytes).unwrap();
        error(
            cli(dir.path(), Path::new("missing.dic")).output().unwrap(),
            1,
            "Invalid generation input JSON",
        );
    }
    for (value, expected) in [
        (
            json!({"version":2,"grammar":[],"bindings":{"vocabulary":[],"grammar":[]}}),
            "Unsupported generation input version 2",
        ),
        (
            json!({"version":1,"grammar":[" "],"bindings":{"vocabulary":[],"grammar":[]}}),
            "Grammar input",
        ),
        (
            json!({"version":1,"grammar":[],"bindings":{"vocabulary":[],"grammar":[{"declaration_id":1,"rule":"NominalDesu"}]}}),
            "declaration ID",
        ),
        (
            json!({"version":1,"grammar":[],"bindings":{"vocabulary":[{"written_form":"犬","reading":" ","sense":"dog","direct_object":false}],"grammar":[]}}),
            "nonblank form",
        ),
    ] {
        fs::write(dir.path().join("input.json"), value.to_string()).unwrap();
        error(
            cli(dir.path(), Path::new("missing.dic")).output().unwrap(),
            1,
            expected,
        );
    }
    let mut bytes = INPUT.as_bytes().to_vec();
    bytes.resize(65536, b' ');
    fs::write(dir.path().join("input.json"), &bytes).unwrap();
    error(
        cli(dir.path(), Path::new("missing.dic")).output().unwrap(),
        1,
        "Dictionary initialization",
    );
    bytes.push(b' ');
    fs::write(dir.path().join("input.json"), bytes).unwrap();
    error(
        cli(dir.path(), Path::new("missing.dic")).output().unwrap(),
        1,
        "Generation input exceeds 64 KiB",
    );
}

#[test]
fn credential_lookup_is_explicit_after_local_preflight_without_discovery_or_writes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("input.json"), INPUT).unwrap();
    let output = cli(dir.path(), &dictionary())
        .args(["--data-dir", "ignored"])
        .output()
        .unwrap();
    error(output, 1, "Set OPENAI_API_KEY");
    let output = cli(dir.path(), &dictionary())
        .env("OPENAI_API_KEY", "synthetic-secret\nInjected")
        .output()
        .unwrap();
    let stderr = error(output, 1, "credential");
    assert!(!stderr.contains("synthetic-secret"));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}
