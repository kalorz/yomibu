use serde_json::{Value, json};
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
    c.arg("--no-keychain");
    c.env_clear()
        .current_dir(dir)
        .env("YOMIBU_DATA_DIR", dir.join("data"))
        .args([
            command,
            "--inventory",
            "inventory.json",
            "--request",
            "request.json",
            "--select",
            "2",
        ]);
    c
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
    let o = cli(dir.path(), "story")
        .args(["--permissions", "x\n\u{1b}"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(!String::from_utf8(o.stderr).unwrap().contains('\u{1b}'));
    for args in [vec!["--help"], vec!["story", "--help"], vec!["--version"]] {
        let o = Command::new(env!("CARGO_BIN_EXE_yomibu"))
            .arg("--no-keychain")
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
fn malformed_and_oversized_inputs_fail_before_credentials() {
    let dir = setup();
    for input in ["{".to_owned(), " ".repeat(4_194_305)] {
        fs::write(dir.path().join("inventory.json"), input).unwrap();
        let o = cli(dir.path(), "story").output().unwrap();
        assert_eq!(o.status.code(), Some(1));
        assert!(o.stdout.is_empty());
        let e = String::from_utf8(o.stderr).unwrap();
        assert!(e.contains("inventory"), "{e}");
        assert!(!e.contains("OPENAI_API_KEY"));
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
            .arg("--no-keychain")
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
fn impossible_selection_fails_before_generation() {
    let dir = setup();
    let mut request: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/story/request.json")).unwrap();
    request["targets"]["vocabulary"] = json!(["cat", "sleep", "dog"]);
    fs::write(dir.path().join("request.json"), request.to_string()).unwrap();
    let out = cli(dir.path(), "story")
        .args(["--openai-api-key", "unused"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(!dir.path().join("vectors.json").exists());
    assert!(out.stdout.is_empty());
    let error = String::from_utf8(out.stderr).unwrap();
    assert!(error.contains("selection limit"), "{error}");
}

#[test]
fn invalid_candidate_count_fails_before_generation() {
    let dir = setup();
    for (count, status) in [("0".to_owned(), 2), (usize::MAX.to_string(), 1)] {
        let out = cli(dir.path(), "story")
            .args(["--candidates", &count])
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
    }
}

#[test]
fn generation_credentials_are_explicit_after_local_preflight_without_discovery_or_writes() {
    let dir = setup();
    fs::write(dir.path().join(".env"), "OPENAI_API_KEY=synthetic-unused").unwrap();
    let before = fs::read_dir(dir.path()).unwrap().count();
    for key in [None, Some("synthetic-secret\nInjected")] {
        let mut command = cli(dir.path(), "story");
        command.args(["--data-dir", "ignored"]);
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
