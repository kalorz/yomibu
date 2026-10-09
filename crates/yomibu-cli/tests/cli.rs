use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    command.arg("--no-keychain");
    command.env_clear();
    command
}

fn cache(dir: &Path, data: &str) {
    fs::write(dir.join("wanikani.json"), data).unwrap();
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn status_is_offline_and_uses_a_global_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    cache(
        dir.path(),
        include_str!("../../../tests/fixtures/mixed.json"),
    );
    fs::write(dir.path().join("config.toml"), "[application]").unwrap();
    fs::write(dir.path().join("wanikani.json.lock"), "existing lock").unwrap();
    let before = fs::read(dir.path().join("wanikani.json")).unwrap();
    for args in [
        vec!["--data-dir", dir.path().to_str().unwrap(), "status"],
        vec!["status", "--data-dir", dir.path().to_str().unwrap()],
    ] {
        let output = cli().args(args).output().unwrap();
        assert_eq!(
            stdout(&output),
            include_str!("../../../tests/fixtures/mixed-status.txt")
        );
    }
    assert_eq!(fs::read(dir.path().join("wanikani.json")).unwrap(), before);
    assert_eq!(
        fs::read_to_string(dir.path().join("wanikani.json.lock")).unwrap(),
        "existing lock"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("config.toml")).unwrap(),
        "[application]"
    );
}

#[test]
fn status_uses_the_configured_cache_without_validating_unused_story_settings() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("source.json"),
        include_bytes!("../../../tests/fixtures/mixed.json"),
    )
    .unwrap();
    fs::write(
        dir.path().join("config.toml"),
        "[application]\nwanikani_cache='source.json'\ndictionary_dir='unused'\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("default-pipeline.toml"),
        "[pipeline]\nmodel=''\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("default-story.toml"),
        "[story]\nselect=40\ncandidates=0\nformat='invalid-unused'\n",
    )
    .unwrap();
    let before = fs::read_dir(dir.path()).unwrap().count();
    let output = cli()
        .args(["--data-dir", dir.path().to_str().unwrap(), "status"])
        .env("YOMIBU_SELECT", "日本語\n\u{1b}")
        .env("YOMIBU_CANDIDATES", "0")
        .output()
        .unwrap();
    assert_eq!(
        stdout(&output),
        include_str!("../../../tests/fixtures/mixed-status.txt")
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), before);
    fs::write(
        dir.path().join("config.toml"),
        "broken TOML '日本語\n\u{1b}",
    )
    .unwrap();
    let output = cli()
        .args(["--data-dir", dir.path().to_str().unwrap(), "status"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(
        diagnostic.starts_with("error: Invalid configuration at "),
        "{diagnostic}"
    );
    assert!(
        diagnostic.ends_with("; use supported settings and valid TOML.\n"),
        "{diagnostic}"
    );
    assert!(!diagnostic.contains("broken TOML"));
    assert!(!diagnostic.contains('\u{1b}'));
}

#[test]
fn pasted_credential_values_in_config_are_rejected_without_echoing_them() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    fs::write(
        &config,
        "[application.credentials]\n\"openai.api-key\"=\"synthetic-secret\\n\\u001b日本語\"\n",
    )
    .unwrap();
    for operation in [vec!["status"], vec!["sync"], vec!["dictionary", "verify"]] {
        let output = cli()
            .arg("--data-dir")
            .arg(dir.path())
            .args(operation)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!(
                "error: Invalid configuration at {}; use supported settings and valid TOML.\n",
                config.display(),
            )
        );
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn empty_account_displays_no_reviews_and_no_assignments() {
    let dir = tempfile::tempdir().unwrap();
    cache(
        dir.path(),
        include_str!("../../../tests/fixtures/empty.json"),
    );
    let output = cli()
        .arg("status")
        .arg("--data-dir")
        .arg(dir.path())
        .output()
        .unwrap();
    let text = stdout(&output);
    for expected in [
        "User: テスト (level 1)",
        "Synchronized kanji: 0",
        "Synchronized vocabulary: 0 (kana-only: 0)",
        "  no assignments",
        "Reading accuracy: no reviews",
        "Meaning accuracy: no reviews",
    ] {
        assert!(text.contains(expected), "{text}");
    }
}

#[test]
fn cache_failures_are_actionable_and_exit_unsuccessfully() {
    for (data, message) in [
        (None, "No cache"),
        (Some("{"), "Corrupt cache"),
        (
            Some(r#"{"schema_version":2}"#),
            "Unsupported cache schema 2",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        if let Some(data) = data {
            cache(dir.path(), data);
        }
        let output = cli()
            .arg("status")
            .arg("--data-dir")
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(!output.status.success(), "cache error exited successfully");
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains(message), "{error}");
        assert!(error.contains("--data-dir"), "{error}");
    }
}

#[test]
fn defaults_to_home_dot_yomibu() {
    let home = tempfile::tempdir().unwrap();
    let data_dir = home.path().join(".yomibu");
    fs::create_dir(&data_dir).unwrap();
    cache(
        &data_dir,
        include_str!("../../../tests/fixtures/empty.json"),
    );
    let output = cli()
        .env("HOME", home.path())
        .arg("status")
        .output()
        .unwrap();
    assert!(stdout(&output).contains("User: テスト (level 1)"));
}

#[test]
fn missing_or_empty_home_requires_an_explicit_directory() {
    for home in [None, Some("")] {
        let mut command = cli();
        if let Some(home) = home {
            command.env("HOME", home);
        }
        let output = command.arg("status").output().unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("HOME is unavailable"), "{error}");
        assert!(error.contains("--data-dir PATH"), "{error}");
    }
}

#[test]
fn sync_requires_an_environment_token_before_creating_files() {
    for token in [None, Some(""), Some("   ")] {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("data");
        let mut command = cli();
        if let Some(token) = token {
            command.env("YOMIBU_WANIKANI_API_KEY", token);
        }
        let output = command
            .arg("sync")
            .arg("--data-dir")
            .arg(&dir)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("YOMIBU_WANIKANI_API_KEY"), "{error}");
        assert!(!dir.exists());
    }
}

#[test]
fn missing_cache_guidance_points_to_sync_and_status_ignores_invalid_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let output = cli()
        .args(["status", "--data-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("yomibu sync"), "{text}");
    cache(
        dir.path(),
        include_str!("../../../tests/fixtures/empty.json"),
    );
    let output = cli()
        .env("YOMIBU_WANIKANI_API_KEY", "invalid\nsynthetic-token")
        .args(["status", "--data-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(stdout(&output).contains("Cached WaniKani observations"));
}

#[test]
fn another_process_cannot_sync_while_status_reads_the_locked_cache() {
    use yomibu_components::file_learning_store::cache::SyncGuard;
    use yomibu_components::file_learning_store::cache::load;
    let dir = tempfile::tempdir().unwrap();
    cache(
        dir.path(),
        include_str!("../../../tests/fixtures/mixed.json"),
    );
    let guard = SyncGuard::acquire(dir.path()).unwrap();
    let before = fs::read(dir.path().join("wanikani.json")).unwrap();
    let token = "synthetic-contending-credential";
    let output = cli()
        .env("YOMIBU_WANIKANI_API_KEY", token)
        .args(["sync", "--data-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("Another sync holds the lock"), "{error}");
    assert!(!error.contains(token));
    let output = cli()
        .args(["status", "--data-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(
        stdout(&output),
        include_str!("../../../tests/fixtures/mixed-status.txt")
    );
    assert_eq!(fs::read(dir.path().join("wanikani.json")).unwrap(), before);
    load(dir.path()).unwrap();
    drop(guard);
    assert!(SyncGuard::acquire(dir.path()).is_ok());
}

#[test]
fn version_output_uses_the_public_executable_name() {
    let mut command = cli();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.arg0("untrusted\n\u{1b}Executable");
    }
    let output = command.arg("--version").output().unwrap();
    assert_eq!(
        stdout(&output),
        format!("yomibu {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn retired_preview_and_prepare_commands_have_no_aliases() {
    for args in [
        vec!["preview", "--word", "猫:ねこ:cat", "--take", "1"],
        vec![
            "prepare",
            "--grammar-file",
            "missing.json",
            "--target",
            "猫:ねこ:cat",
        ],
    ] {
        let output = cli().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("unrecognized subcommand"), "{error}");
        assert!(error.contains("Usage: yomibu"), "{error}");
    }
    let output = cli().arg("--help").output().unwrap();
    let help = stdout(&output);
    assert!(!help.contains("  preview "));
    assert!(!help.contains("  prepare "));
    assert!(help.contains("preview-story"));
    assert!(help.contains("prepare-retrieval"));
}

#[test]
fn component_model_options_and_provider_keys_have_safe_parser_diagnostics() {
    let help = cli().args(["story", "--help"]).output().unwrap();
    let help = stdout(&help);
    assert!(help.contains("--openai-model"));
    assert!(help.contains("--http-embeddings-dimensions"));
    assert!(!help.contains("--generation-model"));
    for args in [
        vec![
            "story",
            "--openai-api-key=synthetic-secret",
            "--format",
            "invalid",
        ],
        vec![
            "story",
            "--openai-api-key",
            "synthetic-secret",
            "--openai-api-key",
            "duplicate-secret",
        ],
        vec![
            "story",
            "--openai-api-key",
            "synthetic-secret",
            "--topic",
            "猫",
            "--request",
            "story.json",
        ],
    ] {
        let output = cli().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.starts_with("error:"), "{error}");
        assert!(error.contains("\n\nUsage: yomibu"), "{error}");
        assert!(!error.contains("synthetic-secret"), "{error}");
        assert!(!error.contains("duplicate-secret"), "{error}");
    }
}

#[test]
fn credential_redaction_preserves_help_and_version() {
    for flag in ["--help", "--version"] {
        let output = cli()
            .args(["--openai-api-key=synthetic-secret", flag])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn retired_credential_flags_are_rejected_without_exposing_keys() {
    for flag in [
        "--wanikani-source-api-key",
        "--openai-story-generation-api-key",
        "--credential-openai",
        "--credential-wanikani",
    ] {
        for args in [
            vec!["story".into(), flag.into(), "synthetic-secret".into()],
            vec!["story".into(), format!("{flag}=synthetic-secret")],
        ] {
            let output = cli().args(args).output().unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            let error = String::from_utf8(output.stderr).unwrap();
            assert!(error.contains("unexpected argument"), "{error}");
            assert!(error.contains("\n\nUsage: yomibu"), "{error}");
            assert!(!error.contains("synthetic-secret"), "{error}");
        }
    }
}

#[cfg(unix)]
#[test]
fn invalid_credentials_are_lazy_and_cannot_fall_through_to_lower_precedence_keys() {
    use std::os::unix::ffi::OsStringExt;
    let dir = tempfile::tempdir().unwrap();
    cache(
        dir.path(),
        include_str!("../../../tests/fixtures/mixed.json"),
    );
    let before = fs::read(dir.path().join("wanikani.json")).unwrap();
    for input in ["environment", "cli", "cli-equals"] {
        for operation in ["status", "sync"] {
            let mut command = cli();
            command.args(["--data-dir", dir.path().to_str().unwrap(), operation]);
            command.env("YOMIBU_WANIKANI_API_KEY", "invalid\nenvironment");
            let invalid = std::ffi::OsString::from_vec(b"synthetic-secret\xff".to_vec());
            if input == "environment" {
                command.env("YOMIBU_WANIKANI_API_KEY", invalid);
            } else {
                let flag = "--wanikani-api-key";
                if input.ends_with("-equals") {
                    let mut argument = std::ffi::OsString::from(format!("{flag}="));
                    argument.push(invalid);
                    command.arg(argument);
                } else {
                    command.arg(flag).arg(invalid);
                }
            }
            let output = command.output().unwrap();
            if operation == "status" {
                assert_eq!(
                    stdout(&output),
                    include_str!("../../../tests/fixtures/mixed-status.txt")
                );
            } else {
                assert_eq!(output.status.code(), Some(1), "{input}");
                assert!(output.stdout.is_empty());
                let error = String::from_utf8(output.stderr).unwrap();
                assert_eq!(
                    error, "error: Invalid credential input; supply a UTF-8 credential.\n",
                    "{input}"
                );
            }
        }
    }
    assert_eq!(fs::read(dir.path().join("wanikani.json")).unwrap(), before);
    assert!(!dir.path().join("wanikani.json.lock").exists());
}

#[test]
fn removed_configuration_spellings_have_no_compatibility_paths() {
    for flag in [
        "--generation-model",
        "--embedding-model",
        "--embedding-revision",
        "--embedding-dimensions",
        "--embedding-endpoint",
    ] {
        let output = cli().args(["story", flag, "value"]).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{flag}");
        assert!(output.stdout.is_empty());
    }
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("config.toml"), "model='old-flat-config'\n").unwrap();
    let output = cli()
        .args(["status", "--data-dir", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Invalid configuration")
    );
}
