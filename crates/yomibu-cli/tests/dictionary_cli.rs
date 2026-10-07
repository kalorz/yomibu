#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;

use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

const INPUT: &str = include_str!("../../../tests/fixtures/analyze/nominal.json");

fn cli(directory: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    command
        .env_clear()
        .current_dir(directory)
        .env("YOMIBU_DATA_DIR", directory.join("data"));
    command
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn help_exposes_only_managed_dictionary_selection_without_accessing_resources() {
    let directory = tempfile::tempdir().unwrap();
    for command in ["analyze", "story"] {
        let help = success(
            cli(directory.path())
                .args([command, "--help"])
                .output()
                .unwrap(),
        );
        assert!(help.contains("--dictionary-dir <PATH>"), "{help}");
        assert!(
            !help
                .lines()
                .any(|line| line.trim_start().starts_with("--dictionary ")),
            "help still exposes arbitrary dictionary files: {help}"
        );
    }
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn offline_import_verify_and_analysis_report_the_selected_generation() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("data")).unwrap();
    fs::write(directory.path().join("data/config.toml"), "model = ''\nformat = 'unused-invalid'\nselect = 40\ncandidates = 0\ntopic = '猫'\nrequest = 'unused.json'\n").unwrap();
    let root = directory.path().join("managed");
    fs::write(directory.path().join("input.json"), INPUT).unwrap();
    let imported = success(
        cli(directory.path())
            .args(["dictionary", "import", "--bundle"])
            .arg(test_dictionary::bundle())
            .arg("--dictionary-dir")
            .arg(&root)
            .output()
            .unwrap(),
    );
    assert!(
        imported.contains("Verified dictionary installed"),
        "{imported}"
    );
    let verified = success(
        cli(directory.path())
            .args(["dictionary", "verify", "--dictionary-dir"])
            .arg(&root)
            .output()
            .unwrap(),
    );
    assert!(verified.contains("Full pinned verification passed"));
    let imported_json: Value = serde_json::from_str(&success(
        cli(directory.path())
            .args(["dictionary", "import", "--json", "--bundle"])
            .arg(test_dictionary::bundle())
            .arg("--dictionary-dir")
            .arg(&root)
            .output()
            .unwrap(),
    ))
    .unwrap();
    assert!(
        imported_json["generation"]
            .as_str()
            .unwrap()
            .starts_with("core-")
    );
    let verified_json: Value = serde_json::from_str(&success(
        cli(directory.path())
            .args(["dictionary", "verify", "--json", "--dictionary-dir"])
            .arg(&root)
            .output()
            .unwrap(),
    ))
    .unwrap();
    assert_eq!(verified_json["verified"], true);
    let pointer = fs::read(root.join("current")).unwrap();
    let mapped = success(
        cli(directory.path())
            .args(["analyze", "--input", "input.json", "--dictionary-dir"])
            .arg(&root)
            .arg("--json")
            .output()
            .unwrap(),
    );
    let mapped: Value = serde_json::from_str(&mapped).unwrap();
    assert_eq!(
        mapped["analysis"]["provenance"]["dictionary_loading"]["generation"],
        imported_json["generation"]
    );
    let text = success(
        cli(directory.path())
            .args(["analyze", "--input", "input.json", "--dictionary-dir"])
            .arg(&root)
            .output()
            .unwrap(),
    );
    assert!(text.contains("Sentence: \"犬です。\""));
    assert!(
        text.contains(
            "full SHA-256 verified at installation; startup checks records, file metadata and header"
        ),
        "{text}"
    );
    assert!(text.contains("requires unchanged managed files"));
    assert_eq!(fs::read(root.join("current")).unwrap(), pointer);
    let failed = cli(directory.path())
        .args([
            "dictionary",
            "import",
            "--bundle",
            "input.json",
            "--dictionary-dir",
        ])
        .arg(&root)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert_eq!(fs::read(root.join("current")).unwrap(), pointer);
}

#[test]
fn managed_data_directory_default_and_explicit_selection_remain_offline() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("input.json"), INPUT).unwrap();
    let root = directory.path().join("data/dictionaries");
    success(
        cli(directory.path())
            .env("HOME", directory.path())
            .args(["dictionary", "import", "--bundle"])
            .arg(test_dictionary::bundle())
            .output()
            .unwrap(),
    );
    for name in ["wanikani.json", "wanikani.json.lock"] {
        fs::write(
            directory.path().join("data").join(name),
            b"poisoned unused learner/config file",
        )
        .unwrap();
    }
    for name in ["sudachi.json", "char.def", ".env"] {
        fs::write(directory.path().join(name), b"poisoned unused ambient file").unwrap();
    }
    let pointer = fs::read(root.join("current")).unwrap();
    let report = success(
        cli(directory.path())
            .env("HOME", directory.path())
            .env("OPENAI_API_KEY", "unused\nsecret")
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .args(["analyze", "--input", "input.json", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        serde_json::from_str::<Value>(&report).unwrap()["outcome"]["Completed"],
        "Pass"
    );
    assert!(!report.contains("secret"));
    assert_eq!(fs::read(root.join("current")).unwrap(), pointer);
    success(
        cli(directory.path())
            .args(["analyze", "--input", "input.json", "--dictionary-dir"])
            .arg(test_dictionary::installation())
            .output()
            .unwrap(),
    );
    let missing = cli(directory.path())
        .args([
            "analyze",
            "--input",
            "input.json",
            "--dictionary-dir",
            "nonexistent",
        ])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("dictionary import --bundle PATH")
    );
}

#[test]
fn missing_installations_and_help_have_safe_useful_layout() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("input.json"), INPUT).unwrap();
    let hostile = "猫\u{1b}[31m\n\r\u{202e}";
    let missing = cli(directory.path())
        .args([
            "analyze",
            "--input",
            "input.json",
            "--dictionary-dir",
            hostile,
        ])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
    let diagnostic = String::from_utf8(missing.stderr).unwrap();
    assert!(diagnostic.contains("dictionary import"), "{diagnostic}");
    assert!(diagnostic.starts_with("error: "), "{diagnostic:?}");
    assert_eq!(diagnostic.lines().count(), 1, "{diagnostic:?}");
    for character in ['\u{1b}', '\r', '\u{202e}'] {
        assert!(!diagnostic.contains(character));
    }
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    for command in [
        vec!["analyze", "--help"],
        vec!["dictionary", "import", "--help"],
        vec!["dictionary", "verify", "--help"],
        vec!["--version"],
    ] {
        let help = success(cli(directory.path()).args(command).output().unwrap());
        assert!(!help.is_empty());
    }
}
