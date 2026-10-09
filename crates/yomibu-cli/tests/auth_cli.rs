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

#[cfg(target_os = "macos")]
#[test]
fn exact_auth_warns_about_overrides_and_allows_skipping_in_a_terminal() {
    use std::{io::Write, process::Stdio};
    let dir = tempfile::tempdir().unwrap();
    for flag in [false, true] {
        let mut command = Command::new("/usr/bin/script");
        command
            .args([
                "-q",
                "/dev/null",
                env!("CARGO_BIN_EXE_yomibu"),
                "auth",
                "openai.api-key",
            ])
            .env_clear()
            .env("HOME", dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if flag {
            command.args(["--openai-api-key", "synthetic-secret日本語\n\u{1b}"]);
        } else {
            command.env("YOMIBU_OPENAI_API_KEY", "synthetic-secret日本語\n\u{1b}");
        }
        let mut child = command.spawn().unwrap();
        child.stdin.take().unwrap().write_all(b"\n").unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n");
        assert!(text.contains("warning: openai.api-key: CLI/environment input overrides the saved Keychain credential. Omit --openai-api-key and unset YOMIBU_OPENAI_API_KEY to use it.\n"), "{text}");
        assert!(
            text.contains("openai.api-key (required for story generation)\n  Story generation:"),
            "{text}"
        );
        assert!(text.contains("API key (Enter to skip): "), "{text}");
        assert!(
            text.contains("Skipped openai.api-key; story generation still needs a credential.\n"),
            "{text}"
        );
        assert!(!text.contains("synthetic-secret"), "{text}");
        assert!(!text.contains('\u{1b}'), "{text}");
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
