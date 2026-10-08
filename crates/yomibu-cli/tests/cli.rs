use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
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
    fs::write(dir.path().join("config.toml"), "model = 'unused'").unwrap();
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
        "model = 'unused'"
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
    fs::write(dir.path().join("config.toml"), "wanikani_cache = 'source.json'\nselect = 40\ncandidates = 0\nmodel = ''\ntopic = '猫'\nrequest = 'unused.json'\ndictionary_dir = 'unused'\nformat = 'invalid-unused'\n").unwrap();
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
