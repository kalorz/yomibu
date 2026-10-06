use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

const INPUT: &str = include_str!("../../../tests/fixtures/analyze/nominal.json");

fn cli(directory: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    command.env_clear().current_dir(directory);
    command
}

fn source() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/a1/current")
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
fn offline_import_verify_and_both_loading_policies_agree_through_the_executable() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("managed");
    fs::write(directory.path().join("input.json"), INPUT).unwrap();
    let imported = success(
        cli(directory.path())
            .args(["dictionary", "import", "--bundle"])
            .arg(source())
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
    let pointer = fs::read(root.join("current")).unwrap();
    let mapped = success(
        cli(directory.path())
            .args(["analyze", "--input", "input.json", "--dictionary-dir"])
            .arg(&root)
            .arg("--json")
            .output()
            .unwrap(),
    );
    let mut mapped: Value = serde_json::from_str(&mapped).unwrap();
    assert_eq!(
        mapped["analysis"]["provenance"]["dictionary_loading"]["storage"],
        "memory_mapped"
    );
    mapped["analysis"]["provenance"]
        .as_object_mut()
        .unwrap()
        .remove("dictionary_loading");
    let external = success(
        cli(directory.path())
            .args(["analyze", "--input", "input.json", "--dictionary"])
            .arg(source().join("system_core.dic"))
            .arg("--json")
            .output()
            .unwrap(),
    );
    assert_eq!(mapped, serde_json::from_str::<Value>(&external).unwrap());
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
            "full SHA-256 verified at installation; startup checks records, size and header"
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
fn managed_home_default_is_offline_and_external_selection_requires_no_home() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("input.json"), INPUT).unwrap();
    let root = directory.path().join(".yomibu/dictionaries");
    success(
        cli(directory.path())
            .env("HOME", directory.path())
            .args(["dictionary", "import", "--bundle"])
            .arg(source())
            .output()
            .unwrap(),
    );
    for name in ["config.toml", "wanikani.json", "wanikani.lock"] {
        fs::write(
            directory.path().join(".yomibu").join(name),
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
            .args([
                "analyze",
                "--input",
                "input.json",
                "--data-dir",
                "unused",
                "--json",
            ])
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
            .args(["analyze", "--input", "input.json", "--dictionary"])
            .arg(source().join("system_core.dic"))
            .output()
            .unwrap(),
    );
    let missing = cli(directory.path())
        .args(["analyze", "--input", "input.json"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("HOME is unavailable; specify --dictionary-dir PATH")
    );
}

#[test]
fn selection_conflicts_missing_installations_and_help_have_safe_useful_layout() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("input.json"), INPUT).unwrap();
    let hostile = "猫\u{1b}[31m\n\r\u{202e}";
    let conflict = cli(directory.path())
        .args([
            "analyze",
            "--input",
            "input.json",
            "--dictionary",
            hostile,
            "--dictionary-dir",
            "managed",
        ])
        .output()
        .unwrap();
    assert_eq!(conflict.status.code(), Some(2));
    assert!(conflict.stdout.is_empty());
    let diagnostic = String::from_utf8(conflict.stderr).unwrap();
    assert!(
        diagnostic.contains("\n\nUsage: yomibu analyze"),
        "{diagnostic:?}"
    );
    for character in ['\u{1b}', '\r', '\u{202e}'] {
        assert!(!diagnostic.contains(character));
    }
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
