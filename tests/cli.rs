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
    cache(dir.path(), include_str!("fixtures/mixed.json"));
    fs::write(dir.path().join("config.toml"), "this is not TOML").unwrap();
    fs::write(dir.path().join("wanikani.lock"), "existing lock").unwrap();
    let before = fs::read(dir.path().join("wanikani.json")).unwrap();
    for args in [
        vec!["--data-dir", dir.path().to_str().unwrap(), "status"],
        vec!["status", "--data-dir", dir.path().to_str().unwrap()],
    ] {
        let output = cli().args(args).output().unwrap();
        assert_eq!(stdout(&output), include_str!("fixtures/mixed-status.txt"));
    }
    assert_eq!(fs::read(dir.path().join("wanikani.json")).unwrap(), before);
    assert_eq!(
        fs::read_to_string(dir.path().join("wanikani.lock")).unwrap(),
        "existing lock"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("config.toml")).unwrap(),
        "this is not TOML"
    );
}

#[test]
fn empty_account_displays_no_reviews_and_no_assignments() {
    let dir = tempfile::tempdir().unwrap();
    cache(dir.path(), include_str!("fixtures/empty.json"));
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
    cache(&data_dir, include_str!("fixtures/empty.json"));
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
            command.env("WANIKANI_API_TOKEN", token);
        }
        let output = command
            .arg("sync")
            .arg("--data-dir")
            .arg(&dir)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("WANIKANI_API_TOKEN"), "{error}");
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
    cache(dir.path(), include_str!("fixtures/empty.json"));
    let output = cli()
        .env("WANIKANI_API_TOKEN", "invalid\nsynthetic-token")
        .args(["status", "--data-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(stdout(&output).contains("Cached WaniKani observations"));
}
