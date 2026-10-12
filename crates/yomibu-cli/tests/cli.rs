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

#[test]
fn removed_commands_and_options_are_rejected_without_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec!["preview-story"],
        vec!["prepare-retrieval"],
        vec!["analyze"],
        vec!["dictionary"],
        vec!["status"],
        vec!["story", "--json"],
        vec!["sync", "--json"],
        vec!["auth", "--json"],
        vec!["story", "--http-embeddings-api-key", "synthetic-secret"],
        vec!["story", "--embedding-provider", "lexical-baseline"],
        vec!["story", "--embedding-cache", "vectors.json"],
        vec!["story", "--allow-embedding-call"],
        vec!["story", "--dictionary-dir", "dictionary"],
        vec!["story", "--enable", "embeddings"],
        vec!["story", "--enable", "assessment"],
    ] {
        let output = cli().env("HOME", dir.path()).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.starts_with("error:"), "{error}");
        assert!(error.contains("\n\nUsage: yomibu"), "{error}");
        assert!(!error.contains("synthetic-secret"));
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn parser_diagnostics_escape_values_and_preserve_trusted_usage_layout() {
    let hostile = "日本語\n\u{1b}";
    for args in [
        vec![hostile],
        vec!["story", "--candidates", hostile],
        vec!["story", "--knowledge-policy", hostile],
    ] {
        let mut command = cli();
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.arg0("untrusted\n\u{1b}executable");
        }
        let output = command.args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error
                .lines()
                .next()
                .unwrap()
                .contains(&hostile.escape_debug().to_string()),
            "{error}"
        );
        assert!(!error.contains('\u{1b}'));
        assert!(!error.contains("untrusted"));
        assert!(error.contains("\n\nUsage: yomibu"), "{error}");
        assert!(
            error.ends_with("\n\nFor more information, try '--help'.\n"),
            "{error}"
        );
    }
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
fn pasted_credential_values_in_config_are_rejected_without_echoing_them() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    fs::write(
        &config,
        "[application.credentials]\n\"openai.api-key\"=\"synthetic-secret\\n\\u001b日本語\"\n",
    )
    .unwrap();
    for operation in [vec!["story"], vec!["sync"]] {
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
fn missing_or_empty_home_requires_an_explicit_directory() {
    for home in [None, Some("")] {
        let mut command = cli();
        if let Some(home) = home {
            command.env("HOME", home);
        }
        let output = command.arg("sync").output().unwrap();
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
fn another_process_cannot_sync_while_the_cache_is_locked() {
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
    assert!(!help.contains("preview-story"));
    assert!(!help.contains("prepare-retrieval"));
}

#[test]
fn component_model_options_and_provider_keys_have_safe_parser_diagnostics() {
    for command in ["story"] {
        let help = cli().args([command, "--help"]).output().unwrap();
        assert_eq!(help.status.code(), Some(0));
        assert!(help.stderr.is_empty());
        let help = stdout(&help);
        assert!(help.contains(&format!("Usage: yomibu {command}")), "{help}");
        for setting in yomibu::configuration::components::settings() {
            assert!(
                help.contains(&format!("--{}", setting.name().cli())),
                "{help}"
            );
            assert!(help.contains(&setting.name().environment()), "{help}");
        }
        assert!(!help.contains("--generation-model"));
    }
    for requirement in yomibu::configuration::components::credentials() {
        let flag = format!("--{}", requirement.name.cli());
        for args in [
            vec![
                "story",
                &format!("{flag}=synthetic-secret"),
                "--format",
                "invalid",
            ],
            vec![
                "story",
                &flag,
                "synthetic-secret",
                &flag,
                "duplicate-secret",
            ],
            vec![
                "story",
                &flag,
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
}

#[test]
fn credential_redaction_preserves_help_and_version() {
    for requirement in yomibu::configuration::components::credentials() {
        let key = format!("--{}=synthetic-secret", requirement.name.cli());
        for flag in ["--help", "--version"] {
            let output = cli().args([&key, flag]).output().unwrap();
            assert_eq!(output.status.code(), Some(0));
            let text = stdout(&output);
            assert!(!text.contains("synthetic-secret"));
            if flag == "--help" {
                assert!(text.contains("Usage: yomibu"));
            } else {
                assert_eq!(text, concat!("yomibu ", env!("CARGO_PKG_VERSION"), "\n"));
            }
        }
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
        let mut command = cli();
        command.args(["--data-dir", dir.path().to_str().unwrap(), "sync"]);
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
        assert_eq!(output.status.code(), Some(1), "{input}");
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert_eq!(
            error, "error: Invalid credential input; supply a UTF-8 credential.\n",
            "{input}"
        );
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
        .args(["sync", "--data-dir", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Invalid configuration")
    );
}
