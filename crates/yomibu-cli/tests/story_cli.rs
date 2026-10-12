use std::{fs, path::Path, process::Command};
fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("wanikani.json"),
        include_bytes!("../../../tests/fixtures/mixed.json"),
    )
    .unwrap();
    dir
}
fn cli(dir: &Path, command: &str) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    c.arg("--no-keychain");
    c.env_clear()
        .current_dir(dir)
        .env("YOMIBU_DATA_DIR", dir)
        .args([command, "--select", "2"]);
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
fn malformed_and_oversized_cache_fails_before_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("日本語\n\u{1b}");
    fs::create_dir(&data).unwrap();
    let cache = data.join("wanikani.json");
    let escaped = cache.display().to_string().escape_debug().to_string();
    for oversized in [false, true] {
        let diagnostic = if oversized {
            fs::File::create(&cache)
                .unwrap()
                .set_len(67_108_865)
                .unwrap();
            format!(
                "Cache at {escaped} exceeds 64 MiB; preserve it and select a bounded complete cache."
            )
        } else {
            fs::write(&cache, "{").unwrap();
            format!(
                "Corrupt cache at {escaped}; preserve the file and restore a valid backup or select another --data-dir PATH.: EOF while parsing an object at line 1 column 1"
            )
        };
        let before = fs::metadata(&cache).unwrap();
        let output = cli(&data, "story").output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!("error: {diagnostic}\n")
        );
        let after = fs::metadata(&cache).unwrap();
        assert_eq!(after.len(), before.len());
        assert_eq!(after.modified().unwrap(), before.modified().unwrap());
        if !oversized {
            assert_eq!(fs::read_to_string(&cache).unwrap(), "{");
        }
        assert_eq!(fs::read_dir(&data).unwrap().count(), 1);
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
fn impossible_selection_fails_before_cache_read_or_generation() {
    let dir = setup();
    fs::write(
        dir.path().join("default-story.toml"),
        "[story.targets]\nvocabulary=['one','two','three']\n",
    )
    .unwrap();
    fs::write(dir.path().join("wanikani.json"), "{").unwrap();
    let out = cli(dir.path(), "story")
        .args(["--openai-api-key", "unused"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(!dir.path().join("vectors.json").exists());
    assert!(out.stdout.is_empty());
    let error = String::from_utf8(out.stderr).unwrap();
    assert_eq!(
        error,
        "error: Invalid story request: selection limit must include all targets and be at most 16.\n"
    );
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

#[test]
fn expired_access_reports_refresh_guidance_without_credentials_or_writes() {
    let dir = setup();
    let cache = dir.path().join("wanikani.json");
    let mut data: serde_json::Value = serde_json::from_slice(&fs::read(&cache).unwrap()).unwrap();
    data["snapshot"]["learner"]["subscription"]["active"] = true.into();
    data["snapshot"]["learner"]["subscription"]["period_ends_at"] = "1970-01-01T00:00:00Z".into();
    fs::write(&cache, data.to_string()).unwrap();
    let before = fs::read(&cache).unwrap();
    let output = cli(dir.path(), "story").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "error: Recorded content access has expired; run yomibu sync to refresh access.\n"
    );
    assert_eq!(fs::read(&cache).unwrap(), before);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}
