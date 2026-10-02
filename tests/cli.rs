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

#[test]
fn another_process_cannot_sync_while_status_reads_the_locked_cache() {
    use yomibu::cache::{SyncGuard, load};
    let dir = tempfile::tempdir().unwrap();
    cache(dir.path(), include_str!("fixtures/mixed.json"));
    let guard = SyncGuard::acquire(dir.path()).unwrap();
    let before = fs::read(dir.path().join("wanikani.json")).unwrap();
    let token = "synthetic-contending-credential";
    let output = cli()
        .env("WANIKANI_API_TOKEN", token)
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
    assert_eq!(stdout(&output), include_str!("fixtures/mixed-status.txt"));
    assert_eq!(fs::read(dir.path().join("wanikani.json")).unwrap(), before);
    load(dir.path()).unwrap();
    drop(guard);
    assert!(SyncGuard::acquire(dir.path()).is_ok());
}

#[test]
fn preview_example_agrees_with_the_direct_library_result() {
    use yomibu::preview::{CheckOutcome, WordEntry, preview};

    let words = [
        WordEntry {
            text: "猫".into(),
            reading: "ねこ".into(),
            meaning: "cat".into(),
        },
        WordEntry {
            text: "犬".into(),
            reading: "いぬ".into(),
            meaning: "dog".into(),
        },
        WordEntry {
            text: "学校".into(),
            reading: "がっこう".into(),
            meaning: "school".into(),
        },
    ];
    let grammar = ["です".into(), "は".into()];
    let result = preview(&words, &grammar, 2).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = cli()
        .arg("--data-dir")
        .arg(dir.path())
        .args([
            "preview",
            "--word",
            "猫:ねこ:cat",
            "--word",
            "犬:いぬ:dog",
            "--word",
            "学校:がっこう:school",
            "--grammar",
            "です",
            "--grammar",
            "は",
            "--take",
            "2",
        ])
        .output()
        .unwrap();
    let text = stdout(&output);
    let selected: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("  Word: "))
        .collect();
    let expected: Vec<_> = result
        .selected
        .iter()
        .map(|word| format!("{}:{}:{}", word.text, word.reading, word.meaning))
        .collect();
    assert_eq!(selected, expected);
    let descriptions: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("  Grammar: "))
        .collect();
    assert_eq!(descriptions, result.grammar);
    assert_eq!(result.checks.membership, CheckOutcome::Pass);
    assert_eq!(result.checks.count, CheckOutcome::Pass);
    assert_eq!(result.checks.grammar, CheckOutcome::NotAssessed);
    assert_eq!(
        result.checks.linguistic_correctness,
        CheckOutcome::NotAssessed
    );
    for expected in [
        "Supplied-entry membership: pass",
        "Requested entry count: pass",
        "Grammar: not assessed",
        "Readings, meanings, naturalness: not assessed",
        "Manual candidate preview (not a validated Japanese exercise)",
    ] {
        assert!(text.contains(expected), "{text}");
    }
}

#[test]
fn preview_escapes_declarations_so_they_cannot_create_report_lines_or_terminal_controls() {
    let output = cli()
        .args([
            "preview",
            "--word",
            "猫\n犬:ね\rこ\t字:cat\n  Word: 偽:よみ:fake\\n\u{1b}[2J",
            "--grammar",
            "です\nGrammar: pass\r\t\u{1b}[31m\\nは\u{2028}偽\u{2029}末",
            "--take",
            "1",
        ])
        .output()
        .unwrap();
    let text = stdout(&output);
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        [
            "Manual candidate preview (not a validated Japanese exercise)",
            r"  Word: 猫\n犬:ね\rこ\t字:cat\n  Word: 偽:よみ:fake\\n\u{1b}[2J",
            r"  Grammar: です\nGrammar: pass\r\t\u{1b}[31m\\nは\u{2028}偽\u{2029}末",
            "Supplied-entry membership: pass",
            "Requested entry count: pass",
            "Grammar: not assessed",
            "Readings, meanings, naturalness: not assessed",
        ]
    );
}

#[test]
fn preview_reports_invalid_syntax_and_inputs_without_partial_output() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        (vec!["--word", "猫:ねこ:cat"], "--take"),
        (vec!["--take", "1"], "0 supplied entries"),
        (vec!["--word", "猫", "--take", "1"], "TEXT:READING:MEANING"),
        (
            vec!["--word", "猫:ねこ", "--take", "1"],
            "TEXT:READING:MEANING",
        ),
        (vec!["--word", ":ねこ:cat", "--take", "1"], "blank text"),
        (vec!["--word", "猫: :cat", "--take", "1"], "blank reading"),
        (vec!["--word", "猫:ねこ:　", "--take", "1"], "blank meaning"),
        (vec!["--word", "猫:ねこ:cat", "--take", "0"], "positive"),
        (
            vec!["--word", "猫:ねこ:cat", "--take", "2"],
            "1 supplied entries",
        ),
        (
            vec!["--word", "猫:ねこ:cat", "--take", "many"],
            "invalid value",
        ),
        (
            vec!["--word", "猫:ねこ:cat", "--grammar", " ", "--take", "1"],
            "blank description",
        ),
    ];
    for (args, message) in cases {
        let output = cli()
            .arg("--data-dir")
            .arg(dir.path())
            .arg("preview")
            .args(&args)
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {args:?}");
        assert!(output.stdout.is_empty(), "partial output for {args:?}");
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains(message), "{args:?}: {error}");
    }
}

#[test]
fn preview_needs_no_environment_or_files_even_with_an_unusable_data_directory() {
    let root = tempfile::tempdir().unwrap();
    let unusable = root.path().join("not-a-directory");
    fs::write(&unusable, "untouched").unwrap();
    let absent = root.path().join("must-not-be-created");
    for data_dir in [None, Some(&unusable), Some(&absent)] {
        let mut command = cli();
        command.current_dir(root.path());
        if let Some(data_dir) = data_dir {
            command.arg("--data-dir").arg(data_dir);
        }
        let output = command
            .args(["preview", "--word", "猫:ねこ:cat", "--take", "1"])
            .output()
            .unwrap();
        assert!(stdout(&output).contains("  Word: 猫:ねこ:cat"));
        assert_eq!(fs::read_to_string(&unusable).unwrap(), "untouched");
        assert!(!absent.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn preview_ignores_home_and_invalid_tokens_without_creating_a_default_directory() {
    let root = tempfile::tempdir().unwrap();
    let output = cli()
        .current_dir(root.path())
        .env("HOME", root.path())
        .env("WANIKANI_API_TOKEN", "invalid\nsynthetic-preview-token")
        .args(["preview", "--word", "猫:ねこ:cat", "--take", "1"])
        .output()
        .unwrap();
    assert!(stdout(&output).contains("  Word: 猫:ねこ:cat"));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}
