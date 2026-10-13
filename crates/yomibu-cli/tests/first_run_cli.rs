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
fn configuration_errors_escape_paths_redact_values_and_keep_one_line() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("日本語\n\u{1b}");
    std::fs::create_dir(&data).unwrap();
    let path = data.join("default-pipeline.toml");
    let private = "synthetic-private\n\u{1b}";
    let invalid_model = private.repeat(20);
    for (file, environment, value, expected) in [
        (
            format!("[pipeline.options.openai]\nmodel=[{}]\n", serde_json::to_string(private).unwrap()),
            "YOMIBU_SELECT",
            "2",
            format!("Invalid configuration at {}: Invalid openai.model: Wrong component option type.", path.display()),
        ),
        (
            String::new(),
            "YOMIBU_SELECT",
            private,
            "Invalid value for YOMIBU_SELECT; check its format in yomibu help story.".into(),
        ),
        (
            String::new(),
            "YOMIBU_OPENAI_MODEL",
            invalid_model.as_str(),
            "Choose a nonblank text model and a positive candidate count that fits the output budget.".into(),
        ),
    ] {
        std::fs::write(&path, file).unwrap();
        let output = cli(dir.path())
            .arg("--data-dir")
            .arg(&data)
            .arg("story")
            .env(environment, value)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert_eq!(error, format!("error: {}\n", expected.escape_debug()));
        assert_eq!(error.lines().count(), 1);
        assert!(!error.contains("synthetic-private"));
        assert!(!error.contains('\u{1b}'));
    }
}

#[test]
fn missing_inventory_setup_names_the_available_inputs_without_listing_tuning_flags() {
    let dir = tempfile::tempdir().unwrap();
    let output = cli(dir.path())
        .args(["story", "--openai-api-key", "synthetic-key"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(
        text.starts_with("error: Missing required setup:\n  Learner vocabulary:\n    "),
        "{text}"
    );
    assert!(text.contains("yomibu sync"), "{text}");
    assert!(!text.contains("--sync"), "{text}");
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
    assert!(!text.contains("--wanikani-api-key"));
    assert!(text.contains("--openai-api-key"));
    assert!(text.contains("Missing required setup:\n  Learner vocabulary:\n    "));
    assert!(!text.contains("--openai-model"));
    assert!(text.contains("yomibu sync"));
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
fn missing_keys_recommend_auth_for_the_requested_operation() {
    let dir = tempfile::tempdir().unwrap();
    for command in ["story", "sync"] {
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
            text.contains(if command == "sync" {
                "On macOS, run yomibu auth wanikani."
            } else {
                "yomibu sync"
            }),
            "{text}"
        );
        if command == "story" {
            assert!(
                text.contains("Use the same --data-dir for sync and story."),
                "{text}"
            );
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
        "--openai-model",
        "--wanikani-api-key",
        "--openai-api-key",
    ] {
        assert!(text.contains(flag), "{text}");
    }
    assert!(text.contains("Run yomibu sync to load learner vocabulary"));
    assert!(!text.contains("On macOS, run yomibu auth wanikani."));
    let key = yomibu_ext::openai::API_KEY;
    assert!(text.contains(key.description), "{text}");
    assert!(text.contains("On macOS, run yomibu auth openai."), "{text}");
    assert!(!text.contains("--enable"));
    assert!(!text.contains("--disable"));
    for (requirement, invalid_option) in ["--openai-api-key", "--wanikani-api-key"]
        .into_iter()
        .flat_map(|key| ["--format", "--candidates", "--select"].map(|flag| (key, flag)))
    {
        let output = cli(dir.path())
            .args([
                "story",
                requirement,
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
