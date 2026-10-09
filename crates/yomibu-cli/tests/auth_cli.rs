use std::process::Command;

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    command.env_clear();
    command
}

#[test]
fn auth_help_lists_component_namespaces_and_credentials_without_reading_configuration() {
    let output = cli().args(["auth", "--help"]).output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    for slot in [
        "wanikani",
        "openai",
        "openai.api-key",
        "wanikani.api-key",
        "http-embeddings",
        "http-embeddings.api-key",
    ] {
        assert!(text.contains(slot), "{text}");
    }
    assert!(text.contains("macOS Keychain"), "{text}");
    assert!(text.contains("missing"), "{text}");
}

#[test]
fn auth_rejects_noninteractive_use_without_exposing_secrets_or_writing_files() {
    let dir = tempfile::tempdir().unwrap();
    for target in [None, Some("wanikani")] {
        let mut command = cli();
        command.arg("auth").arg("--data-dir").arg(dir.path());
        if let Some(target) = target {
            command.arg(target);
        }
        let output = command
            .args(["--openai-api-key", "synthetic-secret\n\u{1b}"])
            .env("YOMIBU_WANIKANI_API_KEY", "synthetic-env-secret")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        let expected = if cfg!(target_os = "macos") {
            "error: Credential setup requires an interactive terminal.\n"
        } else {
            "error: Credential storage is supported only on macOS; use CLI/environment credentials on this platform.\n"
        };
        assert_eq!(stderr, expected);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}

#[test]
fn explicit_only_credentials_do_not_consult_the_system_store() {
    let dir = tempfile::tempdir().unwrap();
    let output = cli()
        .args(["--no-keychain", "sync", "--data-dir"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("error: Missing required setup:\n"),
        "{stderr}"
    );
    assert!(stderr.contains("WaniKani sync"), "{stderr}");
    assert!(!stderr.contains("Cannot access"), "{stderr}");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
