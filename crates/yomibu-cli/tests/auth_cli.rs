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
    for slot in ["wanikani", "openai", "openai.api-key", "wanikani.api-key"] {
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
    for (namespace, purpose, requirement) in [
        (
            "openai",
            "story generation",
            yomibu::configuration::components::GENERATION_KEY,
        ),
        (
            "wanikani",
            "sync",
            yomibu::configuration::components::SOURCE_KEY,
        ),
    ] {
        for flag in [false, true] {
            let mut command = Command::new("/usr/bin/script");
            command
                .args([
                    "-q",
                    "/dev/null",
                    env!("CARGO_BIN_EXE_yomibu"),
                    "auth",
                    &requirement.name.key(),
                ])
                .env_clear()
                .env("HOME", dir.path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if flag {
                command.args([
                    &format!("--{namespace}-api-key"),
                    "synthetic-secret日本語\n\u{1b}",
                ]);
            } else {
                command.env(
                    requirement.name.environment(),
                    "synthetic-secret日本語\n\u{1b}",
                );
            }
            let mut child = command.spawn().unwrap();
            let mut input = child.stdin.take().unwrap();
            input.write_all(b"\n").unwrap();
            let output = child.wait_with_output().unwrap();
            drop(input);
            let text = String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n");
            assert_eq!(
                output.status.code(),
                Some(0),
                "{text}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                output.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(text.contains(&format!("warning: {namespace}.api-key: CLI/environment input overrides the saved Keychain credential. Omit --{namespace}-api-key and unset {} to use it.\n", requirement.name.environment())), "{text}");
            assert!(
                text.contains(&format!(
                    "{namespace}.api-key (required for {purpose})\n  {}",
                    requirement.description
                )),
                "{text}"
            );
            assert!(text.contains("API key (Enter to skip): "), "{text}");
            assert!(
                text.contains(&format!(
                    "Skipped {namespace}.api-key; {purpose} still needs a credential.\n"
                )),
                "{text}"
            );
            assert!(!text.contains("synthetic-secret"), "{text}");
            assert!(!text.contains('\u{1b}'), "{text}");
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        }
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
    assert!(stderr.contains("  wanikani.api-key:\n    "), "{stderr}");
    assert!(!stderr.contains("Cannot access"), "{stderr}");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[cfg(target_os = "macos")]
#[test]
fn auth_with_supplied_keys_ignores_unrelated_configuration() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), "invalid TOML").unwrap();
    let output = Command::new("/usr/bin/script")
        .args([
            "-q",
            "/dev/null",
            env!("CARGO_BIN_EXE_yomibu"),
            "auth",
            "--data-dir",
        ])
        .arg(dir.path())
        .env_clear()
        .env("YOMIBU_OPENAI_API_KEY", "synthetic-openai")
        .env("YOMIBU_WANIKANI_API_KEY", "synthetic-wanikani")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("synthetic"), "{text}");
    assert!(!text.contains("API key (Enter to skip)"), "{text}");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
