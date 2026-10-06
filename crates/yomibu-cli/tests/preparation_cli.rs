use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    command.env_clear();
    command
}

fn setup() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("wanikani.json"),
        include_str!("../../../tests/fixtures/preparation.json"),
    )
    .unwrap();
    let grammar = dir.path().join("grammar.json");
    fs::write(&grammar, r#"{"version":1,"declarations":["です","は"]}"#).unwrap();
    (dir, grammar)
}

fn stdout(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn prepare_matches_direct_library_use_without_credentials_or_file_changes() {
    use yomibu::{
        adapters::grammar_file,
        cache,
        knowledge::LearnerKnowledgePolicy,
        preparation::{PracticeTarget, prepare_context},
    };

    let (dir, grammar_path) = setup();
    let cache_path = dir.path().join("wanikani.json");
    let before = fs::read(&cache_path).unwrap();
    let grammar_before = fs::read(&grammar_path).unwrap();
    fs::write(dir.path().join("wanikani.lock"), "untouched").unwrap();
    fs::write(dir.path().join("config.toml"), "not parsed").unwrap();
    let data = cache::load(dir.path()).unwrap();
    let grammar = grammar_file::load(&grammar_path).unwrap();
    let targets = [PracticeTarget {
        word: "一つ".into(),
        intended_reading: "ひとつ".into(),
        intended_sense: "one thing".into(),
    }];
    let policy = LearnerKnowledgePolicy::default();
    let result = prepare_context(&data, &grammar, &policy, &targets).unwrap();
    let text = stdout(
        cli()
            .env("WANIKANI_API_TOKEN", "invalid\nunused")
            .args(["prepare", "--data-dir"])
            .arg(dir.path())
            .arg("--grammar-file")
            .arg(&grammar_path)
            .args(["--target", " 一つ : ひとつ : one thing "])
            .output()
            .unwrap(),
    );
    assert_eq!(result.targets[0].subject.id, 2);
    for expected in [
        "Practice context (not a validated Japanese exercise)",
        "Policy: lesson-started",
        "Eligible cached subjects: 2; excluded: 3",
        "Target: 一つ:ひとつ:one thing",
        "Source subject: 2; assignment: 102",
        "Reading: ひとつ (primary: true; accepted: true)",
        "Gloss: one thing (primary: true; accepted: true)",
        "Part of speech: noun",
        "Example (source-attached): 一つあります。 / There is one.",
        "Grammar 1: です",
        "Grammar: not assessed",
        "Reading/sense association: not assessed",
        "Example suitability: not assessed",
        "Linguistic correctness: not assessed",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    assert_eq!(fs::read(&cache_path).unwrap(), before);
    assert_eq!(fs::read(&grammar_path).unwrap(), grammar_before);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 4);
    assert_eq!(
        fs::read_to_string(dir.path().join("wanikani.lock")).unwrap(),
        "untouched"
    );
}

#[test]
fn prepare_errors_have_no_partial_report_or_implicit_target_declaration() {
    let (dir, grammar) = setup();
    let cases = [
        (vec![], "--target"),
        (vec!["--target", "一つ"], "WORD:READING:SENSE"),
        (
            vec!["--target", "一つ: :one thing"],
            "blank intended_reading",
        ),
        (
            vec!["--target", "一つ:ひとつ:single thing"],
            "exact accepted",
        ),
        (
            vec![
                "--target",
                "一つ:ひとつ:one thing",
                "--target",
                "absent:reading:sense",
            ],
            "entry 2",
        ),
        (
            vec![
                "--target",
                "一つ:ひとつ:one thing",
                "--knowledge-policy",
                "invented",
            ],
            "invalid value",
        ),
    ];
    for (arguments, message) in cases {
        let output = cli()
            .args(["prepare", "--data-dir"])
            .arg(dir.path())
            .arg("--grammar-file")
            .arg(&grammar)
            .args(arguments)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::write(&grammar, r#"{"version":2}"#).unwrap();
    let output = cli()
        .args(["prepare", "--data-dir"])
        .arg(dir.path())
        .arg("--grammar-file")
        .arg(&grammar)
        .args(["--target", "一つ:ひとつ:one thing"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unsupported grammar"));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn a_different_policy_recomputes_eligibility_without_rewriting_source_data() {
    let (dir, grammar) = setup();
    let path = dir.path().join("wanikani.json");
    let mut data: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/preparation.json")).unwrap();
    data["snapshot"]["assignments"][1]["passed_at"] = serde_json::Value::Null;
    let bytes = serde_json::to_vec(&data).unwrap();
    fs::write(&path, &bytes).unwrap();
    for (policy, success) in [("lesson-started", true), ("recorded-pass", false)] {
        let output = cli()
            .args(["prepare", "--data-dir"])
            .arg(dir.path())
            .arg("--grammar-file")
            .arg(&grammar)
            .args([
                "--knowledge-policy",
                policy,
                "--target",
                "一つ:ひとつ:one thing",
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.success(), success);
        if !success {
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("NoRecordedPass"));
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn absent_cache_is_not_created_and_home_is_only_needed_without_an_explicit_directory() {
    let (dir, grammar) = setup();
    let absent = dir.path().join("absent");
    let output = cli()
        .args(["prepare", "--data-dir"])
        .arg(&absent)
        .arg("--grammar-file")
        .arg(&grammar)
        .args(["--target", "一つ:ひとつ:one thing"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No cache"));
    assert!(!absent.exists());
    let output = cli()
        .args(["prepare", "--grammar-file"])
        .arg(&grammar)
        .args(["--target", "一つ:ひとつ:one thing"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("HOME is unavailable"));
}

#[test]
fn rendering_escapes_declarations_and_source_fields_while_preserving_sense_colons() {
    let (dir, grammar) = setup();
    let mut data: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/preparation.json")).unwrap();
    data["snapshot"]["learner"]["id"] = "synthetic\nlearner".into();
    let subject = &mut data["snapshot"]["subjects"][1];
    subject["characters"] = "一つ\n偽".into();
    subject["meanings"][0]["meaning"] = "one thing: unit\nfake".into();
    subject["lexical"]["readings"][0]["reading"] = "ひとつ\r字".into();
    subject["lexical"]["parts_of_speech"][0] = "noun\nforged".into();
    subject["lexical"]["context_sentences"][0]["japanese"] = "例\n偽\u{1b}[2J".into();
    subject["lexical"]["context_sentences"][0]["english"] = "example\ttext".into();
    fs::write(
        dir.path().join("wanikani.json"),
        serde_json::to_vec(&data).unwrap(),
    )
    .unwrap();
    fs::write(
        &grammar,
        r#"{"version":1,"declarations":["です\nGrammar: pass\u001b[31m"]}"#,
    )
    .unwrap();
    let text = stdout(
        cli()
            .args(["prepare", "--data-dir"])
            .arg(dir.path())
            .arg("--grammar-file")
            .arg(&grammar)
            .args(["--target", "一つ\n偽:ひとつ\r字:one thing: unit\nfake"])
            .output()
            .unwrap(),
    );
    for expected in [
        r"Learner: synthetic\nlearner",
        r"Target: 一つ\n偽:ひとつ\r字:one thing: unit\nfake",
        r"Part of speech: noun\nforged",
        r"Example (source-attached): 例\n偽\u{1b}[2J / example\ttext",
        r"Grammar 1: です\nGrammar: pass\u{1b}[31m",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    assert!(!text.contains('\u{1b}'));
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("  Target: "))
            .count(),
        1
    );
    assert!(!text.lines().any(|line| line == "Grammar: pass"));
}

#[test]
fn report_explains_each_cached_subject_decision_and_source_interval() {
    let (dir, grammar) = setup();
    let text = stdout(
        cli()
            .args(["prepare", "--data-dir"])
            .arg(dir.path())
            .arg("--grammar-file")
            .arg(&grammar)
            .args(["--target", "一つ:ひとつ:one thing"])
            .output()
            .unwrap(),
    );
    let source = yomibu::cache::load(dir.path()).unwrap();
    assert!(
        text.contains(
            &source
                .sync_started_at
                .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
        )
    );
    assert!(
        text.contains(
            &source
                .sync_completed_at
                .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
        )
    );
    for expected in [
        "Subject 1 (Kanji): excluded: Hidden",
        "Subject 2 (Vocabulary): eligible under lesson-started",
        "Subject 3 (KanaVocabulary): eligible under lesson-started",
        "Subject 4 (Kanji): excluded: NoAssignment",
        "Subject 5 (Vocabulary): excluded: ContentUnavailable",
    ] {
        assert!(text.contains(expected), "{text}");
    }
}

#[test]
fn report_attaches_retained_evidence_to_each_policy_decision() {
    for (hidden_source, policy, reason) in [
        ("subject", "lesson-started", "Hidden"),
        ("assignment", "lesson-started", "Hidden"),
        ("statistic", "lesson-started", "Hidden"),
        ("none", "lesson-started", "NoRecordedLessonStart"),
        ("none", "recorded-pass", "NoRecordedPass"),
    ] {
        let (dir, grammar) = setup();
        let mut data: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/preparation.json")).unwrap();
        let snapshot = &mut data["snapshot"];
        if hidden_source != "subject" {
            snapshot["subjects"][0]["hidden_at"] = serde_json::Value::Null;
        }
        snapshot["assignments"][0]["hidden"] = (hidden_source == "assignment").into();
        snapshot["review_statistics"][0]["hidden"] = (hidden_source == "statistic").into();
        if policy == "recorded-pass" {
            snapshot["assignments"][0]["started_at"] = "2026-09-27T09:00:00Z".into();
        }
        let path = dir.path().join("wanikani.json");
        let bytes = serde_json::to_vec(&data).unwrap();
        fs::write(&path, &bytes).unwrap();
        let text = stdout(
            cli()
                .args(["prepare", "--data-dir"])
                .arg(dir.path())
                .arg("--grammar-file")
                .arg(&grammar)
                .args([
                    "--knowledge-policy",
                    policy,
                    "--target",
                    "一つ:ひとつ:one thing",
                ])
                .output()
                .unwrap(),
        );
        let first = text
            .split("  Subject 1 (Kanji): ")
            .nth(1)
            .unwrap()
            .split("  Subject 2")
            .next()
            .unwrap();
        let hidden_at = if hidden_source == "subject" {
            "Some(2026-09-27T09:00:00Z)"
        } else {
            "None"
        };
        let started_at = if policy == "recorded-pass" {
            "Some(2026-09-27T09:00:00Z)"
        } else {
            "None"
        };
        for expected in [
            format!("excluded: {reason}"),
            format!("Content: available; hidden_at: {hidden_at}"),
            format!(
                "Assignment: 101; hidden: {}; started_at: {started_at}; passed_at: None",
                hidden_source == "assignment"
            ),
            format!(
                "Review statistic: 201; hidden: {}",
                hidden_source == "statistic"
            ),
        ] {
            assert!(first.contains(&expected), "missing {expected:?}:\n{first}");
        }
        for expected in [
            "  Subject 4 (Kanji): excluded: NoAssignment\n    Content: available; hidden_at: None\n    Assignment: none recorded\n    Review statistic: 204; hidden: false\n",
            "  Subject 5 (Vocabulary): excluded: ContentUnavailable\n    Content: unavailable (access limit)\n    Assignment: 105; hidden: false; started_at: Some(2026-09-27T09:00:00Z); passed_at: None\n    Review statistic: 205; hidden: true\n",
            "    Assignment: 103; hidden: false; started_at: Some(2026-09-27T09:00:00Z); passed_at: None\n    Review statistic: none recorded\n",
        ] {
            assert!(text.contains(expected), "missing {expected:?}:\n{text}");
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
