use std::{path::Path, process::Command};
fn cli(dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    command.arg("--no-keychain");
    command.env_clear().env("HOME", dir);
    command
}

#[test]
fn bare_help_and_help_command_list_every_command_without_setup_or_writes() {
    let dir = tempfile::tempdir().unwrap();
    for args in [vec![], vec!["help"], vec!["--help"]] {
        let output = cli(dir.path()).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Usage: yomibu"));
        for command in ["auth", "sync", "story", "help"] {
            assert!(text.contains(command), "{text}");
        }
        for removed in [
            "preview-story",
            "prepare-retrieval",
            "analyze",
            "dictionary",
            "status",
            "--json",
        ] {
            assert!(!text.contains(removed), "{text}");
        }
        assert!(!text.contains("generate-story"));
        let credential_flags: Vec<_> = text
            .split_whitespace()
            .filter(|word| word.starts_with("--") && word.ends_with("-api-key"))
            .collect();
        assert_eq!(credential_flags, ["--openai-api-key", "--wanikani-api-key"]);
        let (_, credentials_help) = text
            .split_once("\nCredentials:\n")
            .expect("Missing Credentials help section");
        assert!(credentials_help.contains("--no-keychain"));
        assert!(credentials_help.contains("--openai-api-key"));
        assert!(credentials_help.contains("--wanikani-api-key"));
        assert!(credentials_help.contains("YOMIBU_OPENAI_API_KEY"));
        assert!(credentials_help.contains("YOMIBU_WANIKANI_API_KEY"));
        assert!(!credentials_help.contains("--data-dir"));
        assert!(!text.contains("--credential-"));
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn missing_inventory_setup_names_the_available_inputs_without_listing_tuning_flags() {
    let dir = tempfile::tempdir().unwrap();
    let output = cli(dir.path())
        .args([
            "story",
            "--disable",
            "sync",
            "--openai-api-key",
            "synthetic-key",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(
        text.starts_with("error: Missing required setup:\n  Learner vocabulary:\n    "),
        "{text}"
    );
    assert!(text.contains("--inventory"), "{text}");
    assert!(text.contains("--enable sync"), "{text}");
    assert!(!text.contains("--openai-model"));
    assert!(!text.contains("synthetic-key"));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn story_collects_minimum_setup_and_only_accepts_prefixed_environment_bindings() {
    let dir = tempfile::tempdir().unwrap();
    let output = cli(dir.path())
        .arg("story")
        .env("WANIKANI_API_TOKEN", "old-secret")
        .env("OPENAI_API_KEY", "old-secret")
        .envs(
            [
                "YOMIBU_WANIKANI_SOURCE_API_KEY",
                "YOMIBU_OPENAI_STORY_GENERATION_API_KEY",
                "YOMIBU_CREDENTIAL_OPENAI",
                "YOMIBU_CREDENTIAL_WANIKANI",
            ]
            .map(|name| (name, "old-secret")),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("--wanikani-api-key"));
    assert!(text.contains("--openai-api-key"));
    assert!(text.contains("Missing required setup:\n  WaniKani sync:\n    "));
    assert!(!text.contains("--openai-model"));
    assert!(text.contains("no write permissions"));
    assert!(!text.contains("old-secret"));
    let output = cli(dir.path())
        .args(["story", "--wanikani-api-key", "flag-secret"])
        .env("YOMIBU_WANIKANI_API_KEY", "environment-secret")
        .output()
        .unwrap();
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!text.contains("--wanikani-api-key"));
    assert!(text.contains("--openai-api-key"));
    assert!(!text.contains("secret"));
    assert!(!dir.path().join(".yomibu").exists());
}

#[test]
fn missing_keys_recommend_component_auth_even_when_automatic_sync_is_disabled() {
    let dir = tempfile::tempdir().unwrap();
    for (command, sync) in [("story", true), ("sync", false)] {
        std::fs::write(
            dir.path().join("config.toml"),
            format!("[application]\nsync={sync}\n"),
        )
        .unwrap();
        let output = cli(dir.path())
            .arg("--data-dir")
            .arg(dir.path())
            .arg(command)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(text.contains("Missing required setup:\n  "), "{text}");
        assert!(
            text.contains("On macOS, run yomibu auth wanikani."),
            "{text}"
        );
        if command == "story" {
            assert!(text.contains("On macOS, run yomibu auth openai."), "{text}");
        }
        assert!(!dir.path().join("wanikani.json.lock").exists());
    }
}

#[test]
fn story_help_and_parser_diagnostics_are_safe_readable_and_redact_keys() {
    let dir = tempfile::tempdir().unwrap();
    let help = cli(dir.path()).args(["help", "story"]).output().unwrap();
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    let text = String::from_utf8(help.stdout).unwrap();
    for flag in [
        "--topic",
        "--model",
        "--openai-model",
        "--enable",
        "--disable",
        "--wanikani-api-key",
        "--openai-api-key",
    ] {
        assert!(text.contains(flag), "{text}");
    }
    assert!(text.contains("api.responses.write"));
    for (requirement, invalid_option) in yomibu::configuration::components::credentials()
        .flat_map(|key| ["--format", "--candidates"].map(|flag| (key, flag)))
    {
        let output = cli(dir.path())
            .args([
                "story",
                &format!("--{}", requirement.name.cli()),
                "synthetic-secret",
                invalid_option,
                "日本語\n\u{1b}",
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(text.contains("日本語"));
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains("\n\u{1b}"));
        assert!(text.contains("\\n\\u{1b}"), "{text}");
        assert!(!text.contains("synthetic-secret"));
        assert!(text.contains("\n\nUsage:"));
    }
}
